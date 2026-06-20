use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cli::ParseOutcome;
use crate::prompt::{PromptSection, render_structured_prompt};
use crate::reporting;
use crate::service_command::{
    self, ParsedRequiredPath, PreparedServiceCommand, RequiredPathParseSpec,
    ServiceCommandDescriptor, ServiceCommandOptions,
};
use crate::service_paths::{self, PathRequirement};

#[derive(Debug, PartialEq, Eq)]
pub struct ImplementationAuditCommand {
    pub service: PreparedServiceCommand,
    pub workspace_root: PathBuf,
    pub plan_path: PathBuf,
    pub implementation_path: Option<PathBuf>,
}

pub(crate) const IMPLEMENTATION_AUDIT_DESCRIPTOR: ServiceCommandDescriptor<'static> =
    ServiceCommandDescriptor {
        default_output_dir: ".audit",
        command_name: "implementation-audit",
        artifact_stem: "implementation-audit",
        saved_label: "audit d'implementation",
        final_label: "audit d'implementation",
        missing_message_label: "audit d'implementation",
        clean_detector: Some(reporting::detect_clean_implementation_audit),
    };

pub fn resolve_plan_file(
    path: PathBuf,
    context: &service_paths::ExecutionContext,
) -> Result<PathBuf, String> {
    service_paths::resolve_existing_path(
        path,
        "plan d'implementation",
        PathRequirement::File,
        context,
    )
}

pub fn resolve_implementation_path(
    path: PathBuf,
    context: &service_paths::ExecutionContext,
) -> Result<PathBuf, String> {
    service_paths::resolve_existing_path(
        path,
        "implementation",
        PathRequirement::FileOrDirectory,
        context,
    )
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
    let body = format!(
        "{}\nRead the implementation plan completely before judging the code. Build a requirements checklist from the plan, gather direct local evidence, and map every plan item to implementation status.\nDo not modify source code. Produce the final answer as a complete Markdown Rust Implementation Plan Audit report only, using the report structure required by the rust-implementation-plan-audit skill.",
        implementation_scope
    );

    render_structured_prompt(
        &format!(
            "Use $rust-implementation-plan-audit to audit whether the Rust implementation follows the implementation plan at \"{}\".\nThe audit must use the central Codex skill named rust-implementation-plan-audit and follow its SKILL.md instructions.",
            plan_path.display()
        ),
        workspace_root,
        output_dir,
        "final audit report",
        &[PromptSection {
            heading: Cow::Borrowed(""),
            body: Cow::Owned(body),
        }],
    )
}

pub fn save_audit(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    service_command::save_markdown_artifact(output_dir, IMPLEMENTATION_AUDIT_DESCRIPTOR, content)
}

#[allow(dead_code)]
pub fn parse_args(args: &[String]) -> Result<ImplementationAuditCommand, ParseOutcome> {
    let context = service_command::resolve_context().map_err(ParseOutcome::Error)?;
    parse_args_for_context(args, &context)
}

pub fn parse_args_for_context(
    args: &[String],
    context: &service_paths::ExecutionContext,
) -> Result<ImplementationAuditCommand, ParseOutcome> {
    let mut common = ServiceCommandOptions::new(Duration::from_secs(900));
    let ParsedRequiredPath {
        required_path: plan_path,
        optional_path: implementation_path,
    } = service_command::parse_required_path_with_optional_named_path(
        args,
        &mut common,
        RequiredPathParseSpec {
            required_option_name: "--plan",
            optional_option_name: "--implementation",
            command_name: "implementation-audit",
            required_label: "un plan",
            required_example: "cargo run -p app -- implementation-audit .plan\\plan.md",
            duplicate_required_message: "le plan d'implementation a deja ete fourni",
            duplicate_optional_message: "le chemin d'implementation a deja ete fourni",
        },
    )?;
    let prepared = service_command::prepare_prompted_service(
        &common,
        IMPLEMENTATION_AUDIT_DESCRIPTOR,
        context.clone(),
        |parse_context| {
            let plan_path = resolve_plan_file(plan_path, &parse_context.context)
                .map_err(ParseOutcome::Error)?;
            let implementation_path = implementation_path
                .map(|path| resolve_implementation_path(path, &parse_context.context))
                .transpose()
                .map_err(ParseOutcome::Error)?;
            let prompt = build_prompt(
                &parse_context.workspace_root,
                &plan_path,
                implementation_path.as_deref(),
                &parse_context.output_dir,
            );
            Ok(((plan_path, implementation_path), prompt))
        },
    )?;
    let (plan_path, implementation_path) = prepared.resolved;

    Ok(ImplementationAuditCommand {
        service: prepared.service,
        workspace_root: prepared.workspace_root,
        plan_path,
        implementation_path,
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
        assert!(prompt.contains("final audit report will be saved by the wrapper"));
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
        let context = service_paths::ExecutionContext::from_workspace_root(
            std::env::current_dir().expect("cwd"),
        );
        let error = resolve_plan_file(std::env::temp_dir(), &context)
            .expect_err("un plan doit etre un fichier");

        assert!(error.contains("le chemin plan d'implementation doit etre un fichier"));
    }

    #[test]
    fn resolve_implementation_path_accepts_directory() {
        let context = service_paths::ExecutionContext::from_workspace_root(
            std::env::current_dir().expect("cwd"),
        );
        let path = resolve_implementation_path(std::env::temp_dir(), &context)
            .expect("implementation doit accepter un dossier");

        assert!(path.is_dir());
    }
}
