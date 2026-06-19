use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact;
use crate::codex::CodexRequest;

#[derive(Debug, PartialEq, Eq)]
pub struct ImplementationAuditCommand {
    pub request: CodexRequest,
    pub workspace_root: PathBuf,
    pub plan_path: PathBuf,
    pub implementation_path: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub timeout: Duration,
}

pub fn resolve_plan_file(path: PathBuf) -> Result<PathBuf, String> {
    resolve_existing_path(path, "plan d'implementation", PathRequirement::File)
}

pub fn resolve_implementation_path(path: PathBuf) -> Result<PathBuf, String> {
    resolve_existing_path(path, "implementation", PathRequirement::FileOrDirectory)
}

pub fn build_prompt(
    workspace_root: &Path,
    plan_path: &Path,
    implementation_path: Option<&Path>,
    output_dir: &Path,
) -> String {
    let implementation_scope = implementation_path.map_or_else(
        || {
            format!(
                "No explicit implementation path was provided. Resolve the implementation scope from git status and git diff in \"{}\"; if no narrower evidence exists, inspect the repository scope and state that limitation.",
                workspace_root.display()
            )
        },
        |path| {
            format!(
                "Review the implementation evidence at \"{}\" and inspect supporting diffs, tests, manifests, and call sites as needed.",
                path.display()
            )
        },
    );

    format!(
        concat!(
            "Use $rust-implementation-plan-audit to audit whether the Rust implementation follows the implementation plan at \"{}\".\n",
            "The audit must use the central Codex skill named rust-implementation-plan-audit and follow its SKILL.md instructions.\n",
            "The current local workspace running this command is \"{}\" and the final audit report will be saved by the wrapper under \"{}\".\n",
            "{}\n",
            "Read the implementation plan completely before judging the code. Build a requirements checklist from the plan, gather direct local evidence, and map every plan item to implementation status.\n",
            "Do not modify source code. Produce the final answer as a complete Markdown Rust Implementation Plan Audit report only, using the report structure required by the rust-implementation-plan-audit skill."
        ),
        plan_path.display(),
        workspace_root.display(),
        output_dir.display(),
        implementation_scope
    )
}

pub fn save_audit(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    artifact::save_timestamped_markdown(output_dir, "implementation-audit", content)
}

#[derive(Debug, Clone, Copy)]
enum PathRequirement {
    File,
    FileOrDirectory,
}

fn resolve_existing_path(
    path: PathBuf,
    label: &str,
    requirement: PathRequirement,
) -> Result<PathBuf, String> {
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|error| format!("impossible de lire le repertoire courant: {error}"))?
            .join(path)
    };

    let metadata = fs::metadata(&path).map_err(|error| {
        format!(
            "impossible d'acceder au chemin {label} {}: {error}",
            path.display()
        )
    })?;

    match requirement {
        PathRequirement::File if !metadata.is_file() => {
            return Err(format!(
                "le chemin {label} doit etre un fichier: {}",
                path.display()
            ));
        }
        PathRequirement::FileOrDirectory if !metadata.is_file() && !metadata.is_dir() => {
            return Err(format!(
                "le chemin {label} doit etre un fichier ou un dossier: {}",
                path.display()
            ));
        }
        _ => {}
    }

    fs::canonicalize(&path).map_err(|error| {
        format!(
            "impossible de resoudre le chemin {label} {}: {error}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_mentions_skill_plan_and_default_scope() {
        let workspace = Path::new("C:\\dev\\rust_agent");
        let plan = Path::new("C:\\dev\\rust_agent\\.plan\\plan.md");
        let output_dir = Path::new("C:\\dev\\rust_agent\\.audit");

        let prompt = build_prompt(workspace, plan, None, output_dir);

        assert!(prompt.contains("$rust-implementation-plan-audit"));
        assert!(prompt.contains("central Codex skill named rust-implementation-plan-audit"));
        assert!(prompt.contains("C:\\dev\\rust_agent\\.plan\\plan.md"));
        assert!(prompt.contains("No explicit implementation path was provided"));
        assert!(prompt.contains("complete Markdown Rust Implementation Plan Audit report only"));
    }

    #[test]
    fn prompt_mentions_explicit_implementation_scope() {
        let workspace = Path::new("C:\\dev\\rust_agent");
        let plan = Path::new("C:\\dev\\rust_agent\\.plan\\plan.md");
        let implementation = Path::new("C:\\dev\\rust_agent\\crates\\app");
        let output_dir = Path::new("C:\\dev\\rust_agent\\.audit");

        let prompt = build_prompt(workspace, plan, Some(implementation), output_dir);

        assert!(prompt.contains("Review the implementation evidence at"));
        assert!(prompt.contains("C:\\dev\\rust_agent\\crates\\app"));
    }

    #[test]
    fn resolve_plan_file_rejects_directory() {
        let error =
            resolve_plan_file(std::env::temp_dir()).expect_err("un plan doit etre un fichier");

        assert!(error.contains("le chemin plan d'implementation doit etre un fichier"));
    }
}
