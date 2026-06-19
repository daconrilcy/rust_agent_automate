use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact;
use crate::codex::{CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT};
use crate::reporting::{self, ReportSpec};
use crate::{ParseOutcome, next_value, parse_timeout};

#[derive(Debug, PartialEq, Eq)]
pub struct PlanCommand {
    pub request: CodexRequest,
    pub workspace_root: PathBuf,
    pub audit_path: PathBuf,
    pub output_dir: PathBuf,
    pub timeout: Duration,
}

pub fn resolve_audit_file(path: PathBuf) -> Result<PathBuf, String> {
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|error| format!("impossible de lire le repertoire courant: {error}"))?
            .join(path)
    };

    let metadata = fs::metadata(&path).map_err(|error| {
        format!(
            "impossible d'acceder au fichier d'audit {}: {error}",
            path.display()
        )
    })?;

    if !metadata.is_file() {
        return Err(format!(
            "le chemin d'audit doit etre un fichier: {}",
            path.display()
        ));
    }

    fs::canonicalize(&path).map_err(|error| {
        format!(
            "impossible de resoudre le fichier d'audit {}: {error}",
            path.display()
        )
    })
}

pub fn build_prompt(workspace_root: &Path, audit_path: &Path, output_dir: &Path) -> String {
    format!(
        concat!(
            "Use $refactor-plan-from-audit to convert the audit at \"{}\" into an implementation-ready integration plan.\n",
            "The plan must use the central Codex skill named refactor-plan-from-audit, follow its SKILL.md instructions, ",
            "and use references/plan-template.md as the output structure.\n",
            "The current local workspace running this command is \"{}\" and the final plan will be saved by the wrapper under \"{}\".\n",
            "Read the audit completely from the provided path. Inspect the local workspace only enough to make the plan concrete.\n",
            "Do not modify source code. Produce the final answer as a complete Markdown implementation handoff plan only.\n",
            "Include the source audit path, target workspace, prioritized phases, task backlog, traceability matrix, verification matrix, ",
            "decision gates, out-of-scope section, rollback or fallback notes, and the first prompt for the implementation agent."
        ),
        audit_path.display(),
        workspace_root.display(),
        output_dir.display()
    )
}

pub fn save_plan(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    artifact::save_timestamped_markdown(output_dir, "plan", content)
}

pub fn run(command: &PlanCommand) {
    eprintln!(
        "Plan Codex en cours depuis {} (timeout: {} secondes)...",
        command.audit_path.display(),
        command.timeout.as_secs()
    );

    reporting::run_codex_report(
        &command.request,
        command.timeout,
        ReportSpec {
            command_name: "plan",
            saved_label: "plan",
            final_label: "plan",
            missing_message_label: "plan",
            output_dir: &command.output_dir,
            save: save_plan,
            clean_detector: None,
        },
    );
}

pub fn parse_args(args: &[String]) -> Result<PlanCommand, ParseOutcome> {
    let mut model = String::from(DEFAULT_MODEL);
    let mut reasoning_effort = DEFAULT_REASONING_EFFORT;
    let mut verbose = false;
    let mut resume_last = false;
    let mut audit_path: Option<PathBuf> = None;
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
            "--audit" => {
                let value = next_value(args, index, "--audit")?;
                if audit_path.is_some() {
                    return Err(ParseOutcome::Error(
                        "l'audit a deja ete fourni pour la commande plan".to_string(),
                    ));
                }
                audit_path = Some(PathBuf::from(value));
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
                if audit_path.is_some() {
                    return Err(ParseOutcome::Error(format!(
                        "argument inattendu pour plan: {value}"
                    )));
                }
                audit_path = Some(PathBuf::from(value));
                index += 1;
            }
        }
    }

    let workspace_root = std::env::current_dir().map_err(|error| {
        ParseOutcome::Error(format!("impossible de lire le repertoire courant: {error}"))
    })?;
    let audit_path = resolve_audit_file(audit_path.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande plan requiert un chemin d'audit. Exemple: cargo run -p app -- plan .audit\\audit.md"
                .to_string(),
        )
    })?)
    .map_err(ParseOutcome::Error)?;
    let output_dir = output_dir.unwrap_or_else(|| workspace_root.join(".plan"));
    let prompt = build_prompt(&workspace_root, &audit_path, &output_dir);

    Ok(PlanCommand {
        request: CodexRequest::new(
            model,
            reasoning_effort,
            CodexMode::Exec,
            Some(prompt),
            verbose,
        )
        .with_resume_last(resume_last),
        workspace_root,
        audit_path,
        output_dir,
        timeout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_mentions_skill_and_paths() {
        let workspace = Path::new("C:\\dev\\rust_agent");
        let audit = Path::new("C:\\dev\\rust_agent\\.audit\\audit.md");

        let output_dir = Path::new("C:\\dev\\rust_agent\\.plan");

        let prompt = build_prompt(workspace, audit, output_dir);

        assert!(prompt.contains("$refactor-plan-from-audit"));
        assert!(prompt.contains("central Codex skill named refactor-plan-from-audit"));
        assert!(prompt.contains("references/plan-template.md"));
        assert!(prompt.contains("C:\\dev\\rust_agent\\.audit\\audit.md"));
        assert!(prompt.contains("C:\\dev\\rust_agent"));
        assert!(prompt.contains("C:\\dev\\rust_agent\\.plan"));
        assert!(prompt.contains("complete Markdown implementation handoff plan only"));
    }
}
