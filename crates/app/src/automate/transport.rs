use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::codex;
use crate::reporting::CommandOutcome;

use super::step_outcome::StepExecution;
use super::workflow_model::{Workflow, WorkflowStep, WorkflowStepKind};
use super::workflow_runner::RunContext;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkflowStepOutcome {
    pub(crate) status_code: Option<i32>,
    pub(crate) artifact_path: Option<PathBuf>,
    pub(crate) clean: Option<bool>,
}

pub(crate) fn command_outcome_for_step(
    _workflow: &Workflow,
    step: &WorkflowStep,
    _context: &RunContext,
    output: &StepExecution,
) -> io::Result<WorkflowStepOutcome> {
    if let Some(outcome) = &output.command_outcome {
        let mut outcome = outcome.clone();
        outcome.status_code = normalized_status_code(outcome.status_code);
        return Ok(WorkflowStepOutcome {
            status_code: outcome.status_code,
            artifact_path: outcome.artifact_path,
            clean: outcome.clean,
        });
    }

    if matches!(step.kind, WorkflowStepKind::DirectRun) {
        return Ok(WorkflowStepOutcome {
            status_code: normalized_status_code(output.status_code),
            artifact_path: None,
            clean: None,
        });
    }

    Err(io::Error::other(format!(
        "l'etape automate '{}' doit produire un resultat structure",
        step.name
    )))
}

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

    use super::*;
    use crate::automate::workflow_model::parse_workflow;

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
