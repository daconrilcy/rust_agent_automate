use crate::codex::{DEFAULT_MODEL, DEFAULT_REASONING_EFFORT};

use super::workflow_model::{Workflow, WorkflowStep, WorkflowStepKind};
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

    if matches!(step.kind, WorkflowStepKind::ServiceCommand) {
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
    } else if matches!(step.kind, WorkflowStepKind::DirectRun) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automate::{WorkflowStepKind, parse_workflow};
    use crate::codex::DEFAULT_MODEL;
    use std::path::PathBuf;

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

        let workflow = parse_workflow(
            r#"{
              "steps":[
                {"name":"plan","rust_command":["plan","audit.md"]},
                {"name":"audit","rust_command":["audit","cycle={cycle}; target={target}; plan={artifact:plan}; prompt={initial_prompt}"]}
              ]
            }"#,
        )
        .expect("workflow valide");

        let args = resolve_step_args(&workflow, &workflow.steps[1], &context);

        assert!(args[1].contains("cycle=2"));
        assert!(args[1].contains("C:\\dev\\rust_agent"));
        assert!(args[1].contains(".plan\\plan.md"));
        assert!(args[1].contains("Durcir le code"));
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
                "42",
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
                "Commit",
            ]
        );
    }

    #[test]
    fn keeps_nested_subcommand_args_unchanged() {
        let workflow = parse_workflow(
            r#"{
              "defaults": {"model":"gpt-x","reasoning":"high"},
              "steps":[{"name":"nested","rust_command":["automate","workflow.json","Initial prompt"],"fresh_codex_call":false}]
            }"#,
        )
        .expect("workflow valide");
        let context = RunContext::default();

        let args = resolve_step_args(&workflow, &workflow.steps[0], &context);

        assert_eq!(workflow.steps[0].kind, WorkflowStepKind::NestedCommand);
        assert_eq!(args, vec!["automate", "workflow.json", "Initial prompt"]);
    }

    #[test]
    fn run_workflow_chains_default_model_when_unspecified() {
        let workflow = parse_workflow(
            r#"{
              "steps":[{"name":"audit","rust_command":["audit","--target","{target}"]}]
            }"#,
        )
        .expect("workflow valide");
        let context = RunContext {
            target_dir: PathBuf::from("C:\\repo"),
            ..RunContext::default()
        };

        let args = resolve_step_args(&workflow, &workflow.steps[0], &context);

        assert!(args.contains(&DEFAULT_MODEL.to_string()));
    }
}
