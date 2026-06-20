use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::codex::{self, CodexRequest};

pub const COMMAND_OUTCOME_PATH_ENV: &str = "RUST_AGENT_COMMAND_OUTCOME_PATH";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandOutcome {
    // Transport payload emitted by service commands for automation.
    //
    // Reporting owns normalization from Codex/report failures into these raw
    // completion facts. Workflow automation consumes the payload later,
    // validates workspace-local artifact paths, and interprets `clean` for
    // loop stopping without re-deriving reporting semantics.
    pub command_name: String,
    pub status_code: Option<i32>,
    pub final_message_present: bool,
    pub artifact_path: Option<PathBuf>,
    pub clean: Option<bool>,
}

// Service commands persist a structured outcome to this optional env-controlled
// file so workflow automation can consume status, artifact, and clean-loop
// state without scraping markdown output.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureOutcome {
    pub status_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub command_outcome: Option<CommandOutcome>,
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
            status_code: result.status.code().unwrap_or(1),
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
                status_code: result.status.code().unwrap_or(1),
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

fn build_command_outcome(
    spec: &ReportSpec<'_>,
    result: &codex::RunResult,
    final_message_present: bool,
    artifact_path: Option<PathBuf>,
    clean: Option<bool>,
) -> CommandOutcome {
    CommandOutcome {
        command_name: spec.command_name.to_string(),
        status_code: result.status.code(),
        final_message_present,
        artifact_path,
        clean,
    }
}

pub fn write_command_outcome_if_requested(outcome: &CommandOutcome) -> io::Result<()> {
    let Some(path) = std::env::var_os(COMMAND_OUTCOME_PATH_ENV).map(PathBuf::from) else {
        return Ok(());
    };

    write_command_outcome(&path, outcome)
}

fn write_command_outcome(path: &Path, outcome: &CommandOutcome) -> io::Result<()> {
    let content = serde_json::to_vec(outcome)
        .map_err(|error| io::Error::other(format!("serialisation du resultat: {error}")))?;
    fs::write(path, content)
}

pub fn detect_clean_implementation_audit(content: &str) -> bool {
    content
        .to_ascii_lowercase()
        .contains("no actionable deviations found")
}

fn print_failure_details(stdout: &str, stderr: &str) {
    let stderr = stderr.trim();
    let stdout = stdout.trim();

    if !stderr.is_empty() {
        eprintln!("{stderr}");
        return;
    }

    if !stdout.is_empty() {
        eprintln!("{stdout}");
    }
}

pub fn command_failure_exit_code(error: &ReportFailure) -> i32 {
    match error {
        ReportFailure::MissingFinalMessage { status_code, .. } => {
            crate::codex::process_exit_code(Some(*status_code))
        }
        _ => 1,
    }
}

