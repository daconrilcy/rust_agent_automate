use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact_subject;
use crate::cli::ParseOutcome;
use crate::prompt::{PromptSection, render_structured_prompt};
use crate::service_command::{
    self, ParsedSubjectArtifact, PreparedServiceCommand, ServiceCommandDescriptor,
    ServiceCommandOptions,
};
use crate::service_paths;

pub use crate::artifact_subject::ReviewSubject;

#[derive(Debug, PartialEq, Eq)]
pub struct ReviewCommand {
    pub service: PreparedServiceCommand,
    pub workspace_root: PathBuf,
    pub subject: ReviewSubject,
    pub artifact_path: PathBuf,
}

pub(crate) const REVIEW_DESCRIPTOR: ServiceCommandDescriptor<'static> = ServiceCommandDescriptor {
    default_output_dir: ".review",
    command_name: "review",
    artifact_stem: "review",
    saved_label: "review",
    final_label: "review",
    missing_message_label: "review",
    clean_detector: None,
};

pub fn resolve_artifact_path(
    subject: ReviewSubject,
    path: PathBuf,
    context: &service_paths::ExecutionContext,
) -> Result<PathBuf, service_paths::PathResolutionError> {
    artifact_subject::resolve_artifact_path(subject, path, context)
}

pub fn build_prompt(
    workspace_root: &Path,
    subject: ReviewSubject,
    artifact_path: &Path,
    output_dir: &Path,
) -> String {
    render_structured_prompt(
        &format!(
            "Use $adversarial-review to perform an adversarial review of the {} at \"{}\".\nThe review must use the central Codex skill named adversarial-review and follow its SKILL.md instructions.\nReview mode: {}.",
            subject.artifact_name(),
            artifact_path.display(),
            match subject {
                ReviewSubject::Plan => "Plan review",
                ReviewSubject::Audit => "Audit review",
                ReviewSubject::Implementation => "Implementation review",
            }
        ),
        workspace_root,
        output_dir,
        "final review",
        &[PromptSection {
            heading: Cow::Borrowed(""),
            body: Cow::Borrowed(
                "Gather direct evidence before judging. Read the artifact completely and inspect supporting local files, diffs, tests, or linked artifacts when available.\nDo not modify source code. Produce the final answer as a complete Markdown adversarial review only.\nUse the output format specified by the adversarial-review skill: Findings, Open Questions, then Verdict.\nRank only actionable findings by real impact and likelihood. If there are no findings, say so explicitly and name residual risks or test gaps.",
            ),
        }],
    )
}

pub fn save_review(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    service_command::save_markdown_artifact(output_dir, REVIEW_DESCRIPTOR, content)
}

#[allow(dead_code)]
pub fn parse_args(args: &[String]) -> Result<ReviewCommand, ParseOutcome> {
    let context = service_command::resolve_context()
        .map_err(|error| ParseOutcome::Error(error.to_string()))?;
    parse_args_for_context(args, &context)
}

pub fn parse_args_for_context(
    args: &[String],
    context: &service_paths::ExecutionContext,
) -> Result<ReviewCommand, ParseOutcome> {
    let mut common = ServiceCommandOptions::new(Duration::from_secs(900));
    let ParsedSubjectArtifact {
        subject,
        artifact_path,
    } = service_command::parse_subject_and_artifact(
        args,
        &mut common,
        "--type",
        "--artifact",
        "review",
        |value| {
            value
                .parse()
                .map_err(service_command::ServiceCommandParseError::Message)
        },
    )
    .map_err(service_command::ServiceCommandParseError::into_parse_outcome)?;
    let prepared = service_command::prepare_prompted_service(
        &common,
        REVIEW_DESCRIPTOR,
        context.clone(),
        |parse_context| {
            let artifact_path =
                resolve_artifact_path(subject, artifact_path, &parse_context.context)
                    .map_err(|error| ParseOutcome::Error(error.to_string()))?;
            let prompt = build_prompt(
                &parse_context.workspace_root,
                subject,
                &artifact_path,
                &parse_context.output_dir,
            );
            Ok((artifact_path, prompt))
        },
    )?;

    Ok(ReviewCommand {
        service: prepared.service,
        workspace_root: prepared.workspace_root,
        subject,
        artifact_path: prepared.resolved,
    })
}
