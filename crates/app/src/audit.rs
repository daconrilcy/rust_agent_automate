use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cli::{ParseOutcome, next_value};
use crate::prompt::{PromptSection, render_structured_prompt};
use crate::service_command::{
    self, PreparedServiceCommand, ServiceCommandDescriptor, ServiceCommandOptions,
};
use crate::service_paths::{self, PathRequirement};

#[derive(Debug, PartialEq, Eq)]
pub struct AuditCommand {
    pub service: PreparedServiceCommand,
    pub workspace_root: PathBuf,
    pub target_dir: PathBuf,
}

const AUDIT_DESCRIPTOR: ServiceCommandDescriptor<'static> = ServiceCommandDescriptor {
    default_output_dir: ".audit",
    command_name: "audit",
    artifact_stem: "audit",
    saved_label: "audit",
    final_label: "audit",
    missing_message_label: "rapport d'audit",
    clean_detector: None,
};

pub fn descriptor() -> ServiceCommandDescriptor<'static> {
    AUDIT_DESCRIPTOR
}

pub fn build_prompt(workspace_root: &Path, target_dir: &Path, output_dir: &Path) -> String {
    render_structured_prompt(
        &format!(
            "Use $rust-refactor-audit to audit the Rust code located at \"{}\".\nThe audit must use the central Codex skill named rust-refactor-audit, follow its SKILL.md instructions, and apply its references/audit-rubric.md rubric.",
            target_dir.display()
        ),
        workspace_root,
        output_dir,
        "audit report",
        &[
            PromptSection {
                heading: Cow::Borrowed(""),
                body: Cow::Borrowed(
                    "The target directory may not be a Git repository; if git commands fail for that reason, mention it briefly and continue.\nInspect the current workspace before concluding and produce the final answer as a complete Markdown audit report only.",
                ),
            },
            PromptSection {
                heading: Cow::Borrowed("Use this exact section order:"),
                body: Cow::Borrowed(
                    "1. Scope\n2. Architecture Snapshot\n3. Key Findings\n4. Principle Review (SOLID / DRY / KISS / YAGNI)\n5. Refactoring Roadmap\n6. Quick Wins\n7. Open Questions / Validation Needed",
                ),
            },
        ],
    )
}

pub fn resolve_target_dir(
    path: PathBuf,
    context: &service_paths::ExecutionContext,
) -> Result<PathBuf, String> {
    service_paths::resolve_existing_path(path, "dossier cible", PathRequirement::Directory, context)
        .map_err(|error| error.replace("le chemin dossier cible", "le chemin cible"))
}

pub fn save_report(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    service_command::save_markdown_artifact(output_dir, AUDIT_DESCRIPTOR, content)
}

#[allow(dead_code)]
pub fn parse_args(args: &[String]) -> Result<AuditCommand, ParseOutcome> {
    let context = service_command::resolve_context().map_err(ParseOutcome::Error)?;
    parse_args_for_context(args, &context)
}

pub fn parse_args_for_context(
    args: &[String],
    context: &service_paths::ExecutionContext,
) -> Result<AuditCommand, ParseOutcome> {
    let mut common = ServiceCommandOptions::new(Duration::from_secs(900));
    let mut target_dir: Option<PathBuf> = None;

    service_command::parse_with_common_options(args, &mut common, |index, value| match value {
        "--target" => {
            let value = next_value(args, index, "--target")?;
            target_dir = Some(PathBuf::from(value));
            Ok(2)
        }
        value if value.starts_with("--") => {
            Err(ParseOutcome::Error(format!("option inconnue: {value}")))
        }
        value => Err(ParseOutcome::Error(format!(
            "argument inattendu pour audit: {value}"
        ))),
    })?;

    let parse_context = service_command::prepare_parse_context_for_context(
        &common,
        AUDIT_DESCRIPTOR,
        context.clone(),
    );
    let workspace_root = parse_context.workspace_root.clone();
    let target_dir = resolve_target_dir(
        target_dir.unwrap_or_else(|| workspace_root.clone()),
        &parse_context.context,
    )
    .map_err(ParseOutcome::Error)?;
    let prompt = build_prompt(&workspace_root, &target_dir, &parse_context.output_dir);
    let service = service_command::prepare_service_from_prompt(
        &common,
        &parse_context,
        AUDIT_DESCRIPTOR,
        prompt,
    );

    Ok(AuditCommand {
        service,
        workspace_root,
        target_dir,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_mentions_skill_and_paths() {
        let workspace = Path::new("C:\\dev\\rust_agent");
        let target = Path::new("C:\\dev\\rust_agent\\crates\\app");
        let output_dir = Path::new("C:\\dev\\rust_agent\\.audit");

        let prompt = build_prompt(workspace, target, output_dir);

        assert!(prompt.contains("$rust-refactor-audit"));
        assert!(prompt.contains("central Codex skill named rust-refactor-audit"));
        assert!(prompt.contains("references/audit-rubric.md"));
        assert!(prompt.contains("C:\\dev\\rust_agent\\crates\\app"));
        assert!(prompt.contains("audit report will be saved by the wrapper"));
        assert!(prompt.contains("C:\\dev\\rust_agent\\.audit"));
        assert!(prompt.contains("complete Markdown audit report only"));
    }

    #[test]
    fn resolve_target_dir_rejects_file() {
        let context = service_paths::ExecutionContext::from_workspace_root(
            std::env::current_dir().expect("cwd"),
        );
        let error = resolve_target_dir(PathBuf::from("Cargo.toml"), &context)
            .expect_err("audit doit exiger un dossier");

        assert!(error.contains("le chemin cible doit etre un dossier"));
    }
}
