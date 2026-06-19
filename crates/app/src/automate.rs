use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::codex::{DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ReasoningEffort};
use crate::command_registry;
use crate::reporting::{COMMAND_OUTCOME_PATH_ENV, CommandOutcome};

#[derive(Debug, PartialEq, Eq)]
pub struct AutomateCommand {
    pub workflow_path: PathBuf,
    pub workflow: Workflow,
    pub initial_prompt: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RefactorAutomateCommand {
    pub workflow: Workflow,
    pub initial_prompt: String,
    pub target_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Workflow {
    #[serde(default)]
    pub defaults: WorkflowDefaults,
    pub steps: Vec<WorkflowStep>,
    #[serde(default)]
    pub loop_policy: Option<LoopPolicy>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct WorkflowDefaults {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub reasoning: Option<ReasoningEffort>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

impl Default for WorkflowDefaults {
    fn default() -> Self {
        Self {
            model: Some(DEFAULT_MODEL.to_string()),
            reasoning: Some(DEFAULT_REASONING_EFFORT),
            timeout_seconds: Some(1_800),
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct WorkflowStep {
    pub name: String,
    pub rust_command: Vec<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub reasoning: Option<ReasoningEffort>,
    #[serde(default = "default_fresh_codex_call")]
    pub fresh_codex_call: bool,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct LoopPolicy {
    pub audit_step: String,
    #[serde(default = "default_max_cycles")]
    pub max_cycles: u32,
    #[serde(default = "default_clean_markers")]
    pub clean_markers: Vec<String>,
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

#[derive(Debug, Default)]
struct RunContext {
    initial_prompt: String,
    target_dir: PathBuf,
    current_cycle: u32,
    last_output: String,
    last_artifact: Option<PathBuf>,
    artifacts_by_step: BTreeMap<String, PathBuf>,
    clean_by_step: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StepExecution {
    status_code: Option<i32>,
    success: bool,
    stdout: String,
    stderr: String,
    command_outcome: Option<CommandOutcome>,
}

pub fn load_workflow(path: &Path) -> Result<Workflow, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("impossible de lire le workflow {}: {error}", path.display()))?;

    parse_workflow(&content)
}

pub fn parse_workflow(content: &str) -> Result<Workflow, String> {
    let workflow: Workflow = serde_json::from_str(content)
        .map_err(|error| format!("workflow JSON invalide: {error}"))?;

    validate_workflow(workflow)
}

pub fn default_refactor_workflow() -> Workflow {
    parse_workflow(DEFAULT_REFACTOR_WORKFLOW)
        .expect("le workflow de refactoring integre est valide")
}

pub fn resolve_target_dir(path: PathBuf) -> Result<PathBuf, String> {
    let path = if path.is_absolute() {
        path
    } else {
        env::current_dir()
            .map_err(|error| format!("impossible de lire le repertoire courant: {error}"))?
            .join(path)
    };

    let metadata = fs::metadata(&path).map_err(|error| {
        format!(
            "impossible d'acceder au dossier cible {}: {error}",
            path.display()
        )
    })?;

    if !metadata.is_dir() {
        return Err(format!("le dossier cible doit exister: {}", path.display()));
    }

    fs::canonicalize(&path).map_err(|error| {
        format!(
            "impossible de resoudre le dossier cible {}: {error}",
            path.display()
        )
    })
}

pub fn run_workflow(
    workflow: &Workflow,
    initial_prompt: &str,
    target_dir: &Path,
) -> io::Result<AutomateReport> {
    run_workflow_with_executor(workflow, initial_prompt, target_dir, run_step)
}

fn run_workflow_with_executor<F>(
    workflow: &Workflow,
    initial_prompt: &str,
    target_dir: &Path,
    mut run_step: F,
) -> io::Result<AutomateReport>
where
    F: FnMut(&Workflow, &WorkflowStep, &RunContext) -> io::Result<StepExecution>,
{
    let mut context = RunContext {
        initial_prompt: initial_prompt.to_string(),
        target_dir: target_dir.to_path_buf(),
        current_cycle: 1,
        ..RunContext::default()
    };
    let max_cycles = workflow
        .loop_policy
        .as_ref()
        .map_or(1, |policy| policy.max_cycles.max(1));
    let mut report = AutomateReport {
        completed_cycles: 0,
        clean_stop: false,
        step_results: Vec::new(),
    };

    for cycle in 1..=max_cycles {
        context.current_cycle = cycle;

        for step in &workflow.steps {
            eprintln!(
                "Automate cycle {cycle}/{max_cycles}: etape '{}'...",
                step.name
            );
            let output = run_step(workflow, step, &context)?;
            let artifact_path = output
                .command_outcome
                .as_ref()
                .and_then(|outcome| outcome.artifact_path.clone())
                .or_else(|| extract_artifact_path(&output.stdout));
            if let Some(path) = &artifact_path {
                context
                    .artifacts_by_step
                    .insert(step.name.clone(), path.clone());
                context.last_artifact = Some(path.clone());
            }
            if let Some(clean) = output
                .command_outcome
                .as_ref()
                .and_then(|outcome| outcome.clean)
            {
                context.clean_by_step.insert(step.name.clone(), clean);
            }
            context.last_output = output.stdout.clone();

            report.step_results.push(StepResult {
                cycle,
                name: step.name.clone(),
                status_code: output.status_code,
                artifact_path,
            });

            if !output.success {
                return Err(io::Error::other(format!(
                    "l'etape automate '{}' a echoue avec le statut {}{}{}",
                    step.name,
                    format_status_code(output.status_code),
                    format_stream("stdout", &output.stdout),
                    format_stream("stderr", &output.stderr)
                )));
            }
        }

        report.completed_cycles = cycle;

        if workflow
            .loop_policy
            .as_ref()
            .is_some_and(|policy| is_clean(policy, &context))
        {
            report.clean_stop = true;
            break;
        }
    }

    Ok(report)
}

fn validate_workflow(workflow: Workflow) -> Result<Workflow, String> {
    if workflow.steps.is_empty() {
        return Err("le workflow doit definir au moins une etape".to_string());
    }

    for step in &workflow.steps {
        if step.name.trim().is_empty() {
            return Err("chaque etape doit avoir un nom non vide".to_string());
        }

        if step.rust_command.is_empty() {
            return Err(format!("l'etape '{}' doit definir rust_command", step.name));
        }
    }

    if let Some(policy) = &workflow.loop_policy
        && !workflow
            .steps
            .iter()
            .any(|step| step.name == policy.audit_step)
    {
        return Err(format!(
            "loop_policy.audit_step '{}' ne correspond a aucune etape",
            policy.audit_step
        ));
    }

    Ok(workflow)
}

fn run_step(
    workflow: &Workflow,
    step: &WorkflowStep,
    context: &RunContext,
) -> io::Result<StepExecution> {
    let current_exe = env::current_exe()?;
    let mut command = Command::new(current_exe);
    let outcome_path = temp_outcome_file_path();
    command
        .args(resolve_step_args(workflow, step, context))
        .env(COMMAND_OUTCOME_PATH_ENV, &outcome_path)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = command.output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let command_outcome = read_command_outcome(&outcome_path)?;

    Ok(StepExecution {
        status_code: output.status.code(),
        success: output.status.success(),
        stdout,
        stderr,
        command_outcome,
    })
}

fn resolve_step_args(
    workflow: &Workflow,
    step: &WorkflowStep,
    context: &RunContext,
) -> Vec<String> {
    let mut args = Vec::new();
    let model = step
        .model
        .as_deref()
        .or(workflow.defaults.model.as_deref())
        .unwrap_or(DEFAULT_MODEL);
    let reasoning = step
        .reasoning
        .or(workflow.defaults.reasoning)
        .unwrap_or(DEFAULT_REASONING_EFFORT);
    let timeout_seconds = step
        .timeout_seconds
        .or(workflow.defaults.timeout_seconds)
        .unwrap_or(1_800);

    for value in &step.rust_command {
        args.push(expand_placeholders(value, context));
    }

    if command_accepts_codex_options(&args) {
        if !step.fresh_codex_call {
            args.push("--continue-codex".to_string());
        }

        args.extend([
            "--model".to_string(),
            model.to_string(),
            "--reasoning".to_string(),
            reasoning.to_string(),
            "--timeout-seconds".to_string(),
            timeout_seconds.to_string(),
        ]);
    } else if command_is_direct_run(&args) {
        if !step.fresh_codex_call {
            args.insert(0, "--continue-codex".to_string());
        }

        args.splice(
            0..0,
            [
                "--model".to_string(),
                model.to_string(),
                "--reasoning".to_string(),
                reasoning.to_string(),
            ],
        );
    }

    args
}

fn command_accepts_codex_options(args: &[String]) -> bool {
    args.first()
        .map(String::as_str)
        .is_some_and(command_registry::accepts_codex_options)
}

fn command_is_direct_run(args: &[String]) -> bool {
    command_registry::is_direct_run(args.first().map(String::as_str))
}

fn expand_placeholders(value: &str, context: &RunContext) -> String {
    let mut expanded = value
        .replace("{initial_prompt}", &context.initial_prompt)
        .replace("{target}", &context.target_dir.display().to_string())
        .replace("{cycle}", &context.current_cycle.to_string())
        .replace("{last_output}", &context.last_output);

    if let Some(path) = &context.last_artifact {
        expanded = expanded.replace("{last_artifact}", &path.display().to_string());
    }

    for (name, path) in &context.artifacts_by_step {
        expanded = expanded.replace(&format!("{{artifact:{name}}}"), &path.display().to_string());
    }

    expanded
}

fn extract_artifact_path(stdout: &str) -> Option<PathBuf> {
    stdout
        .lines()
        .find_map(|line| line.split_once(" enregistre dans ").map(|(_, path)| path))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

fn is_clean(policy: &LoopPolicy, context: &RunContext) -> bool {
    if let Some(clean) = context.clean_by_step.get(&policy.audit_step).copied() {
        return clean;
    }

    let Some(audit_path) = context.artifacts_by_step.get(&policy.audit_step) else {
        return false;
    };
    let Ok(content) = fs::read_to_string(audit_path) else {
        return false;
    };
    let content = content.to_lowercase();

    policy
        .clean_markers
        .iter()
        .any(|marker| content.contains(&marker.to_lowercase()))
}

fn format_stream(label: &str, content: &str) -> String {
    let content = content.trim();
    if content.is_empty() {
        String::new()
    } else {
        format!("\n{label}:\n{content}")
    }
}

fn format_status_code(code: Option<i32>) -> String {
    code.map_or_else(|| "inconnu".to_string(), |code| code.to_string())
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

fn read_command_outcome(path: &Path) -> io::Result<Option<CommandOutcome>> {
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

fn default_max_cycles() -> u32 {
    2
}

fn default_fresh_codex_call() -> bool {
    true
}

fn default_clean_markers() -> Vec<String> {
    vec![
        "no actionable findings".to_string(),
        "aucun finding actionnable".to_string(),
        "aucune correction requise".to_string(),
    ]
}

const DEFAULT_REFACTOR_WORKFLOW: &str = include_str!("../../../workflows/refactor.json");

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn parses_default_refactor_workflow() {
        let workflow = default_refactor_workflow();

        assert_eq!(workflow.steps.len(), 7);
        assert_eq!(workflow.steps[0].name, "audit");
        assert_eq!(workflow.steps[1].name, "plan");
        assert_eq!(workflow.steps[2].name, "implementation");
        assert_eq!(workflow.steps[3].name, "dev_review_corrections");
        assert_eq!(
            workflow
                .loop_policy
                .as_ref()
                .map(|policy| policy.audit_step.as_str()),
            Some("alignment_audit")
        );
    }

    #[test]
    fn rejects_empty_workflow() {
        let error = parse_workflow(r#"{"steps":[]}"#).expect_err("workflow vide invalide");

        assert_eq!(error, "le workflow doit definir au moins une etape");
    }

    #[test]
    fn defaults_steps_to_fresh_codex_calls() {
        let workflow = parse_workflow(
            r#"{
              "steps":[{"name":"audit","rust_command":["audit","--target","{target}"]}]
            }"#,
        )
        .expect("workflow valide");

        assert!(workflow.steps[0].fresh_codex_call);
    }

    #[test]
    fn expands_artifact_placeholders() {
        let mut context = RunContext {
            initial_prompt: "Durcir le code".to_string(),
            target_dir: PathBuf::from("C:\\dev\\rust_agent"),
            current_cycle: 2,
            ..RunContext::default()
        };
        context.artifacts_by_step.insert(
            "plan".to_string(),
            PathBuf::from("C:\\dev\\rust_agent\\.plan\\plan.md"),
        );

        let result = expand_placeholders(
            "cycle={cycle}; target={target}; plan={artifact:plan}; prompt={initial_prompt}",
            &context,
        );

        assert!(result.contains("cycle=2"));
        assert!(result.contains("C:\\dev\\rust_agent"));
        assert!(result.contains(".plan\\plan.md"));
        assert!(result.contains("Durcir le code"));
    }

    #[test]
    fn extracts_artifact_path_from_command_output() {
        let output = "Plan enregistre dans C:\\dev\\rust_agent\\.plan\\plan-1.md\n\n# Plan";

        let path = extract_artifact_path(output).expect("artefact attendu");

        assert_eq!(path, PathBuf::from("C:\\dev\\rust_agent\\.plan\\plan-1.md"));
    }

    #[test]
    fn injects_model_reasoning_and_timeout_for_service_commands() {
        let workflow = parse_workflow(
            r#"{
              "defaults": {"model":"gpt-x","reasoning":"medium","timeout_seconds":42},
              "steps":[{"name":"audit","rust_command":["audit","--target","{target}"]}]
            }"#,
        )
        .expect("workflow valide");
        let context = RunContext {
            target_dir: PathBuf::from("C:\\repo"),
            current_cycle: 1,
            ..RunContext::default()
        };

        let args = resolve_step_args(&workflow, &workflow.steps[0], &context);

        assert_eq!(
            args,
            vec![
                "audit",
                "--target",
                "C:\\repo",
                "--model",
                "gpt-x",
                "--reasoning",
                "medium",
                "--timeout-seconds",
                "42"
            ]
        );
    }

    #[test]
    fn step_model_reasoning_and_timeout_override_workflow_defaults() {
        let workflow = parse_workflow(
            r#"{
              "defaults": {"model":"gpt-default","reasoning":"low","timeout_seconds":42},
              "steps":[
                {
                  "name":"audit",
                  "rust_command":["audit","--target","{target}"],
                  "model":"gpt-step",
                  "reasoning":"high",
                  "timeout_seconds":99
                }
              ]
            }"#,
        )
        .expect("workflow valide");
        let context = RunContext {
            target_dir: PathBuf::from("C:\\repo"),
            current_cycle: 1,
            ..RunContext::default()
        };

        let args = resolve_step_args(&workflow, &workflow.steps[0], &context);

        assert_eq!(
            args,
            vec![
                "audit",
                "--target",
                "C:\\repo",
                "--model",
                "gpt-step",
                "--reasoning",
                "high",
                "--timeout-seconds",
                "99"
            ]
        );
    }

    #[test]
    fn injects_resume_for_non_fresh_service_steps() {
        let workflow = parse_workflow(
            r#"{
              "defaults": {"model":"gpt-x","reasoning":"medium","timeout_seconds":42},
              "steps":[{"name":"loop","rust_command":["fix-loop","plan","{last_artifact}"],"fresh_codex_call":false}]
            }"#,
        )
        .expect("workflow valide");
        let context = RunContext {
            last_artifact: Some(PathBuf::from("C:\\repo\\.plan\\plan.md")),
            ..RunContext::default()
        };

        let args = resolve_step_args(&workflow, &workflow.steps[0], &context);

        assert!(args.contains(&"--continue-codex".to_string()));
        assert_eq!(args[0], "fix-loop");
    }

    #[test]
    fn step_model_and_reasoning_override_defaults_for_direct_run_commands() {
        let workflow = parse_workflow(
            r#"{
              "defaults": {"model":"gpt-default","reasoning":"low"},
              "steps":[
                {
                  "name":"implementation",
                  "rust_command":["--mode","exec","Implement"],
                  "model":"gpt-step",
                  "reasoning":"medium"
                }
              ]
            }"#,
        )
        .expect("workflow valide");
        let context = RunContext::default();

        let args = resolve_step_args(&workflow, &workflow.steps[0], &context);

        assert_eq!(
            args,
            vec![
                "--model",
                "gpt-step",
                "--reasoning",
                "medium",
                "--mode",
                "exec",
                "Implement"
            ]
        );
    }

    #[test]
    fn injects_model_reasoning_and_resume_before_direct_run_prompt() {
        let workflow = parse_workflow(
            r#"{
              "defaults": {"model":"gpt-x","reasoning":"high"},
              "steps":[{"name":"commit","rust_command":["--mode","exec","Commit"],"fresh_codex_call":false}]
            }"#,
        )
        .expect("workflow valide");
        let context = RunContext::default();

        let args = resolve_step_args(&workflow, &workflow.steps[0], &context);

        assert_eq!(
            args,
            vec![
                "--model",
                "gpt-x",
                "--reasoning",
                "high",
                "--continue-codex",
                "--mode",
                "exec",
                "Commit"
            ]
        );
    }

    #[test]
    fn run_workflow_chains_artifacts_from_structured_outcomes() {
        let workflow = parse_workflow(
            r#"{
              "steps":[
                {"name":"audit","rust_command":["audit","--target","{target}"]},
                {"name":"plan","rust_command":["plan","{artifact:audit}"]}
              ]
            }"#,
        )
        .expect("workflow valide");
        let calls = Rc::new(RefCell::new(Vec::new()));
        let seen = Rc::clone(&calls);

        let report = run_workflow_with_executor(
            &workflow,
            "Durcir",
            Path::new("C:\\repo"),
            move |workflow, step, context| {
                seen.borrow_mut()
                    .push(resolve_step_args(workflow, step, context));

                let artifact_path = match step.name.as_str() {
                    "audit" => Some(PathBuf::from("C:\\repo\\.audit\\audit.md")),
                    "plan" => Some(PathBuf::from("C:\\repo\\.plan\\plan.md")),
                    _ => None,
                };

                Ok(StepExecution {
                    status_code: Some(0),
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                    command_outcome: Some(CommandOutcome {
                        command_name: step.name.clone(),
                        status_code: Some(0),
                        final_message_present: true,
                        artifact_path,
                        clean: None,
                    }),
                })
            },
        )
        .expect("workflow execute");

        assert_eq!(report.completed_cycles, 1);
        let calls = calls.borrow();
        assert_eq!(
            calls[0],
            vec![
                "audit",
                "--target",
                "C:\\repo",
                "--model",
                DEFAULT_MODEL,
                "--reasoning",
                "low",
                "--timeout-seconds",
                "1800"
            ]
        );
        assert_eq!(calls[1][0], "plan");
        assert_eq!(calls[1][1], "C:\\repo\\.audit\\audit.md");
    }

    #[test]
    fn run_workflow_stops_on_failed_step_with_context() {
        let workflow = parse_workflow(
            r#"{"steps":[{"name":"audit","rust_command":["audit","--target","{target}"]}]}"#,
        )
        .expect("workflow valide");

        let error = run_workflow_with_executor(
            &workflow,
            "Durcir",
            Path::new("C:\\repo"),
            |_workflow, _step, _context| {
                Ok(StepExecution {
                    status_code: Some(17),
                    success: false,
                    stdout: "sortie".to_string(),
                    stderr: "erreur".to_string(),
                    command_outcome: None,
                })
            },
        )
        .expect_err("l'etape doit echouer");

        assert!(
            error
                .to_string()
                .contains("l'etape automate 'audit' a echoue avec le statut 17")
        );
        assert!(error.to_string().contains("stdout:\nsortie"));
        assert!(error.to_string().contains("stderr:\nerreur"));
    }

    #[test]
    fn run_workflow_uses_structured_clean_status_for_loop_stop() {
        let workflow = parse_workflow(
            r#"{
              "steps":[
                {"name":"alignment_audit","rust_command":["implementation-audit","plan.md"]},
                {"name":"commit","rust_command":["--mode","exec","Commit"]}
              ],
              "loop_policy":{"audit_step":"alignment_audit","max_cycles":3}
            }"#,
        )
        .expect("workflow valide");

        let calls = Rc::new(RefCell::new(0_u32));
        let seen = Rc::clone(&calls);

        let report = run_workflow_with_executor(
            &workflow,
            "Durcir",
            Path::new("C:\\repo"),
            move |_workflow, step, _context| {
                *seen.borrow_mut() += 1;
                Ok(StepExecution {
                    status_code: Some(0),
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                    command_outcome: Some(CommandOutcome {
                        command_name: step.name.clone(),
                        status_code: Some(0),
                        final_message_present: true,
                        artifact_path: Some(PathBuf::from("C:\\repo\\.audit\\artifact.md")),
                        clean: (step.name == "alignment_audit").then_some(true),
                    }),
                })
            },
        )
        .expect("workflow execute");

        assert!(report.clean_stop);
        assert_eq!(report.completed_cycles, 1);
        assert_eq!(*calls.borrow(), 2);
    }
}
