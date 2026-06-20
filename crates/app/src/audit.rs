use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cli::{ParseOutcome, next_value};
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

pub fn build_prompt(workspace_root: &Path, target_dir: &Path, output_dir: &Path) -> String {
    format!(
        concat!(
            "Use $rust-refactor-audit to audit the Rust code located at \"{}\".\n",
            "The audit must use the central Codex skill named rust-refactor-audit, follow its SKILL.md instructions, ",
            "and apply its references/audit-rubric.md rubric.\n",
            "The current local workspace running this command is \"{}\" and the audit report will be saved by the wrapper under \"{}\".\n",
            "The target directory may not be a Git repository; if git commands fail for that reason, mention it briefly and continue.\n",
            "Inspect the current workspace before concluding and produce the final answer as a complete Markdown audit report only.\n",
            "Use this exact section order:\n",
            "1. Scope\n",
            "2. Architecture Snapshot\n",
            "3. Key Findings\n",
            "4. Principle Review (SOLID / DRY / KISS / YAGNI)\n",
            "5. Refactoring Roadmap\n",
            "6. Quick Wins\n",
            "7. Open Questions / Validation Needed\n"
        ),
        target_dir.display(),
        workspace_root.display(),
        output_dir.display()
    )
}

pub fn resolve_target_dir(
    path: PathBuf,
    context: &service_paths::ExecutionContext,
) -> Result<PathBuf, String> {
    service_paths::resolve_existing_path(path, "dossier cible", PathRequirement::Directory, context)
        .map_err(|error| error.replace("le chemin dossier cible", "le chemin cible"))
}

pub fn run(
    command: &AuditCommand,
) -> Result<crate::reporting::CompletedReport, crate::reporting::ReportFailure> {
    service_command::execute_service_command(
        &command.service,
        AUDIT_DESCRIPTOR,
        format!(
            "Audit Codex en cours sur {} (timeout: {} secondes)...",
            command.target_dir.display(),
            command.service.timeout.as_secs()
        ),
        save_report,
    )
}

pub fn run_silently(
    command: &AuditCommand,
) -> Result<crate::reporting::CompletedReport, crate::reporting::ReportFailure> {
    service_command::execute_service_command_silently(&command.service, AUDIT_DESCRIPTOR, save_report)
}

pub fn save_report(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    service_command::save_markdown_artifact(output_dir, AUDIT_DESCRIPTOR, content)
}

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

    let parse_context =
        service_command::prepare_parse_context_for_context(&common, AUDIT_DESCRIPTOR, context.clone());
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
    fn resolve_target_dir_rejects_file() {
        let context = service_paths::ExecutionContext::from_workspace_root(
            std::env::current_dir().expect("cwd"),
        );
        let error = resolve_target_dir(PathBuf::from("Cargo.toml"), &context)
            .expect_err("audit doit exiger un dossier");

        assert!(error.contains("le chemin cible doit etre un dossier"));
    }
}
