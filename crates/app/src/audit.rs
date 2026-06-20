use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cli::ParseOutcome;
use crate::prompt::{PromptSection, render_structured_prompt};
use crate::service_command::{
    self, ParsedRequiredPath, PreparedServiceCommand, ServiceCommandDescriptor,
    ServiceCommandOptions,
};
use crate::service_paths::{self, PathRequirement};

#[derive(Debug, PartialEq, Eq)]
pub struct AuditCommand {
    pub service: PreparedServiceCommand,
    pub workspace_root: PathBuf,
    pub target_dir: PathBuf,
}

pub(crate) const AUDIT_DESCRIPTOR: ServiceCommandDescriptor<'static> = ServiceCommandDescriptor {
    default_output_dir: ".audit",
    command_name: "audit",
    artifact_stem: "audit",
    saved_label: "audit",
    final_label: "audit",
    missing_message_label: "rapport d'audit",
    clean_detector: None,
};

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
) -> Result<PathBuf, service_paths::PathResolutionError> {
    service_paths::resolve_existing_path(path, "dossier cible", PathRequirement::Directory, context)
        .map_err(|error| match error {
            service_paths::PathResolutionError::Access { path, error, .. } => {
                service_paths::PathResolutionError::Access {
                    label: "cible".to_string(),
                    path,
                    error,
                }
            }
            service_paths::PathResolutionError::WrongKind { path, expected, .. } => {
                service_paths::PathResolutionError::WrongKind {
                    label: "cible".to_string(),
                    path,
                    expected,
                }
            }
            service_paths::PathResolutionError::CanonicalizePath { path, error, .. } => {
                service_paths::PathResolutionError::CanonicalizePath {
                    label: "cible".to_string(),
                    path,
                    error,
                }
            }
            other => other,
        })
}

pub fn save_report(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    service_command::save_markdown_artifact(output_dir, AUDIT_DESCRIPTOR, content)
}

#[allow(dead_code)]
pub fn parse_args(args: &[String]) -> Result<AuditCommand, ParseOutcome> {
    let context = service_command::resolve_context().map_err(service_command::parse_error)?;
    parse_args_for_context(args, &context)
}

pub fn parse_args_for_context(
    args: &[String],
    context: &service_paths::ExecutionContext,
) -> Result<AuditCommand, ParseOutcome> {
    let mut common = ServiceCommandOptions::new(Duration::from_secs(900));
    let target_dir = service_command::parse_required_path(
        args,
        &mut common,
        service_command::RequiredPathParseSpec {
            required_option_name: "--target",
            optional_option_name: "",
            command_name: "audit",
            required_label: "un dossier cible",
            required_example: "cargo run -p app -- audit --target ..\\mon-projet",
            duplicate_required_message: "la cible de audit a deja ete fournie",
            duplicate_optional_message: "",
            allow_positional: false,
        },
    )?
    .unwrap_or_else(|| context.workspace_root().to_path_buf());
    let prepared = service_command::prepare_required_path_service(
        &common,
        AUDIT_DESCRIPTOR,
        context.clone(),
        ParsedRequiredPath {
            required_path: target_dir,
            optional_path: None,
        },
        |target_dir, _unused, resolution_context| {
            resolve_target_dir(target_dir, resolution_context)
        },
        |parse_context, target_dir| {
            build_prompt(
                &parse_context.workspace_root,
                target_dir,
                &parse_context.output_dir,
            )
        },
    )?;

    Ok(AuditCommand {
        service: prepared.service,
        workspace_root: prepared.workspace_root,
        target_dir: prepared.resolved,
    })
}
