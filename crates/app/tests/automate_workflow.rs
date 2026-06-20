use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use app::automate::step_args::resolve_step_args;
use app::automate::step_outcome::{StepExecution, command_outcome_for_step};
use app::automate::workflow_model::{
    Workflow, WorkflowStepKind, default_refactor_workflow, parse_workflow,
};
use app::automate::workflow_runner::{RunContext, run_workflow_with_executor};
use app::codex::DEFAULT_MODEL;
use app::reporting::CommandOutcome;

#[test]
fn parses_default_refactor_workflow() {
    let workflow = default_refactor_workflow();

    assert_eq!(workflow.steps.len(), 7);
    assert_eq!(workflow.steps[0].name, "audit");
    assert_eq!(workflow.steps[1].name, "plan");
    assert_eq!(workflow.steps[2].name, "implementation");
    assert_eq!(workflow.steps[3].name, "dev_review_corrections");
    assert_eq!(workflow.steps[0].kind, WorkflowStepKind::ServiceCommand);
    assert_eq!(workflow.steps[2].kind, WorkflowStepKind::DirectRun);
    assert_eq!(workflow.steps[3].kind, WorkflowStepKind::ServiceCommand);
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
fn rejects_duplicate_step_names() {
    let error = parse_workflow(
        r#"{
          "steps":[
            {"name":"audit","rust_command":["audit","--target","{target}"]},
            {"name":"audit","rust_command":["plan","audit.md"]}
          ]
        }"#,
    )
    .expect_err("les noms d'etapes doivent etre uniques");

    assert!(error.contains("doit etre unique"));
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

    let workflow = parse_workflow(
        r#"{
          "steps":[
            {"name":"plan","rust_command":["plan","audit.md"]},
            {"name":"audit","rust_command":["audit","cycle={cycle}; target={target}; plan={artifact:plan}; prompt={initial_prompt}"]}
          ]
        }"#
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
            "42"
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
fn keeps_nested_subcommand_args_unchanged() {
    let workflow = parse_workflow(
        r#"{
          "defaults": {"model":"gpt-x","reasoning":"high"},
          "steps":[{"name":"nested","rust_command":["automate","workflow.json","Initial prompt"],"fresh_codex_call":false}]
        }"#,
    )
    .expect("workflow valide");
    let context = RunContext::default();

    assert_eq!(workflow.steps[0].kind, WorkflowStepKind::NestedCommand);
    let args = resolve_step_args(&workflow, &workflow.steps[0], &context);

    assert_eq!(args, vec!["automate", "workflow.json", "Initial prompt"]);
}

#[test]
fn rejects_unknown_artifact_reference_during_validation() {
    let error = parse_workflow(
        r#"{
          "steps":[{"name":"plan","rust_command":["plan","{artifact:audit}"]}]
        }"#,
    )
    .expect_err("les references d'artefact inconnues doivent etre rejetees");

    assert!(error.contains("artefact inconnu"));
}

#[test]
fn rejects_future_artifact_reference_during_validation() {
    let error = parse_workflow(
        r#"{
          "steps":[
            {"name":"plan","rust_command":["plan","{artifact:audit}"]},
            {"name":"audit","rust_command":["audit","--target","{target}"]}
          ]
        }"#,
    )
    .expect_err("les references futures doivent etre rejetees");

    assert!(error.contains("artefact futur"));
}

#[test]
fn rejects_artifact_reference_to_direct_run_step() {
    let error = parse_workflow(
        r#"{
          "steps":[
            {"name":"implementation","rust_command":["--mode","exec","Implement"]},
            {"name":"audit","rust_command":["implementation-audit","{artifact:implementation}"]}
          ]
        }"#,
    )
    .expect_err("une etape directe ne produit pas d'artefact structure");

    assert!(error.contains("execution directe"));
}

