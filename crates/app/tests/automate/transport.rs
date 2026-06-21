use std::fs;
use std::path::PathBuf;

use app::CommandOutcome;
use app::automate::step_outcome::{StepExecution, command_outcome_for_step};
use app::automate::transport::decode_command_outcome;
use app::automate::workflow_model::parse_workflow;
use app::automate::workflow_runner::RunContext;

#[test]
fn decode_command_outcome_rejects_invalid_json() {
    let path = crate::support::temp_dir("invalid_command_outcome").join("outcome.json");
    fs::create_dir_all(path.parent().expect("parent")).expect("creation du dossier");
    fs::write(&path, b"{ invalid json").expect("ecriture du resultat invalide");

    let error = decode_command_outcome(&path).expect_err("le JSON invalide doit echouer");

    assert!(error.to_string().contains("resultat structure invalide"));
    let _ = fs::remove_file(&path);
    let _ = fs::remove_dir_all(path.parent().expect("parent"));
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
