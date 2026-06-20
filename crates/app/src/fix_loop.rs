use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact_subject::ReviewSubject;
use crate::cli::ParseOutcome;
use crate::prompt::{PromptSection, render_structured_prompt};
use crate::service_command::{
    self, ParsedSubjectArtifact, PreparedServiceCommand, ServiceCommandDescriptor,
    ServiceCommandOptions,
};

#[derive(Debug, PartialEq, Eq)]
pub struct FixLoopCommand {
    pub service: PreparedServiceCommand,
    pub workspace_root: PathBuf,
    pub input_kind: ReviewSubject,
    pub artifact_path: PathBuf,
}

pub(crate) const FIX_LOOP_DESCRIPTOR: ServiceCommandDescriptor<'static> =
    ServiceCommandDescriptor {
        default_output_dir: ".fix-loop",
        command_name: "fix-loop",
        artifact_stem: "fix-loop",
        saved_label: "rapport fix-loop",
        final_label: "rapport fix-loop",
        missing_message_label: "rapport fix-loop",
        clean_detector: None,
    };

pub fn resolve_artifact_path(
    kind: ReviewSubject,
    path: PathBuf,
    context: &crate::service_paths::ExecutionContext,
) -> Result<PathBuf, crate::service_paths::PathResolutionError> {
    crate::artifact_subject::resolve_artifact_path(kind, path, context)
}

pub fn build_prompt(
    workspace_root: &Path,
    input_kind: ReviewSubject,
    artifact_path: &Path,
    output_dir: &Path,
) -> String {
    render_structured_prompt(
        &format!(
            "Use $rust-review-fix-loop to process the {} at \"{}\".\nThe loop must use the central Codex skill named rust-review-fix-loop and follow its SKILL.md instructions.\nInput kind: {}.",
            input_kind.artifact_name(),
            artifact_path.display(),
            input_kind
        ),
        workspace_root,
        output_dir,
        "final loop report",
        &[PromptSection {
            heading: Cow::Borrowed(""),
            body: Cow::Borrowed(
                "If the input is an audit, read the audit completely, derive the required Rust implementation work, apply corrections with rust-dev-solid, then run adversarial-review cycles until no actionable findings remain.\nIf the input is a plan, read the plan completely, implement the scoped Rust changes with rust-dev-solid, then run adversarial-review cycles until no actionable findings remain.\nIf the input is an implementation, review the current implementation at the provided path, correct actionable findings with rust-dev-solid, then repeat review and correction until clean.\nInspect the local workspace directly before editing. If this workspace is not a Git repository, mention that briefly and continue from filesystem evidence.\nProduce the final answer as a complete Markdown loop report with: final review verdict, files changed, verification commands and results, residual risks, and any blocker.",
            ),
        }],
    )
}

pub fn save_report(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    service_command::save_markdown_artifact(output_dir, FIX_LOOP_DESCRIPTOR, content)
}

#[allow(dead_code)]
pub fn parse_args(args: &[String]) -> Result<FixLoopCommand, ParseOutcome> {
    let context = service_command::resolve_context().map_err(service_command::parse_error)?;
    parse_args_for_context(args, &context)
}

pub fn parse_args_for_context(
    args: &[String],
    context: &crate::service_paths::ExecutionContext,
) -> Result<FixLoopCommand, ParseOutcome> {
    let mut common = ServiceCommandOptions::new(Duration::from_secs(1800));
    let ParsedSubjectArtifact {
        subject: input_kind,
        artifact_path,
    } = service_command::parse_subject_and_artifact(
        args,
        &mut common,
        "--type",
        "--artifact",
        "fix-loop",
        parse_input_kind,
    )
    .map_err(service_command::ServiceCommandParseError::into_parse_outcome)?;
    let prepared = service_command::prepare_prompted_service(
        &common,
        FIX_LOOP_DESCRIPTOR,
        context.clone(),
        |parse_context| {
            let artifact_path =
                resolve_artifact_path(input_kind, artifact_path, &parse_context.context)
                    .map_err(service_command::parse_error)?;
            let prompt = build_prompt(
                &parse_context.workspace_root,
                input_kind,
                &artifact_path,
                &parse_context.output_dir,
            );
            Ok((artifact_path, prompt))
        },
    )?;

    Ok(FixLoopCommand {
        service: prepared.service,
        workspace_root: prepared.workspace_root,
        input_kind,
        artifact_path: prepared.resolved,
    })
}

fn parse_input_kind(
    value: &str,
) -> Result<ReviewSubject, service_command::ServiceCommandParseError> {
    value.parse::<ReviewSubject>().map_err(|_| {
        service_command::ServiceCommandParseError::Message(format!(
            "type d'entree fix-loop invalide: {value}. Valeurs attendues: plan, audit, implementation"
        ))
    })
}
