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

const PLAN_DESCRIPTOR: ServiceCommandDescriptor<'static> = ServiceCommandDescriptor {
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
) -> Result<PathBuf, String> {
    service_paths::resolve_existing_path(path, "audit", PathRequirement::File, context)
        .map_err(|error| error.replace("le chemin audit", "le chemin d'audit"))
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

pub fn run(
    command: &PlanCommand,
) -> Result<crate::reporting::CompletedReport, crate::reporting::ReportFailure> {
    service_command::execute_service_command(
        &command.service,
        PLAN_DESCRIPTOR,
        format!(
            "Plan Codex en cours depuis {} (timeout: {} secondes)...",
            command.audit_path.display(),
            command.service.timeout.as_secs()
        ),
        save_plan,
    )
}

pub fn run_silently(
    command: &PlanCommand,
) -> Result<crate::reporting::CompletedReport, crate::reporting::ReportFailure> {
    service_command::execute_service_command_silently(&command.service, PLAN_DESCRIPTOR, save_plan)
}

#[allow(dead_code)]
pub fn parse_args(args: &[String]) -> Result<PlanCommand, ParseOutcome> {
    let context = service_command::resolve_context().map_err(ParseOutcome::Error)?;
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

    let parse_context = service_command::prepare_parse_context_for_context(
        &common,
        PLAN_DESCRIPTOR,
        context.clone(),
    );
    let workspace_root = parse_context.workspace_root.clone();
    let audit_path = resolve_audit_file(audit_path.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande plan requiert un chemin d'audit. Exemple: cargo run -p app -- plan .audit\\audit.md"
                .to_string(),
        )
    })?, &parse_context.context)
    .map_err(ParseOutcome::Error)?;
    let prompt = build_prompt(&workspace_root, &audit_path, &parse_context.output_dir);
    let service = service_command::prepare_service_from_prompt(
        &common,
        &parse_context,
        PLAN_DESCRIPTOR,
        prompt,
    );

    Ok(PlanCommand {
        service,
        workspace_root,
        audit_path,
    })
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
        assert!(prompt.contains("final plan will be saved by the wrapper"));
        assert!(prompt.contains("C:\\dev\\rust_agent\\.plan"));
        assert!(prompt.contains("complete Markdown implementation handoff plan only"));
    }

    #[test]
    fn resolve_audit_file_rejects_directory() {
        let context = service_paths::ExecutionContext::from_workspace_root(
            std::env::current_dir().expect("cwd"),
        );
        let error = resolve_audit_file(std::env::temp_dir(), &context)
            .expect_err("un audit doit etre un fichier");

        assert!(error.contains("le chemin d'audit doit etre un fichier"));
    }
}
