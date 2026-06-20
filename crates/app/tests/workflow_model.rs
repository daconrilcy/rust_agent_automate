use app::{WorkflowStepKind, default_refactor_workflow, parse_workflow};

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

    assert!(error.contains("doit etre unique"));
}
