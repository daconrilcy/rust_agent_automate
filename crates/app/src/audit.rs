use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact;
use crate::codex::CodexRequest;
use crate::service_command::{self, ServiceCommandOptions, ServiceRunSpec};
use crate::service_paths::{self, PathRequirement};
use crate::{ParseOutcome, next_value};

#[derive(Debug, PartialEq, Eq)]
pub struct AuditCommand {
    pub request: CodexRequest,
    pub workspace_root: PathBuf,
    pub target_dir: PathBuf,
    pub output_dir: PathBuf,
    pub timeout: Duration,
}

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

pub fn resolve_target_dir(path: PathBuf, context: &service_paths::ExecutionContext) -> Result<PathBuf, String> {
    service_paths::resolve_existing_path(path, "dossier cible", PathRequirement::Directory, &context)
        .map_err(|error| error.replace("le chemin dossier cible", "le chemin cible"))
}

pub fn run(command: &AuditCommand) {
    service_command::run_service_command(
        &command.request,
        command.timeout,
        ServiceRunSpec {
            intro: format!(
                "Audit Codex en cours sur {} (timeout: {} secondes)...",
                command.target_dir.display(),
                command.timeout.as_secs()
            ),
            command_name: "audit",
            saved_label: "audit",
            final_label: "audit",
            missing_message_label: "rapport d'audit",
            output_dir: &command.output_dir,
            save: save_report,
            clean_detector: None,
        },
    );
}

pub fn save_report(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    artifact::save_timestamped_markdown(output_dir, "audit", content)
}

pub fn parse_args(args: &[String]) -> Result<AuditCommand, ParseOutcome> {
    let mut common = ServiceCommandOptions::new(Duration::from_secs(900));
    let mut target_dir: Option<PathBuf> = None;

    let mut index = 0;
    while index < args.len() {
        if let Some(consumed) = service_command::parse_common_option(args, index, &mut common)? {
            index += consumed;
            continue;
        }

        match args[index].as_str() {
            "--target" => {
                let value = next_value(args, index, "--target")?;
                target_dir = Some(PathBuf::from(value));
                index += 2;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            value => {
                return Err(ParseOutcome::Error(format!(
                    "argument inattendu pour audit: {value}"
                )));
            }
        }
    }

    let context = service_command::resolve_context().map_err(ParseOutcome::Error)?;
    let workspace_root = context.workspace_root().to_path_buf();
    let target_dir = resolve_target_dir(target_dir.unwrap_or_else(|| workspace_root.clone()), &context)
        .map_err(ParseOutcome::Error)?;
    let output_dir = service_command::resolve_output_dir(&common, &context, ".audit");
    let prompt = build_prompt(&workspace_root, &target_dir, &output_dir);

    Ok(AuditCommand {
        request: common.build_request(prompt),
        workspace_root,
        target_dir,
        output_dir,
        timeout: common.timeout,
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
