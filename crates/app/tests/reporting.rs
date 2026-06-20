use app::codex::RunResult;
use app::reporting::{
    CommandOutcome, ReportFailure, ReportSpec, command_failure_outcome,
    detect_clean_implementation_audit, finalize_report, write_command_outcome,
};
use std::fs;
use std::io;
use std::os::windows::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

fn save_test_report(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    let path = output_dir.join("implementation-audit.md");
    fs::create_dir_all(output_dir)?;
    fs::write(&path, content)?;
    Ok(path)
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
    let report = finalize_report(
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
fn finalize_report_reports_missing_final_message() {
    let error = finalize_report(
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
