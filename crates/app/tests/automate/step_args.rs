use std::path::PathBuf;

use app::automate::step_args::resolve_step_args;
use app::automate::workflow_model::parse_workflow;
use app::automate::workflow_runner::RunContext;
use app::{AgentContext, DEFAULT_MODEL, WorkflowStepKind};

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
fn injects_agent_context_extensions_for_service_commands() {
    let workflow = parse_workflow(
        r#"{
          "steps":[{"name":"audit","rust_command":["audit","--target","{target}"]}]
        }"#,
    )
    .expect("workflow valide");
    let mut agent_context = AgentContext::default();
    agent_context.enable_team();
    agent_context.enable_portability();
    agent_context.enable_docker();
    let context = RunContext {
        target_dir: PathBuf::from("C:\\repo"),
        agent_context,
        ..RunContext::default()
    };

    let args = resolve_step_args(&workflow, &workflow.steps[0], &context);

    assert!(args.contains(&"--team".to_string()));
    assert!(args.contains(&"--portable".to_string()));
    assert!(args.contains(&"--docker".to_string()));
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
