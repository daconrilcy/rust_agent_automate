use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact_subject::ReviewSubject;
use crate::cli::ParseOutcome;
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

const FIX_LOOP_DESCRIPTOR: ServiceCommandDescriptor<'static> = ServiceCommandDescriptor {
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
) -> Result<PathBuf, String> {
    crate::artifact_subject::resolve_artifact_path(kind, path, context)
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
    service_command::save_markdown_artifact(output_dir, FIX_LOOP_DESCRIPTOR, content)
}

pub fn run(
    command: &FixLoopCommand,
) -> Result<crate::reporting::CompletedReport, crate::reporting::ReportFailure> {
    service_command::execute_service_command(
        &command.service,
        FIX_LOOP_DESCRIPTOR,
        format!(
            "Boucle review/correction Codex en cours ({}) sur {} (timeout: {} secondes)...",
            command.input_kind,
            command.artifact_path.display(),
            command.service.timeout.as_secs()
        ),
        save_report,
    )
}

pub fn run_silently(
    command: &FixLoopCommand,
) -> Result<crate::reporting::CompletedReport, crate::reporting::ReportFailure> {
    service_command::execute_service_command_silently(
        &command.service,
        FIX_LOOP_DESCRIPTOR,
        save_report,
    )
}

pub fn parse_args(args: &[String]) -> Result<FixLoopCommand, ParseOutcome> {
    let context = service_command::resolve_context().map_err(ParseOutcome::Error)?;
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
    )?;
    let parse_context = service_command::prepare_parse_context_for_context(
        &common,
        FIX_LOOP_DESCRIPTOR,
        context.clone(),
    );
    let workspace_root = parse_context.workspace_root.clone();
    let artifact_path = resolve_artifact_path(input_kind, artifact_path, &parse_context.context)
        .map_err(ParseOutcome::Error)?;
    let prompt = build_prompt(
        &workspace_root,
        input_kind,
        &artifact_path,
        &parse_context.output_dir,
    );
    let service = service_command::prepare_service_from_prompt(
        &common,
        &parse_context,
        FIX_LOOP_DESCRIPTOR,
        prompt,
    );

    Ok(FixLoopCommand {
        service,
        workspace_root,
        input_kind,
        artifact_path,
    })
}

fn parse_input_kind(value: &str) -> Result<ReviewSubject, String> {
    value.parse::<ReviewSubject>().map_err(|_| {
        format!(
            "type d'entree fix-loop invalide: {value}. Valeurs attendues: plan, audit, implementation"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_type_keeps_fix_loop_specific_message() {
        let error = parse_args(&[
            "--type".to_string(),
            "plan".to_string(),
            "--type".to_string(),
            "audit".to_string(),
            "Cargo.toml".to_string(),
        ])
        .expect_err("double type refuse");

        assert_eq!(
            error,
            ParseOutcome::Error("le type de fix-loop a deja ete fourni".to_string())
        );
    }

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
        let context = crate::service_paths::ExecutionContext::from_workspace_root(
            std::env::current_dir().expect("cwd"),
        );
        let path = resolve_artifact_path(
            ReviewSubject::Implementation,
            std::env::temp_dir(),
            &context,
        )
        .expect("implementation doit accepter un dossier");

        assert!(path.is_dir());
    }
}
