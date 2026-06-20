use crate::codex::{DEFAULT_MODEL, DEFAULT_REASONING_EFFORT};
use crate::command_registry;

use super::workflow_model::{Workflow, WorkflowStep};
use super::workflow_runner::RunContext;

pub fn resolve_step_args(
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
