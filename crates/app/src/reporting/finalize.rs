use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::codex::{self, CodexRequest};

use super::transport::{CommandOutcome, write_command_outcome_if_requested};

pub struct ReportSpec<'a> {
    pub command_name: &'a str,
    pub saved_label: &'a str,
    pub final_label: &'a str,
    pub missing_message_label: &'a str,
    pub output_dir: &'a Path,
    pub save: fn(&Path, &str) -> io::Result<PathBuf>,
    pub clean_detector: Option<fn(&str) -> bool>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReportFailure {
    CodexCall(String),
    MissingFinalMessage {
        status_code: i32,
        stdout: String,
        stderr: String,
    },
    Save {
        message: String,
        clean: Option<bool>,
        error: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompletedReport {
    pub status_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub message: String,
    pub saved_path: PathBuf,
    pub outcome: CommandOutcome,
}

pub fn run_codex_report(
    request: &CodexRequest,
    timeout: Duration,
    spec: ReportSpec<'_>,
) -> Result<CompletedReport, ReportFailure> {
    let result = codex::run_until_final_message(request, timeout)
        .map_err(|error| ReportFailure::CodexCall(error.to_string()))?;

    finalize_report(result, &spec)
}

pub fn finalize_report(
    result: codex::RunResult,
    spec: &ReportSpec<'_>,
) -> Result<CompletedReport, ReportFailure> {
    let Some(message) = result.final_message.as_ref() else {
        let outcome = build_command_outcome(spec, &result, false, None, None);
        let _ = write_command_outcome_if_requested(&outcome);

        return Err(ReportFailure::MissingFinalMessage {
            status_code: run_result_status_code(&result),
            stdout: result.stdout,
            stderr: result.stderr,
        });
    };

    let clean = spec.clean_detector.map(|detector| detector(message));
    match (spec.save)(spec.output_dir, message) {
        Ok(path) => {
            let outcome = build_command_outcome(spec, &result, true, Some(path.clone()), clean);
            let _ = write_command_outcome_if_requested(&outcome);

            Ok(CompletedReport {
                status_code: run_result_status_code(&result),
                stdout: result.stdout,
                stderr: result.stderr,
                message: message.to_string(),
                saved_path: path,
                outcome,
            })
        }
        Err(error) => {
            let outcome = build_command_outcome(spec, &result, true, None, clean);
            let _ = write_command_outcome_if_requested(&outcome);

            Err(ReportFailure::Save {
                message: message.to_string(),
                clean,
                error: error.to_string(),
            })
        }
    }
}

pub fn detect_clean_implementation_audit(content: &str) -> bool {
    content
        .to_ascii_lowercase()
        .contains("no actionable deviations found")
}

fn build_command_outcome(
    spec: &ReportSpec<'_>,
    result: &codex::RunResult,
    final_message_present: bool,
    artifact_path: Option<PathBuf>,
    clean: Option<bool>,
) -> CommandOutcome {
    CommandOutcome {
        command_name: spec.command_name.to_string(),
        status_code: Some(run_result_status_code(result)),
        final_message_present,
        artifact_path,
        clean,
    }
}

fn run_result_status_code(result: &codex::RunResult) -> i32 {
    codex::process_exit_code(result.status.code())
}
