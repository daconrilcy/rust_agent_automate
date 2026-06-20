use app::automate::{WorkflowStepKind, default_refactor_workflow, parse_workflow};

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

    assert!(error.to_string().contains("doit etre unique"));
}

#[test]
fn rejects_empty_workflow() {
    let error = parse_workflow(r#"{"steps":[]}"#).expect_err("workflow vide invalide");
    assert_eq!(
        error.to_string(),
        "le workflow doit definir au moins une etape"
    );
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
fn rejects_unknown_artifact_reference_during_validation() {
    let error = parse_workflow(
        r#"{
          "steps":[{"name":"plan","rust_command":["plan","{artifact:audit}"]}]
        }"#,
    )
    .expect_err("les references d'artefact inconnues doivent etre rejetees");

    assert!(error.to_string().contains("artefact inconnu"));
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

    assert!(error.to_string().contains("artefact futur"));
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

    assert!(error.to_string().contains("execution directe"));
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

    assert!(error.to_string().contains("commande structuree"));
}
