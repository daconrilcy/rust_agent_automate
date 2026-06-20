use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::codex;
use crate::reporting::{COMMAND_OUTCOME_PATH_ENV, CommandOutcome, CompletedReport, ReportFailure};
use crate::service_paths::ExecutionContext;

use super::workflow_model::{LoopPolicy, Workflow, WorkflowStep, WorkflowStepKind};
use super::workflow_runner::RunContext;

struct StepCommandSpec {
    current_dir: PathBuf,
    args: Vec<String>,
    cargo_target_dir: PathBuf,
    workspace_root: PathBuf,
    outcome_path: PathBuf,
}

struct CurrentDirGuard {
    previous: PathBuf,
}

impl CurrentDirGuard {
    fn change_to(path: &Path) -> io::Result<Self> {
        let previous = std::env::current_dir()?;
        std::env::set_current_dir(normalize_working_dir(path))?;
        Ok(Self { previous })
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.previous);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowStepOutcome {
    pub(crate) status_code: Option<i32>,
    pub(crate) artifact_path: Option<PathBuf>,
    pub(crate) clean: Option<bool>,
}

#[derive(Debug)]
pub struct AutomateReport {
    pub completed_cycles: u32,
    pub clean_stop: bool,
    pub step_results: Vec<StepResult>,
}

#[derive(Debug)]
pub struct StepResult {
    pub cycle: u32,
    pub name: String,
    pub status_code: Option<i32>,
    pub artifact_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepExecution {
    pub status_code: Option<i32>,
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub command_outcome: Option<CommandOutcome>,
}

pub fn run_step(
    workflow: &Workflow,
    step: &WorkflowStep,
    context: &RunContext,
) -> io::Result<StepExecution> {
    let command = build_step_command_spec(workflow, step, context);
    match step.kind {
        WorkflowStepKind::ServiceCommand => execute_service_step_in_process(&command),
        WorkflowStepKind::DirectRun | WorkflowStepKind::NestedCommand => {
            let output = execute_step_command(&command)?;
            let command_outcome = decode_command_outcome(&command.outcome_path)?;
            Ok(step_execution_from_output(output, command_outcome))
        }
    }
}

pub(crate) fn command_outcome_for_step(
    _workflow: &Workflow,
    step: &WorkflowStep,
    _context: &RunContext,
    output: &StepExecution,
) -> io::Result<WorkflowStepOutcome> {
    // Transport decoding lives in this module.
    //
    // The workflow runner only consumes this reduced state and does not depend
    // on the persisted `CommandOutcome` payload shape beyond these fields.
    if let Some(outcome) = &output.command_outcome {
        let mut outcome = outcome.clone();
        outcome.status_code = normalized_status_code(outcome.status_code);
        return Ok(WorkflowStepOutcome {
            status_code: outcome.status_code,
            artifact_path: outcome.artifact_path,
            clean: outcome.clean,
        });
    }

    if matches!(step.kind, WorkflowStepKind::DirectRun) {
        return Ok(WorkflowStepOutcome {
            status_code: normalized_status_code(output.status_code),
            artifact_path: None,
            clean: None,
        });
    }

    Err(io::Error::other(format!(
        "l'etape automate '{}' doit produire un resultat structure",
        step.name
    )))
}

pub fn evaluate_clean_stop(policy: &LoopPolicy, context: &RunContext) -> io::Result<bool> {
    // Loop termination is driven by the structured `clean` status emitted by
    // the configured audit step, not by parsing free-form markdown output.
    if let Some(clean) = context.clean_by_step.get(&policy.audit_step).copied() {
        return Ok(clean);
    }

    let artifact_hint = context
        .artifacts_by_step
        .get(&policy.audit_step)
        .map(|path| format!(" artefact observe: {}", path.display()))
        .unwrap_or_default();

    Err(io::Error::other(format!(
        "l'etape automate '{}' doit produire un resultat structure avec le statut clean avant l'evaluation de loop_policy.{}",
        policy.audit_step, artifact_hint
    )))
}

pub fn normalized_status_code(status_code: Option<i32>) -> Option<i32> {
    status_code.map(|code| codex::process_exit_code(Some(code)))
}

fn build_step_command_spec(
    workflow: &Workflow,
    step: &WorkflowStep,
    context: &RunContext,
) -> StepCommandSpec {
    StepCommandSpec {
        current_dir: child_current_dir(context).to_path_buf(),
        args: crate::automate::step_args::resolve_step_args(workflow, step, context),
        cargo_target_dir: cargo_target_dir_for_context(context, std::process::id()),
        workspace_root: context.workspace_root.clone(),
        outcome_path: temp_outcome_file_path(),
    }
}

fn execute_step_command(spec: &StepCommandSpec) -> io::Result<std::process::Output> {
    let current_exe = env::current_exe()?;
    let mut command = Command::new(current_exe);
    command
        .current_dir(&spec.current_dir)
        .args(&spec.args)
        .env("CARGO_TARGET_DIR", &spec.cargo_target_dir)
        .env(
            crate::service_paths::WORKSPACE_ROOT_ENV,
            &spec.workspace_root,
        )
        .env(crate::service_paths::WORKSPACE_ROOT_OVERRIDE_ENV, "1")
        .env(COMMAND_OUTCOME_PATH_ENV, &spec.outcome_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    command.output()
}

fn execute_service_step_in_process(spec: &StepCommandSpec) -> io::Result<StepExecution> {
    let context = ExecutionContext::from_workspace_root(spec.workspace_root.clone())
        .with_output_root(spec.current_dir.clone());
    let _guard = CurrentDirGuard::change_to(&spec.current_dir)?;
    let dispatch = crate::command_registry::parse_service_subcommand_for_context(&spec.args, &context)
        .ok_or_else(|| {
            io::Error::other(format!(
                "l'etape automate '{}' n'est pas une commande de service prise en charge",
                spec.args.first().cloned().unwrap_or_default()
            ))
        })?
        .map_err(|error| io::Error::other(format!("echec du parseur interne: {error:?}")))?;

    match dispatch.execute_silently() {
        Ok(report) => Ok(step_execution_from_report(report)),
        Err(error) => Ok(step_execution_from_report_failure(
            dispatch.command_name(),
            error,
        )),
    }
}

fn step_execution_from_output(
    output: std::process::Output,
    command_outcome: Option<CommandOutcome>,
) -> StepExecution {
    StepExecution {
        status_code: normalized_status_code(output.status.code()),
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        command_outcome,
    }
}

fn step_execution_from_report(report: CompletedReport) -> StepExecution {
    StepExecution {
        status_code: normalized_status_code(report.outcome.status_code),
        success: report.status_code == 0,
        stdout: report.stdout,
        stderr: report.stderr,
        command_outcome: Some(report.outcome),
    }
}

fn step_execution_from_report_failure(command_name: &str, failure: ReportFailure) -> StepExecution {
    match failure {
        ReportFailure::CodexCall(error) => StepExecution {
            status_code: Some(1),
            success: false,
            stdout: String::new(),
            stderr: error,
            command_outcome: None,
        },
        ReportFailure::MissingFinalMessage {
            status_code,
            stdout,
            stderr,
        } => StepExecution {
            status_code: normalized_status_code(Some(status_code)),
            success: false,
            stdout,
            stderr,
            command_outcome: Some(CommandOutcome {
                command_name: command_name.to_string(),
                status_code: Some(status_code),
                final_message_present: false,
                artifact_path: None,
                clean: None,
            }),
        },
        ReportFailure::Save { clean, error, .. } => StepExecution {
            status_code: Some(1),
            success: false,
            stdout: String::new(),
            stderr: error,
            command_outcome: Some(CommandOutcome {
                command_name: command_name.to_string(),
                status_code: Some(1),
                final_message_present: true,
                artifact_path: None,
                clean,
            }),
        },
    }
}

fn temp_outcome_file_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    std::env::temp_dir().join(format!(
        "rust_agent_command_outcome_{}_{}.json",
        std::process::id(),
        timestamp
    ))
}

// Child steps own process execution and publish their transport payload through
// `RUST_AGENT_COMMAND_OUTCOME_PATH`.
//
// This module decodes that payload into `CommandOutcome`; workflow-facing
// shaping happens in `command_outcome_for_step`, and later artifact
// normalization remains the runner's responsibility.
fn decode_command_outcome(path: &Path) -> io::Result<Option<CommandOutcome>> {
    let content = match fs::read(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let _ = fs::remove_file(path);

    serde_json::from_slice(&content)
        .map(Some)
        .map_err(|error| io::Error::other(format!("resultat structure invalide: {error}")))
}

fn cargo_target_dir_for_context(context: &RunContext, process_id: u32) -> PathBuf {
    context
        .workspace_root
        .join(format!(".cargo-target-loop-{process_id}"))
}

fn child_current_dir(context: &RunContext) -> &Path {
    &context.workspace_root
}

fn normalize_working_dir(path: &Path) -> PathBuf {
    let text = path.display().to_string();
    if let Some(stripped) = text.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::automate::workflow_model::parse_workflow;
    #[test]
    fn decode_command_outcome_rejects_invalid_json() {
        let path = temp_outcome_file_path();
        fs::write(&path, b"{ invalid json").expect("ecriture du resultat invalide");

        let error = decode_command_outcome(&path).expect_err("le JSON invalide doit echouer");

        assert!(error.to_string().contains("resultat structure invalide"));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn cargo_target_dir_is_scoped_to_the_workspace_root() {
        let context = RunContext {
            workspace_root: PathBuf::from("C:\\dev\\rust_agent"),
            ..RunContext::default()
        };

        let path = cargo_target_dir_for_context(&context, 1234);

        assert_eq!(
            path,
            Path::new("C:\\dev\\rust_agent\\.cargo-target-loop-1234")
        );
    }

    #[test]
    fn run_step_workspace_root_is_used_as_child_current_dir() {
        let context = RunContext {
            workspace_root: PathBuf::from("C:\\dev\\rust_agent\\workspace"),
            ..RunContext::default()
        };

        assert_eq!(
            child_current_dir(&context),
            Path::new("C:\\dev\\rust_agent\\workspace")
        );
    }

    #[test]
    fn step_execution_from_output_normalizes_status_codes() {
        use std::os::windows::process::ExitStatusExt;

        let output = std::process::Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: b"ok".to_vec(),
            stderr: b"err".to_vec(),
        };

        let execution = step_execution_from_output(output, None);

        assert_eq!(execution.status_code, Some(0));
        assert!(execution.success);
        assert_eq!(execution.stdout, "ok");
        assert_eq!(execution.stderr, "err");
    }

    #[test]
    fn command_outcome_for_step_extracts_automation_state() {
        let workflow = parse_workflow(
            r#"{"steps":[{"name":"audit","rust_command":["audit","--target","{target}"]}]}"#,
        )
        .expect("workflow valide");
        let step = &workflow.steps[0];
        let output = StepExecution {
            status_code: Some(0),
            success: true,
            stdout: String::new(),
            stderr: String::new(),
            command_outcome: Some(CommandOutcome {
                command_name: step.name.clone(),
                status_code: Some(0),
                final_message_present: true,
                artifact_path: Some(PathBuf::from("C:\\repo\\.audit\\audit.md")),
                clean: Some(true),
            }),
        };

        let outcome = command_outcome_for_step(&workflow, step, &RunContext::default(), &output)
            .expect("resultat structure");

        assert_eq!(outcome.status_code, Some(0));
        assert_eq!(
            outcome.artifact_path,
            Some(PathBuf::from("C:\\repo\\.audit\\audit.md"))
        );
        assert_eq!(outcome.clean, Some(true));
    }
}
