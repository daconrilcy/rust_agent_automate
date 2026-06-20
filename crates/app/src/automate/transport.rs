use std::fs;
use std::io;
use std::path::Path;

use crate::codex;
use crate::reporting::CommandOutcome;

pub(crate) fn decode_command_outcome(path: &Path) -> io::Result<Option<CommandOutcome>> {
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

pub(crate) fn normalized_status_code(status_code: Option<i32>) -> Option<i32> {
    status_code.map(|code| codex::process_exit_code(Some(code)))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::*;
    use crate::automate::step_outcome::{StepExecution, command_outcome_for_step};
    use crate::automate::workflow_model::parse_workflow;
    use crate::automate::workflow_runner::RunContext;

    #[test]
    fn decode_command_outcome_rejects_invalid_json() {
        let path = std::env::temp_dir().join("rust_agent_invalid_command_outcome.json");
        fs::write(&path, b"{ invalid json").expect("ecriture du resultat invalide");

        let error = decode_command_outcome(&path).expect_err("le JSON invalide doit echouer");

        assert!(error.to_string().contains("resultat structure invalide"));
        let _ = fs::remove_file(&path);
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
