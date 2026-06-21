use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::reporting::{COMMAND_OUTCOME_PATH_ENV, CommandOutcome, CompletedReport, ReportFailure};
use crate::service_paths::ExecutionContext;

use super::transport::{decode_command_outcome, normalized_status_code};
use super::workflow_model::{Workflow, WorkflowStep, WorkflowStepKind};
use super::workflow_runner::RunContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepOutcome {
    pub status_code: Option<i32>,
    pub artifact_path: Option<PathBuf>,
    pub clean: Option<bool>,
}

struct StepCommandSpec {
    current_dir: PathBuf,
    args: Vec<String>,
    cargo_target_dir: PathBuf,
    workspace_root: PathBuf,
    outcome_path: PathBuf,
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
) -> Result<StepExecution, super::AutomationError> {
    let command = build_step_command_spec(workflow, step, context);
    match step.kind {
        WorkflowStepKind::ServiceCommand => execute_service_step_in_process(&command),
        WorkflowStepKind::DirectRun | WorkflowStepKind::NestedCommand => {
            let output = execute_step_command(&command).map_err(|error| {
                super::AutomationError::structured_result_decode(error.to_string())
            })?;
            let command_outcome = decode_command_outcome(&command.outcome_path)?;
            Ok(step_execution_from_output(output, command_outcome))
        }
    }
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

fn execute_service_step_in_process(
    spec: &StepCommandSpec,
) -> Result<StepExecution, super::AutomationError> {
    let context = ExecutionContext::from_workspace_root(spec.workspace_root.clone())
        .with_output_root(spec.current_dir.clone());
    let dispatch = crate::service_command::dispatch_service_args_for_context(&spec.args, &context)
        .map_err(super::AutomationError::internal_parse_failure)?
        .ok_or_else(|| {
            super::AutomationError::unsupported_service_command(
                spec.args.first().map(String::as_str),
            )
        })?;

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
    let failure = crate::reporting::command_failure_outcome(command_name, &failure);
    StepExecution {
        status_code: normalized_status_code(Some(failure.status_code)),
        success: false,
        stdout: failure.stdout,
        stderr: failure.stderr,
        command_outcome: failure.command_outcome,
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

fn cargo_target_dir_for_context(context: &RunContext, process_id: u32) -> PathBuf {
    context
        .workspace_root
        .join(format!(".cargo-target-loop-{process_id}"))
}

fn child_current_dir(context: &RunContext) -> &Path {
    &context.workspace_root
}

pub fn validate_and_normalize_outcome(
    workflow: &Workflow,
    step: &WorkflowStep,
    context: &RunContext,
    output: &StepExecution,
) -> Result<StepOutcome, super::AutomationError> {
    let mut outcome = command_outcome_for_step(workflow, step, context, output)?;
    if let Some(path) = outcome.artifact_path.take() {
        outcome.artifact_path = Some(super::artifact_resolution::normalize_artifact_path(
            &path,
            &context.workspace_root,
        )?);
    }

    Ok(outcome)
}

pub fn command_outcome_for_step(
    _workflow: &Workflow,
    step: &WorkflowStep,
    _context: &RunContext,
    output: &StepExecution,
) -> Result<StepOutcome, super::AutomationError> {
    if let Some(outcome) = &output.command_outcome {
        let mut outcome = outcome.clone();
        outcome.status_code = normalized_status_code(outcome.status_code);
        return Ok(StepOutcome {
            status_code: outcome.status_code,
            artifact_path: outcome.artifact_path,
            clean: outcome.clean,
        });
    }

    if matches!(step.kind, WorkflowStepKind::DirectRun) {
        return Ok(StepOutcome {
            status_code: normalized_status_code(output.status_code),
            artifact_path: None,
            clean: None,
        });
    }

    Err(super::AutomationError::missing_structured_result(
        &step.name,
    ))
}
