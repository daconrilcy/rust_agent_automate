use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub const COMMAND_OUTCOME_PATH_ENV: &str = "RUST_AGENT_COMMAND_OUTCOME_PATH";
pub const COMMAND_OUTCOME_SCHEMA_VERSION: u16 = 1;

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

#[derive(Debug, Serialize)]
struct CommandOutcomeEnvelope<'a> {
    schema_version: u16,
    outcome: &'a CommandOutcome,
}

pub fn write_command_outcome_if_requested(outcome: &CommandOutcome) -> io::Result<()> {
    let Some(path) = std::env::var_os(COMMAND_OUTCOME_PATH_ENV).map(PathBuf::from) else {
        return Ok(());
    };

    write_command_outcome(&path, outcome)
}

pub fn write_command_outcome(path: &Path, outcome: &CommandOutcome) -> io::Result<()> {
    let envelope = CommandOutcomeEnvelope {
        schema_version: COMMAND_OUTCOME_SCHEMA_VERSION,
        outcome,
    };
    let content = serde_json::to_vec(&envelope)
        .map_err(|error| io::Error::other(format!("serialisation du resultat: {error}")))?;
    write_atomic(path, &content)
}

fn write_atomic(path: &Path, content: &[u8]) -> io::Result<()> {
    let temp_path = temp_sibling_path(path);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp_path)?;

    if let Err(error) = file.write_all(content).and_then(|_| file.sync_all()) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }

    drop(file);

    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }

    Ok(())
}

fn temp_sibling_path(path: &Path) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("command-outcome.json");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    parent.join(format!(
        ".{file_name}.tmp.{}.{}",
        std::process::id(),
        timestamp
    ))
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
