use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const COMMAND_OUTCOME_PATH_ENV: &str = "RUST_AGENT_COMMAND_OUTCOME_PATH";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandOutcome {
    pub command_name: String,
    pub status_code: Option<i32>,
    pub final_message_present: bool,
    pub artifact_path: Option<PathBuf>,
    pub clean: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureOutcome {
    pub status_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub command_outcome: Option<CommandOutcome>,
}

pub fn write_command_outcome_if_requested(outcome: &CommandOutcome) -> io::Result<()> {
    let Some(path) = std::env::var_os(COMMAND_OUTCOME_PATH_ENV).map(PathBuf::from) else {
        return Ok(());
    };

    write_command_outcome(&path, outcome)
}

pub fn write_command_outcome(path: &Path, outcome: &CommandOutcome) -> io::Result<()> {
    let content = serde_json::to_vec(outcome)
        .map_err(|error| io::Error::other(format!("serialisation du resultat: {error}")))?;
    fs::write(path, content)
}

pub fn command_failure_outcome(
    command_name: &str,
    error: &crate::reporting::ReportFailure,
) -> FailureOutcome {
    match error {
        crate::reporting::ReportFailure::CodexCall(error) => FailureOutcome {
            status_code: 1,
            stdout: String::new(),
            stderr: error.clone(),
            command_outcome: None,
        },
        crate::reporting::ReportFailure::MissingFinalMessage {
            status_code,
            stdout,
            stderr,
        } => FailureOutcome {
            status_code: crate::codex::process_exit_code(Some(*status_code)),
            stdout: stdout.clone(),
            stderr: stderr.clone(),
            command_outcome: Some(CommandOutcome {
                command_name: command_name.to_string(),
                status_code: Some(crate::codex::process_exit_code(Some(*status_code))),
                final_message_present: false,
                artifact_path: None,
                clean: None,
            }),
        },
        crate::reporting::ReportFailure::Save { clean, error, .. } => FailureOutcome {
            status_code: 1,
            stdout: String::new(),
            stderr: error.clone(),
            command_outcome: Some(CommandOutcome {
                command_name: command_name.to_string(),
                status_code: Some(1),
                final_message_present: true,
                artifact_path: None,
                clean: *clean,
            }),
        },
    }
}
