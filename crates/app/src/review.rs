use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact;
use crate::codex::CodexRequest;

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

pub fn resolve_artifact_path(subject: ReviewSubject, path: PathBuf) -> Result<PathBuf, String> {
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|error| format!("impossible de lire le repertoire courant: {error}"))?
            .join(path)
    };

    let metadata = fs::metadata(&path).map_err(|error| {
        format!(
            "impossible d'acceder a l'artefact {} {}: {error}",
            subject,
            path.display()
        )
    })?;

    match subject {
        ReviewSubject::Plan | ReviewSubject::Audit if !metadata.is_file() => {
            return Err(format!(
                "le chemin de review {subject} doit etre un fichier: {}",
                path.display()
            ));
        }
        ReviewSubject::Implementation if !metadata.is_file() && !metadata.is_dir() => {
            return Err(format!(
                "le chemin de review implementation doit etre un fichier ou un dossier: {}",
                path.display()
            ));
        }
        _ => {}
    }

    fs::canonicalize(&path).map_err(|error| {
        format!(
            "impossible de resoudre l'artefact {} {}: {error}",
            subject,
            path.display()
        )
    })
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
        let error = resolve_artifact_path(ReviewSubject::Plan, std::env::temp_dir())
            .expect_err("plan doit exiger un fichier");

        assert!(error.contains("le chemin de review plan doit etre un fichier"));
    }

    #[test]
    fn resolve_artifact_path_accepts_directory_for_implementation() {
        let path = resolve_artifact_path(ReviewSubject::Implementation, std::env::temp_dir())
            .expect("implementation doit accepter un dossier");

        assert!(path.is_dir());
    }
}