#[test]
fn command_outcome_accepts_direct_run_when_prompt_starts_with_known_subcommand() {
    let workflow = parse_workflow(
        r#"{
          "defaults": {"model":"gpt-x","reasoning":"medium"},
          "steps":[{"name":"implementation","rust_command":["audit this repo"],"fresh_codex_call":true}]
        }"#,
    )
    .expect("workflow valide");
    let step = &workflow.steps[0];
    let output = StepExecution {
        status_code: Some(0),
        success: true,
        stdout: "implementation terminee".to_string(),
        stderr: String::new(),
        command_outcome: None,
    };

    let outcome = command_outcome_for_step(&workflow, step, &RunContext::default(), &output)
        .expect("une etape directe doit etre acceptee");

    assert_eq!(outcome.command_name, "implementation");
    assert_eq!(outcome.status_code, Some(0));
    assert!(outcome.final_message_present);
    assert_eq!(outcome.artifact_path, None);
}

#[test]
fn command_outcome_requires_structured_result_for_nested_subcommands() {
    let workflow = parse_workflow(
        r#"{
          "steps":[{"name":"nested","rust_command":["automate","workflow.json","Prompt"]}]
        }"#,
    )
    .expect("workflow valide");
    let step = &workflow.steps[0];
    let output = StepExecution {
        status_code: Some(0),
        success: true,
        stdout: "automate termine".to_string(),
        stderr: String::new(),
        command_outcome: None,
    };

    let error = command_outcome_for_step(&workflow, step, &RunContext::default(), &output)
        .expect_err("une sous-commande imbriquee doit rester structuree");

    assert!(
        error
            .to_string()
            .contains("doit produire un resultat structure")
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
    let workspace = std::env::temp_dir().join("rust_agent_workflow_chain_artifacts");
    let audit_artifact = workspace.join(".audit").join("audit.md");
    let plan_artifact = workspace.join(".plan").join("plan.md");
    fs::create_dir_all(audit_artifact.parent().expect("parent audit")).expect("dossier audit");
    fs::create_dir_all(plan_artifact.parent().expect("parent plan")).expect("dossier plan");
    fs::write(&audit_artifact, "audit").expect("artefact audit");
    fs::write(&plan_artifact, "plan").expect("artefact plan");
    let audit_artifact_for_step = audit_artifact.clone();
    let plan_artifact_for_step = plan_artifact.clone();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&calls);

    let report = run_workflow_with_executor(
        &workflow,
        "Durcir",
        &workspace,
        &workspace,
        move |workflow, step, context| {
            seen.borrow_mut()
                .push(resolve_step_args(workflow, step, context));

            let artifact_path = match step.name.as_str() {
                "audit" => Some(audit_artifact_for_step.clone()),
                "plan" => Some(plan_artifact_for_step.clone()),
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
            workspace.to_str().expect("workspace utf-8"),
            "--model",
            DEFAULT_MODEL,
            "--reasoning",
            "low",
            "--timeout-seconds",
            "1800"
        ]
    );
    assert_eq!(calls[1][0], "plan");
    assert_eq!(
        calls[1][1],
        fs::canonicalize(&audit_artifact)
            .expect("artefact canonical")
            .display()
            .to_string()
    );
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn run_workflow_rejects_missing_structured_outcome() {
    let workflow = parse_workflow(
        r#"{"steps":[{"name":"audit","rust_command":["audit","--target","{target}"]}]}"#,
    )
    .expect("workflow valide");

    let error = run_workflow_with_executor(
        &workflow,
        "Durcir",
        Path::new("C:\\repo"),
        Path::new("C:\\repo"),
        |_workflow, _step, _context| {
            Ok(StepExecution {
                status_code: Some(0),
                success: true,
                stdout: "Plan enregistre dans C:\\repo\\.audit\\audit.md".to_string(),
                stderr: String::new(),
                command_outcome: None,
            })
        },
    )
    .expect_err("un resultat structure est requis");

    assert!(
        error
            .to_string()
            .contains("doit produire un resultat structure")
    );
}

#[test]
fn run_workflow_accepts_direct_run_without_structured_outcome() {
    let workflow = parse_workflow(
        r#"{"steps":[{"name":"implementation","rust_command":["--mode","exec","Implement"]}]}"#,
    )
    .expect("workflow valide");

    let report = run_workflow_with_executor(
        &workflow,
        "Durcir",
        Path::new("C:\\repo"),
        Path::new("C:\\repo"),
        |_workflow, _step, _context| {
            Ok(StepExecution {
                status_code: Some(0),
                success: true,
                stdout: "implementation terminee".to_string(),
                stderr: String::new(),
                command_outcome: None,
            })
        },
    )
    .expect("une etape directe n'a pas d'artefact structure");

    assert_eq!(report.completed_cycles, 1);
    assert_eq!(report.step_results[0].status_code, Some(0));
    assert_eq!(report.step_results[0].artifact_path, None);
}

#[test]
fn run_workflow_uses_structured_clean_status_for_loop_stop() {
    let workflow: Workflow = parse_workflow(
        r#"{
          "steps":[
            {"name":"alignment_audit","rust_command":["implementation-audit","plan.md"]},
            {"name":"commit","rust_command":["--mode","exec","Commit"]}
          ],
          "loop_policy":{"audit_step":"alignment_audit","max_cycles":3}
        }"#,
    )
    .expect("workflow valide");

    let workspace = std::env::temp_dir().join("rust_agent_workflow_loop_stop");
    let artifact = workspace.join(".audit").join("artifact.md");
    fs::create_dir_all(artifact.parent().expect("parent")).expect("dossier audit");
    fs::write(&artifact, "artifact").expect("artefact audit");
    let calls = Rc::new(RefCell::new(0_u32));
    let seen = Rc::clone(&calls);

    let report = run_workflow_with_executor(
        &workflow,
        "Durcir",
        &workspace,
        &workspace,
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
                    artifact_path: Some(artifact.clone()),
                    clean: (step.name == "alignment_audit").then_some(true),
                }),
            })
        },
    )
    .expect("workflow execute");

    assert!(report.clean_stop);
    assert_eq!(report.completed_cycles, 1);
    assert_eq!(*calls.borrow(), 2);
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn run_workflow_rejects_loop_audit_without_structured_clean_status() {
    let workflow: Workflow = parse_workflow(
        r#"{
          "steps":[
            {"name":"alignment_audit","rust_command":["implementation-audit","plan.md"]}
          ],
          "loop_policy":{"audit_step":"alignment_audit","max_cycles":2}
        }"#,
    )
    .expect("workflow valide");

    let workspace = std::env::temp_dir().join("rust_agent_workflow_missing_clean");
    let artifact = workspace.join(".audit").join("artifact.md");
    fs::create_dir_all(artifact.parent().expect("parent")).expect("dossier audit");
    fs::write(&artifact, "artifact").expect("artefact audit");
    let error = run_workflow_with_executor(
        &workflow,
        "Durcir",
        &workspace,
        &workspace,
        move |_workflow, step, _context| {
            Ok(StepExecution {
                status_code: Some(0),
                success: true,
                stdout: String::new(),
                stderr: String::new(),
                command_outcome: Some(CommandOutcome {
                    command_name: step.name.clone(),
                    status_code: Some(0),
                    final_message_present: true,
                    artifact_path: Some(artifact.clone()),
                    clean: None,
                }),
            })
        },
    )
    .expect_err("loop_policy doit exiger un statut clean structure");

    assert!(error.to_string().contains("statut clean"));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn rejects_loop_policy_on_direct_run_step_during_validation() {
    let error = parse_workflow(
        r#"{
          "steps":[
            {"name":"alignment_audit","rust_command":["--mode","exec","Audit"]}]
          ,
          "loop_policy":{"audit_step":"alignment_audit","max_cycles":2}
        }"#,
    )
    .expect_err("loop_policy doit pointer vers une commande structuree");

    assert!(error.contains("commande structuree"));
}
