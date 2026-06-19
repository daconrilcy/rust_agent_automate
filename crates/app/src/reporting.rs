use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::codex::{self, CodexRequest};

pub const COMMAND_OUTCOME_PATH_ENV: &str = "RUST_AGENT_COMMAND_OUTCOME_PATH";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandOutcome {
    pub command_name: String,
    pub status_code: Option<i32>,
    pub final_message_present: bool,
    pub artifact_path: Option<PathBuf>,
    pub clean: Option<bool>,
}

pub struct ReportSpec<'a> {
    pub command_name: &'a str,
    pub saved_label: &'a str,
    pub final_label: &'a str,
    pub missing_message_label: &'a str,
    pub output_dir: &'a Path,
    pub save: fn(&Path, &str) -> io::Result<PathBuf>,
    pub clean_detector: Option<fn(&str) -> bool>,
}

pub fn run_codex_report(request: &CodexRequest, timeout: Duration, spec: ReportSpec<'_>) {
    match codex::run_until_final_message(request, timeout) {
        Ok(result) => {
            let Some(message) = result.final_message else {
                let outcome = CommandOutcome {
                    command_name: spec.command_name.to_string(),
                    status_code: result.status.code(),
                    final_message_present: false,
                    artifact_path: None,
                    clean: None,
                };
                let _ = write_command_outcome_if_requested(&outcome);

                if !result.status.success() {
                    print_failure_details(&result.stdout, &result.stderr);
                    process::exit(result.status.code().unwrap_or(1));
                }

                eprintln!(
                    "codex n'a pas retourne de message final pour {}",
                    spec.missing_message_label
                );
                process::exit(1);
            };

            if !result.status.success() {
                eprintln!(
                    "codex a produit un {} mais s'est termine avec le statut {}. Le {} est conserve.",
                    spec.final_label, result.status, spec.saved_label
                );
                print_failure_details(&result.stdout, &result.stderr);
            }

            match (spec.save)(spec.output_dir, &message) {
                Ok(path) => {
                    let outcome = CommandOutcome {
                        command_name: spec.command_name.to_string(),
                        status_code: result.status.code(),
                        final_message_present: true,
                        artifact_path: Some(path.clone()),
                        clean: spec.clean_detector.map(|detector| detector(&message)),
                    };
                    let _ = write_command_outcome_if_requested(&outcome);

                    println!(
                        "{} enregistre dans {}",
                        capitalize(spec.saved_label),
                        path.display()
                    );
                    println!();
                    println!("{message}");
                }
                Err(error) => {
                    let outcome = CommandOutcome {
                        command_name: spec.command_name.to_string(),
                        status_code: result.status.code(),
                        final_message_present: true,
                        artifact_path: None,
                        clean: spec.clean_detector.map(|detector| detector(&message)),
                    };
                    let _ = write_command_outcome_if_requested(&outcome);

                    eprintln!(
                        "echec lors de l'enregistrement du {}: {error}",
                        spec.saved_label
                    );
                    process::exit(1);
                }
            }
        }
        Err(error) => {
            eprintln!("echec lors de l'appel a codex: {error}");
            process::exit(1);
        }
    }
}

pub fn write_command_outcome_if_requested(outcome: &CommandOutcome) -> io::Result<()> {
    let Some(path) = std::env::var_os(COMMAND_OUTCOME_PATH_ENV).map(PathBuf::from) else {
        return Ok(());
    };

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

    #[test]
    fn detects_clean_implementation_audit_summary_line() {
        assert!(detect_clean_implementation_audit(
            "# Rust Implementation Plan Audit\n\n## Summary\n- No actionable deviations found.\n"
        ));
        assert!(!detect_clean_implementation_audit(
            "# Rust Implementation Plan Audit\n\n## Summary\n- 2 actionable deviations found.\n"
        ));
    }
}
