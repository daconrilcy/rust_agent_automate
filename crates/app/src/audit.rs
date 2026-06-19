use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact;
use crate::codex::{CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT};
use crate::reporting::{self, ReportSpec};
use crate::{ParseOutcome, next_value, parse_timeout};

#[derive(Debug, PartialEq, Eq)]
pub struct AuditCommand {
    pub request: CodexRequest,
    pub workspace_root: PathBuf,
    pub target_dir: PathBuf,
    pub output_dir: PathBuf,
    pub timeout: Duration,
}

pub fn build_prompt(workspace_root: &Path, target_dir: &Path, output_dir: &Path) -> String {
    format!(
        concat!(
            "Use $rust-refactor-audit to audit the Rust code located at \"{}\".\n",
            "The audit must use the central Codex skill named rust-refactor-audit, follow its SKILL.md instructions, ",
            "and apply its references/audit-rubric.md rubric.\n",
            "The current local workspace running this command is \"{}\" and the audit report will be saved by the wrapper under \"{}\".\n",
            "The target directory may not be a Git repository; if git commands fail for that reason, mention it briefly and continue.\n",
            "Inspect the current workspace before concluding and produce the final answer as a complete Markdown audit report only.\n",
            "Use this exact section order:\n",
            "1. Scope\n",
            "2. Architecture Snapshot\n",
            "3. Key Findings\n",
            "4. Principle Review (SOLID / DRY / KISS / YAGNI)\n",
            "5. Refactoring Roadmap\n",
            "6. Quick Wins\n",
            "7. Open Questions / Validation Needed\n"
        ),
        target_dir.display(),
        workspace_root.display(),
        output_dir.display()
    )
}

pub fn resolve_target_dir(path: PathBuf) -> Result<PathBuf, String> {
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|error| format!("impossible de lire le repertoire courant: {error}"))?
            .join(path)
    };

    let metadata = fs::metadata(&path).map_err(|error| {
        format!(
            "impossible d'acceder au dossier cible {}: {error}",
            path.display()
        )
    })?;

    if !metadata.is_dir() {
        return Err(format!(
            "le chemin cible doit etre un dossier: {}",
            path.display()
        ));
    }

    fs::canonicalize(&path).map_err(|error| {
        format!(
            "impossible de resoudre le dossier cible {}: {error}",
            path.display()
        )
    })
}

pub fn run(command: &AuditCommand) {
    eprintln!(
        "Audit Codex en cours sur {} (timeout: {} secondes)...",
        command.target_dir.display(),
        command.timeout.as_secs()
    );

    reporting::run_codex_report(
        &command.request,
        command.timeout,
        ReportSpec {
            command_name: "audit",
            saved_label: "audit",
            final_label: "audit",
            missing_message_label: "rapport d'audit",
            output_dir: &command.output_dir,
            save: save_report,
            clean_detector: None,
        },
    );
}

pub fn save_report(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    artifact::save_timestamped_markdown(output_dir, "audit", content)
}

pub fn parse_args(args: &[String]) -> Result<AuditCommand, ParseOutcome> {
    let mut model = String::from(DEFAULT_MODEL);
    let mut reasoning_effort = DEFAULT_REASONING_EFFORT;
    let mut verbose = false;
    let mut resume_last = false;
    let mut target_dir: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut timeout = Duration::from_secs(900);

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--model" => {
                let value = next_value(args, index, "--model")?;
                model = value.to_owned();
                index += 2;
            }
            "--reasoning" => {
                let value = next_value(args, index, "--reasoning")?;
                reasoning_effort = value.parse().map_err(ParseOutcome::Error)?;
                index += 2;
            }
            "--target" => {
                let value = next_value(args, index, "--target")?;
                target_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--output-dir" => {
                let value = next_value(args, index, "--output-dir")?;
                output_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--timeout-seconds" => {
                let value = next_value(args, index, "--timeout-seconds")?;
                timeout = parse_timeout(value)?;
                index += 2;
            }
            "--verbose" => {
                verbose = true;
                index += 1;
            }
            "--continue-codex" => {
                resume_last = true;
                index += 1;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            value => {
                return Err(ParseOutcome::Error(format!(
                    "argument inattendu pour audit: {value}"
                )));
            }
        }
    }

    let workspace_root = std::env::current_dir().map_err(|error| {
        ParseOutcome::Error(format!("impossible de lire le repertoire courant: {error}"))
    })?;
    let target_dir = resolve_target_dir(target_dir.unwrap_or_else(|| workspace_root.clone()))
        .map_err(ParseOutcome::Error)?;
    let output_dir = output_dir.unwrap_or_else(|| workspace_root.join(".audit"));
    let prompt = build_prompt(&workspace_root, &target_dir, &output_dir);

    Ok(AuditCommand {
        request: CodexRequest::new(
            model,
            reasoning_effort,
            CodexMode::Exec,
            Some(prompt),
            verbose,
        )
        .with_resume_last(resume_last),
        workspace_root,
        target_dir,
        output_dir,
        timeout,
    })
}
