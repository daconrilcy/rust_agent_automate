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
pub struct PlanCommand {
    pub service: PreparedServiceCommand,
    pub workspace_root: PathBuf,
    pub audit_path: PathBuf,
}

pub(crate) const PLAN_DESCRIPTOR: ServiceCommandDescriptor<'static> = ServiceCommandDescriptor {
    default_output_dir: ".plan",
    command_name: "plan",
    artifact_stem: "plan",
    saved_label: "plan",
    final_label: "plan",
    missing_message_label: "plan",
    clean_detector: None,
};

pub fn resolve_audit_file(
    path: PathBuf,
    context: &service_paths::ExecutionContext,
) -> Result<PathBuf, service_paths::PathResolutionError> {
    service_paths::resolve_existing_path(path, "audit", PathRequirement::File, context).map_err(
        |error| match error {
            service_paths::PathResolutionError::Access { path, error, .. } => {
                service_paths::PathResolutionError::Access {
                    label: "d'audit".to_string(),
                    path,
                    error,
                }
            }
            service_paths::PathResolutionError::WrongKind { path, expected, .. } => {
                service_paths::PathResolutionError::WrongKind {
                    label: "d'audit".to_string(),
                    path,
                    expected,
                }
            }
            service_paths::PathResolutionError::CanonicalizePath { path, error, .. } => {
                service_paths::PathResolutionError::CanonicalizePath {
                    label: "d'audit".to_string(),
                    path,
                    error,
                }
            }
            other => other,
        },
    )
}

pub fn build_prompt(workspace_root: &Path, audit_path: &Path, output_dir: &Path) -> String {
    render_structured_prompt(
        &format!(
            "Use $refactor-plan-from-audit to convert the audit at \"{}\" into an implementation-ready integration plan.\nThe plan must use the central Codex skill named refactor-plan-from-audit, follow its SKILL.md instructions, and use references/plan-template.md as the output structure.",
            audit_path.display()
        ),
        workspace_root,
        output_dir,
        "final plan",
        &[PromptSection {
            heading: Cow::Borrowed(""),
            body: Cow::Borrowed(
                "Read the audit completely from the provided path. Inspect the local workspace only enough to make the plan concrete.\nDo not modify source code. Produce the final answer as a complete Markdown implementation handoff plan only.\nInclude the source audit path, target workspace, prioritized phases, task backlog, traceability matrix, verification matrix, decision gates, out-of-scope section, rollback or fallback notes, and the first prompt for the implementation agent.",
            ),
        }],
    )
}

pub fn save_plan(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    service_command::save_markdown_artifact(output_dir, PLAN_DESCRIPTOR, content)
}

#[allow(dead_code)]
pub fn parse_args(args: &[String]) -> Result<PlanCommand, ParseOutcome> {
    let context = service_command::resolve_context().map_err(service_command::parse_error)?;
    parse_args_for_context(args, &context)
}

pub fn parse_args_for_context(
    args: &[String],
    context: &service_paths::ExecutionContext,
) -> Result<PlanCommand, ParseOutcome> {
    let mut common = ServiceCommandOptions::new(Duration::from_secs(900));
    let mut audit_path: Option<PathBuf> = None;

    service_command::parse_with_common_options(args, &mut common, |index, value| match value {
        "--audit" => {
            let value = next_value(args, index, "--audit")?;
            if audit_path.is_some() {
                return Err(ParseOutcome::Error(
                    "l'audit a deja ete fourni pour la commande plan".to_string(),
                ));
            }
            audit_path = Some(PathBuf::from(value));
            Ok(2)
        }
        value if value.starts_with("--") => {
            Err(ParseOutcome::Error(format!("option inconnue: {value}")))
        }
        value => {
            if audit_path.is_some() {
                Err(ParseOutcome::Error(format!(
                    "argument inattendu pour plan: {value}"
                )))
            } else {
                audit_path = Some(PathBuf::from(value));
                Ok(1)
            }
        }
    })?;

    let prepared = service_command::prepare_required_path_service(
        &common,
        PLAN_DESCRIPTOR,
        context.clone(),
        service_command::ParsedRequiredPath {
            required_path: audit_path.ok_or_else(|| {
                ParseOutcome::Error(
                    "la commande plan requiert un chemin d'audit. Exemple: cargo run -p app -- plan .audit\\audit.md"
                        .to_string(),
                )
            })?,
            optional_path: None,
        },
        |audit_path, _unused, resolution_context| resolve_audit_file(audit_path, resolution_context),
        |parse_context, audit_path| {
            build_prompt(
                &parse_context.workspace_root,
                audit_path,
                &parse_context.output_dir,
            )
        },
    )?;

    Ok(PlanCommand {
        service: prepared.service,
        workspace_root: prepared.workspace_root,
        audit_path: prepared.resolved,
    })
}