pub fn command_failure_outcome(command_name: &str, error: &ReportFailure) -> FailureOutcome {
    match error {
        ReportFailure::CodexCall(error) => FailureOutcome {
            status_code: 1,
            stdout: String::new(),
            stderr: error.clone(),
            command_outcome: None,
        },
        ReportFailure::MissingFinalMessage {
            status_code,
            stdout,
            stderr,
        } => FailureOutcome {
            status_code: *status_code,
            stdout: stdout.clone(),
            stderr: stderr.clone(),
            command_outcome: Some(CommandOutcome {
                command_name: command_name.to_string(),
                status_code: Some(*status_code),
                final_message_present: false,
                artifact_path: None,
                clean: None,
            }),
        },
        ReportFailure::Save { clean, error, .. } => FailureOutcome {
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

pub fn print_completed_report(report: &CompletedReport, spec: &ReportSpec<'_>) {
    if report.status_code != 0 {
        eprintln!(
            "codex a produit un {} mais s'est termine avec le statut {}. Le {} est conserve.",
            spec.final_label, report.status_code, spec.saved_label
        );
        print_failure_details(&report.stdout, &report.stderr);
    }

    println!(
        "{} enregistre dans {}",
        capitalize(spec.saved_label),
        report.saved_path.display()
    );
    println!();
    println!("{}", report.message);
}

pub fn print_report_failure(failure: &ReportFailure, spec: &ReportSpec<'_>) {
    eprintln!("{}", render_report_failure(failure, spec));
}

pub fn render_report_failure(failure: &ReportFailure, spec: &ReportSpec<'_>) -> String {
    match failure {
        ReportFailure::CodexCall(error) => format!("echec lors de l'appel a codex: {error}"),
        ReportFailure::MissingFinalMessage {
            status_code,
            stdout,
            stderr,
        } => {
            if *status_code != 0 {
                let stderr = stderr.trim();
                let stdout = stdout.trim();
                if !stderr.is_empty() {
                    stderr.to_string()
                } else if !stdout.is_empty() {
                    stdout.to_string()
                } else {
                    String::new()
                }
            } else {
                format!(
                    "codex n'a pas retourne de message final pour {}",
                    spec.missing_message_label
                )
            }
        }
        ReportFailure::Save { error, .. } => {
            format!(
                "echec lors de l'enregistrement du {}: {error}",
                spec.saved_label
            )
        }
    }
}

fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::RunResult;
    use std::os::windows::process::ExitStatusExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn save_test_report(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
        crate::artifact::save_timestamped_markdown(output_dir, "implementation-audit", content)
    }

    fn report_spec<'a>(output_dir: &'a Path) -> ReportSpec<'a> {
        ReportSpec {
            command_name: "implementation-audit",
            saved_label: "audit d'implementation",
            final_label: "audit d'implementation",
            missing_message_label: "audit d'implementation",
            output_dir,
            save: save_test_report,
            clean_detector: Some(detect_clean_implementation_audit),
        }
    }

    fn finalize_test_report(
        result: RunResult,
        spec: &ReportSpec<'_>,
    ) -> Result<CompletedReport, ReportFailure> {
        finalize_report(result, spec)
    }

    fn temp_test_dir(prefix: &str) -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("rust_agent_{prefix}_{id}"))
    }

    #[test]
    fn detects_clean_implementation_audit_summary_line() {
        assert!(detect_clean_implementation_audit(
            "# Rust Implementation Plan Audit\n\n## Summary\n- No actionable deviations found.\n"
        ));
        assert!(!detect_clean_implementation_audit(
            "# Rust Implementation Plan Audit\n\n## Summary\n- 2 actionable deviations found.\n"
        ));
    }

    #[test]
    fn writes_command_outcome_to_requested_path() {
        let output_path = temp_test_dir("outcome").join("outcome.json");
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).expect("creation du dossier");
        }

        let outcome = CommandOutcome {
            command_name: "audit".to_string(),
            status_code: Some(0),
            final_message_present: true,
            artifact_path: Some(PathBuf::from("C:\\tmp\\audit.md")),
            clean: Some(true),
        };

        write_command_outcome(&output_path, &outcome).expect("ecriture du resultat");

        let saved = fs::read(&output_path).expect("lecture du resultat");
        let decoded: CommandOutcome = serde_json::from_slice(&saved).expect("decodage JSON");
        assert_eq!(decoded, outcome);
        let _ = fs::remove_dir_all(output_path.parent().expect("dossier parent"));
    }

    #[test]
    fn finalize_report_persists_message_and_marks_clean() {
        let output_dir = temp_test_dir("report_success");
        let report = finalize_test_report(
            RunResult {
                status: std::process::ExitStatus::from_raw(0),
                final_message: Some(
                    "# Rust Implementation Plan Audit\n\n## Deviations\nNo actionable deviations found.\n"
                        .to_string(),
                ),
                stdout: String::new(),
                stderr: String::new(),
            },
            &report_spec(&output_dir),
        )
        .expect("rapport finalise");

        assert_eq!(report.status_code, 0);
        assert_eq!(report.outcome.clean, Some(true));
        assert_eq!(
            report.outcome.artifact_path.as_ref(),
            Some(&report.saved_path)
        );
        assert!(fs::metadata(&report.saved_path).is_ok());

        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn finalize_report_preserves_non_zero_status_with_artifact() {
        let output_dir = temp_test_dir("report_non_zero");
        let report = finalize_test_report(
            RunResult {
                status: std::process::ExitStatus::from_raw(7),
                final_message: Some("rapport".to_string()),
                stdout: "stdout".to_string(),
                stderr: "stderr".to_string(),
            },
            &report_spec(&output_dir),
        )
        .expect("rapport finalise");

        assert_eq!(report.status_code, 7);
        assert_eq!(report.outcome.status_code, Some(7));
        assert!(fs::metadata(&report.saved_path).is_ok());

        let _ = fs::remove_dir_all(output_dir);
    }

    #[test]
    fn finalize_report_reports_missing_final_message() {
        let error = finalize_test_report(
            RunResult {
                status: std::process::ExitStatus::from_raw(0),
                final_message: None,
                stdout: String::new(),
                stderr: String::new(),
            },
            &report_spec(Path::new("C:\\tmp")),
        )
        .expect_err("absence de message final doit echouer");

        assert_eq!(
            error,
            ReportFailure::MissingFinalMessage {
                status_code: 0,
                stdout: String::new(),
                stderr: String::new(),
            }
        );
    }

    #[test]
    fn finalize_report_reports_save_failure_and_preserves_outcome_shape() {
        let file_path = temp_test_dir("report_save_failure");
        fs::write(&file_path, "occupied").expect("creation du fichier");

        let error = finalize_test_report(
            RunResult {
                status: std::process::ExitStatus::from_raw(0),
                final_message: Some("content".to_string()),
                stdout: String::new(),
                stderr: String::new(),
            },
            &report_spec(&file_path),
        )
        .expect_err("un chemin fichier doit faire echouer la sauvegarde");

        match error {
            ReportFailure::Save {
                message,
                clean,
                error,
            } => {
                assert_eq!(message, "content");
                assert_eq!(clean, Some(false));
                assert!(!error.is_empty());
            }
            other => panic!("erreur inattendue: {other:?}"),
        }

        let _ = fs::remove_file(file_path);
    }

    #[test]
    fn command_failure_outcome_preserves_missing_final_message_state() {
        let failure = ReportFailure::MissingFinalMessage {
            status_code: 7,
            stdout: "stdout".to_string(),
            stderr: "stderr".to_string(),
        };

        let outcome = command_failure_outcome("audit", &failure);

        assert_eq!(outcome.status_code, 7);
        assert_eq!(outcome.stdout, "stdout");
        assert_eq!(outcome.stderr, "stderr");
        assert_eq!(
            outcome.command_outcome,
            Some(CommandOutcome {
                command_name: "audit".to_string(),
                status_code: Some(7),
                final_message_present: false,
                artifact_path: None,
                clean: None,
            })
        );
    }

    #[test]
    fn command_failure_outcome_preserves_save_failure_clean_state() {
        let failure = ReportFailure::Save {
            message: "rapport".to_string(),
            clean: Some(true),
            error: "disk full".to_string(),
        };

        let outcome = command_failure_outcome("implementation-audit", &failure);

        assert_eq!(outcome.status_code, 1);
        assert_eq!(outcome.stdout, "");
        assert_eq!(outcome.stderr, "disk full");
        assert_eq!(
            outcome.command_outcome,
            Some(CommandOutcome {
                command_name: "implementation-audit".to_string(),
                status_code: Some(1),
                final_message_present: true,
                artifact_path: None,
                clean: Some(true),
            })
        );
    }
}
