use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact;
use crate::codex::CodexRequest;
use crate::service_command::{self, ServiceCommandOptions, ServiceRunSpec};
use crate::service_paths::{self, PathRequirement};
use crate::{ParseOutcome, next_value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewSubject {
    Plan,
    Audit,
    Implementation,
}

impl ReviewSubject {
    pub fn as_cli_value(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Audit => "audit",
            Self::Implementation => "implementation",
        }
    }

    pub(crate) fn artifact_name(self) -> &'static str {
        match self {
            Self::Plan => "implementation plan",
            Self::Audit => "audit report",
            Self::Implementation => "implementation",
        }
    }

    fn review_mode(self) -> &'static str {
        match self {
            Self::Plan => "Plan review",
            Self::Audit => "Audit review",
            Self::Implementation => "Implementation review",
        }
    }
}

impl std::fmt::Display for ReviewSubject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_cli_value())
    }
}

impl std::str::FromStr for ReviewSubject {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "plan" => Ok(Self::Plan),
            "audit" => Ok(Self::Audit),
            "implementation" => Ok(Self::Implementation),
            _ => Err(format!(
                "type de review invalide: {value}. Valeurs attendues: plan, audit, implementation"
            )),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ReviewCommand {
    pub request: CodexRequest,
    pub workspace_root: PathBuf,
    pub subject: ReviewSubject,
    pub artifact_path: PathBuf,
    pub output_dir: PathBuf,
    pub timeout: Duration,
}

pub fn resolve_artifact_path(
    subject: ReviewSubject,
    path: PathBuf,
    context: &service_paths::ExecutionContext,
) -> Result<PathBuf, String> {
    let path = service_paths::resolve_existing_path(
        path,
        &format!("de review {subject}"),
        match subject {
            ReviewSubject::Plan | ReviewSubject::Audit => PathRequirement::File,
            ReviewSubject::Implementation => PathRequirement::FileOrDirectory,
        },
        &context,
    )?;

    match subject {
        ReviewSubject::Plan | ReviewSubject::Audit if !path.is_file() => Err(format!(
            "le chemin de review {subject} doit etre un fichier: {}",
            path.display()
        )),
        ReviewSubject::Implementation if !path.is_file() && !path.is_dir() => Err(format!(
            "le chemin de review implementation doit etre un fichier ou un dossier: {}",
            path.display()
        )),
        _ => Ok(path),
    }
}

pub fn build_prompt(
    workspace_root: &Path,
    subject: ReviewSubject,
    artifact_path: &Path,
    output_dir: &Path,
) -> String {
    format!(
        concat!(
            "Use $adversarial-review to perform an adversarial review of the {} at \"{}\".\n",
            "The review must use the central Codex skill named adversarial-review and follow its SKILL.md instructions.\n",
            "Review mode: {}.\n",
            "The current local workspace running this command is \"{}\" and the final review will be saved by the wrapper under \"{}\".\n",
            "Gather direct evidence before judging. Read the artifact completely and inspect supporting local files, diffs, tests, or linked artifacts when available.\n",
            "Do not modify source code. Produce the final answer as a complete Markdown adversarial review only.\n",
            "Use the output format specified by the adversarial-review skill: Findings, Open Questions, then Verdict.\n",
            "Rank only actionable findings by real impact and likelihood. If there are no findings, say so explicitly and name residual risks or test gaps."
        ),
        subject.artifact_name(),
        artifact_path.display(),
        subject.review_mode(),
        workspace_root.display(),
        output_dir.display()
    )
}

pub fn save_review(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    artifact::save_timestamped_markdown(output_dir, "review", content)
}

pub fn run(command: &ReviewCommand) {
    service_command::run_service_command(
        &command.request,
        command.timeout,
        ServiceRunSpec {
            intro: format!(
                "Review adversariale Codex en cours ({}) sur {} (timeout: {} secondes)...",
                command.subject,
                command.artifact_path.display(),
                command.timeout.as_secs()
            ),
            command_name: "review",
            saved_label: "review",
            final_label: "review",
            missing_message_label: "review",
            output_dir: &command.output_dir,
            save: save_review,
            clean_detector: None,
        },
    );
}

pub fn parse_args(args: &[String]) -> Result<ReviewCommand, ParseOutcome> {
    let mut common = ServiceCommandOptions::new(Duration::from_secs(900));
    let mut subject: Option<ReviewSubject> = None;
    let mut artifact_path: Option<PathBuf> = None;

    let mut index = 0;
    while index < args.len() {
        if let Some(consumed) = service_command::parse_common_option(args, index, &mut common)? {
            index += consumed;
            continue;
        }

        match args[index].as_str() {
            "--type" => {
                let value = next_value(args, index, "--type")?;
                if subject.is_some() {
                    return Err(ParseOutcome::Error(
                        "le type de review a deja ete fourni".to_string(),
                    ));
                }
                subject = Some(value.parse().map_err(ParseOutcome::Error)?);
                index += 2;
            }
            "--artifact" => {
                let value = next_value(args, index, "--artifact")?;
                if artifact_path.is_some() {
                    return Err(ParseOutcome::Error(
                        "l'artefact de review a deja ete fourni".to_string(),
                    ));
                }
                artifact_path = Some(PathBuf::from(value));
                index += 2;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            value => {
                if subject.is_none() {
                    subject = Some(value.parse().map_err(ParseOutcome::Error)?);
                    index += 1;
                    continue;
                }
                if artifact_path.is_none() {
                    artifact_path = Some(PathBuf::from(value));
                    index += 1;
                    continue;
                }
                return Err(ParseOutcome::Error(format!(
                    "argument inattendu pour review: {value}"
                )));
            }
        }
    }

    let subject = subject.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande review requiert un type: plan, audit ou implementation".to_string(),
        )
    })?;
    let artifact_path = artifact_path.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande review requiert un artefact. Exemple: cargo run -p app -- review plan .plan\\plan.md"
                .to_string(),
        )
    })?;
    let context = service_command::resolve_context().map_err(ParseOutcome::Error)?;
    let workspace_root = context.workspace_root().to_path_buf();
    let artifact_path =
        resolve_artifact_path(subject, artifact_path, &context).map_err(ParseOutcome::Error)?;
    let output_dir = service_command::resolve_output_dir(&common, &context, ".review");
    let prompt = build_prompt(&workspace_root, subject, &artifact_path, &output_dir);

    Ok(ReviewCommand {
        request: common.build_request(prompt),
        workspace_root,
        subject,
        artifact_path,
        output_dir,
        timeout: common.timeout,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_review_subjects() {
        assert_eq!("plan".parse::<ReviewSubject>(), Ok(ReviewSubject::Plan));
        assert_eq!("audit".parse::<ReviewSubject>(), Ok(ReviewSubject::Audit));
        assert_eq!(
            "implementation".parse::<ReviewSubject>(),
            Ok(ReviewSubject::Implementation)
        );
        assert!("design".parse::<ReviewSubject>().is_err());
    }

    #[test]
    fn prompt_mentions_skill_mode_and_paths() {
        let workspace = Path::new("C:\\dev\\rust_agent");
        let artifact = Path::new("C:\\dev\\rust_agent\\.plan\\plan.md");
        let output_dir = Path::new("C:\\dev\\rust_agent\\.review");

        let prompt = build_prompt(workspace, ReviewSubject::Plan, artifact, output_dir);

        assert!(prompt.contains("$adversarial-review"));
        assert!(prompt.contains("central Codex skill named adversarial-review"));
        assert!(prompt.contains("Review mode: Plan review"));
        assert!(prompt.contains("C:\\dev\\rust_agent\\.plan\\plan.md"));
        assert!(prompt.contains("C:\\dev\\rust_agent"));
        assert!(prompt.contains("C:\\dev\\rust_agent\\.review"));
        assert!(prompt.contains("complete Markdown adversarial review only"));
        assert!(prompt.contains("output format specified by the adversarial-review skill"));
    }

    #[test]
    fn resolve_artifact_path_rejects_directory_for_plan() {
        let context = service_paths::ExecutionContext::from_workspace_root(
            std::env::current_dir().expect("cwd"),
        );
        let error = resolve_artifact_path(ReviewSubject::Plan, std::env::temp_dir(), &context)
            .expect_err("plan doit exiger un fichier");

        assert!(error.contains("le chemin de review plan doit etre un fichier"));
    }

    #[test]
    fn resolve_artifact_path_accepts_directory_for_implementation() {
        let context = service_paths::ExecutionContext::from_workspace_root(
            std::env::current_dir().expect("cwd"),
        );
        let path = resolve_artifact_path(ReviewSubject::Implementation, std::env::temp_dir(), &context)
            .expect("implementation doit accepter un dossier");

        assert!(path.is_dir());
    }
}
