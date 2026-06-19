use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact;
use crate::codex::{CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT};
use crate::reporting::{self, ReportSpec};
use crate::review::{self, ReviewSubject};
use crate::service_paths;
use crate::{ParseOutcome, next_value, parse_timeout};

#[derive(Debug, PartialEq, Eq)]
pub struct FixLoopCommand {
    pub request: CodexRequest,
    pub workspace_root: PathBuf,
    pub input_kind: ReviewSubject,
    pub artifact_path: PathBuf,
    pub output_dir: PathBuf,
    pub timeout: Duration,
}

pub fn resolve_artifact_path(kind: ReviewSubject, path: PathBuf) -> Result<PathBuf, String> {
    review::resolve_artifact_path(kind, path)
}

pub fn build_prompt(
    workspace_root: &Path,
    input_kind: ReviewSubject,
    artifact_path: &Path,
    output_dir: &Path,
) -> String {
    format!(
        concat!(
            "Use $rust-review-fix-loop to process the {} at \"{}\".\n",
            "The loop must use the central Codex skill named rust-review-fix-loop and follow its SKILL.md instructions.\n",
            "Input kind: {}.\n",
            "The current local workspace running this command is \"{}\" and the final loop report will be saved by the wrapper under \"{}\".\n",
            "If the input is an audit, read the audit completely, derive the required Rust implementation work, apply corrections with rust-dev-solid, then run adversarial-review cycles until no actionable findings remain.\n",
            "If the input is a plan, read the plan completely, implement the scoped Rust changes with rust-dev-solid, then run adversarial-review cycles until no actionable findings remain.\n",
            "If the input is an implementation, review the current implementation at the provided path, correct actionable findings with rust-dev-solid, then repeat review and correction until clean.\n",
            "Inspect the local workspace directly before editing. If this workspace is not a Git repository, mention that briefly and continue from filesystem evidence.\n",
            "Produce the final answer as a complete Markdown loop report with: final review verdict, files changed, verification commands and results, residual risks, and any blocker."
        ),
        input_kind.artifact_name(),
        artifact_path.display(),
        input_kind,
        workspace_root.display(),
        output_dir.display()
    )
}

pub fn save_report(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    artifact::save_timestamped_markdown(output_dir, "fix-loop", content)
}

pub fn run(command: &FixLoopCommand) {
    eprintln!(
        "Boucle review/correction Codex en cours ({}) sur {} (timeout: {} secondes)...",
        command.input_kind,
        command.artifact_path.display(),
        command.timeout.as_secs()
    );

    reporting::run_codex_report(
        &command.request,
        command.timeout,
        ReportSpec {
            command_name: "fix-loop",
            saved_label: "rapport fix-loop",
            final_label: "rapport fix-loop",
            missing_message_label: "rapport fix-loop",
            output_dir: &command.output_dir,
            save: save_report,
            clean_detector: None,
        },
    );
}

pub fn parse_args(args: &[String]) -> Result<FixLoopCommand, ParseOutcome> {
    let mut model = String::from(DEFAULT_MODEL);
    let mut reasoning_effort = DEFAULT_REASONING_EFFORT;
    let mut verbose = false;
    let mut resume_last = false;
    let mut input_kind: Option<ReviewSubject> = None;
    let mut artifact_path: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut timeout = Duration::from_secs(1800);

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
            "--type" => {
                let value = next_value(args, index, "--type")?;
                if input_kind.is_some() {
                    return Err(ParseOutcome::Error(
                        "le type d'entree de fix-loop a deja ete fourni".to_string(),
                    ));
                }
                input_kind = Some(parse_input_kind(value).map_err(ParseOutcome::Error)?);
                index += 2;
            }
            "--artifact" => {
                let value = next_value(args, index, "--artifact")?;
                if artifact_path.is_some() {
                    return Err(ParseOutcome::Error(
                        "l'artefact de fix-loop a deja ete fourni".to_string(),
                    ));
                }
                artifact_path = Some(PathBuf::from(value));
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
                if input_kind.is_none() {
                    input_kind = Some(parse_input_kind(value).map_err(ParseOutcome::Error)?);
                    index += 1;
                    continue;
                }
                if artifact_path.is_none() {
                    artifact_path = Some(PathBuf::from(value));
                    index += 1;
                    continue;
                }
                return Err(ParseOutcome::Error(format!(
                    "argument inattendu pour fix-loop: {value}"
                )));
            }
        }
    }

    let input_kind = input_kind.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande fix-loop requiert un type: plan, audit ou implementation".to_string(),
        )
    })?;
    let artifact_path = artifact_path.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande fix-loop requiert un artefact. Exemple: cargo run -p app -- fix-loop plan .plan\\plan.md"
                .to_string(),
        )
    })?;
    let workspace_root = service_paths::current_workspace_root().map_err(ParseOutcome::Error)?;
    let artifact_path =
        resolve_artifact_path(input_kind, artifact_path).map_err(ParseOutcome::Error)?;
    let output_dir = service_paths::resolve_output_dir(output_dir, &workspace_root, ".fix-loop");
    let prompt = build_prompt(&workspace_root, input_kind, &artifact_path, &output_dir);

    Ok(FixLoopCommand {
        request: CodexRequest::new(
            model,
            reasoning_effort,
            CodexMode::Exec,
            Some(prompt),
            verbose,
        )
        .with_resume_last(resume_last),
        workspace_root,
        input_kind,
        artifact_path,
        output_dir,
        timeout,
    })
}

fn parse_input_kind(value: &str) -> Result<ReviewSubject, String> {
    value.parse::<ReviewSubject>().map_err(|_| {
        format!(
            "type d'entree fix-loop invalide: {value}. Valeurs attendues: plan, audit, implementation"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_mentions_loop_skill_and_input_kind() {
        let workspace = Path::new("C:\\dev\\rust_agent");
        let artifact = Path::new("C:\\dev\\rust_agent\\.plan\\plan.md");
        let output_dir = Path::new("C:\\dev\\rust_agent\\.fix-loop");

        let prompt = build_prompt(workspace, ReviewSubject::Plan, artifact, output_dir);

        assert!(prompt.contains("$rust-review-fix-loop"));
        assert!(prompt.contains("central Codex skill named rust-review-fix-loop"));
        assert!(prompt.contains("Input kind: plan"));
        assert!(prompt.contains("C:\\dev\\rust_agent\\.plan\\plan.md"));
        assert!(prompt.contains("C:\\dev\\rust_agent\\.fix-loop"));
        assert!(prompt.contains("rust-dev-solid"));
        assert!(prompt.contains("adversarial-review cycles until no actionable findings remain"));
        assert!(prompt.contains("complete Markdown loop report"));
    }

    #[test]
    fn resolve_accepts_implementation_directory() {
        let path = resolve_artifact_path(ReviewSubject::Implementation, std::env::temp_dir())
            .expect("implementation doit accepter un dossier");

        assert!(path.is_dir());
    }
}
