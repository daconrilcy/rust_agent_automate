use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact;
use crate::codex::CodexRequest;
use crate::review::{self, ReviewSubject};

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
