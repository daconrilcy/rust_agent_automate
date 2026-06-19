use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact;
use crate::codex::CodexRequest;

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
