use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::artifact;
use crate::codex::{CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT};
use crate::reporting::{self, ReportSpec};
use crate::service_paths::{self, PathRequirement};
use crate::{ParseOutcome, next_value, parse_timeout};

#[derive(Debug, PartialEq, Eq)]
pub struct ImplementationAuditCommand {
    pub request: CodexRequest,
    pub workspace_root: PathBuf,
    pub plan_path: PathBuf,
    pub implementation_path: Option<PathBuf>,
    pub output_dir: PathBuf,
    pub timeout: Duration,
}

pub fn resolve_plan_file(path: PathBuf) -> Result<PathBuf, String> {
    service_paths::resolve_existing_path(path, "plan d'implementation", PathRequirement::File)
}

pub fn resolve_implementation_path(path: PathBuf) -> Result<PathBuf, String> {
    service_paths::resolve_existing_path(path, "implementation", PathRequirement::FileOrDirectory)
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

    format!(
        concat!(
            "Use $rust-implementation-plan-audit to audit whether the Rust implementation follows the implementation plan at \"{}\".\n",
            "The audit must use the central Codex skill named rust-implementation-plan-audit and follow its SKILL.md instructions.\n",
            "The current local workspace running this command is \"{}\" and the final audit report will be saved by the wrapper under \"{}\".\n",
            "{}\n",
            "Read the implementation plan completely before judging the code. Build a requirements checklist from the plan, gather direct local evidence, and map every plan item to implementation status.\n",
            "Do not modify source code. Produce the final answer as a complete Markdown Rust Implementation Plan Audit report only, using the report structure required by the rust-implementation-plan-audit skill."
        ),
        plan_path.display(),
        workspace_root.display(),
        output_dir.display(),
        implementation_scope
    )
}

pub fn save_audit(output_dir: &Path, content: &str) -> io::Result<PathBuf> {
    artifact::save_timestamped_markdown(output_dir, "implementation-audit", content)
}

pub fn run(command: &ImplementationAuditCommand) {
    let scope = command.implementation_path.as_deref().map_or_else(
        || "git diff / workspace".to_string(),
        |path| path.display().to_string(),
    );

    eprintln!(
        "Audit d'implementation Codex en cours depuis {} sur {} (timeout: {} secondes)...",
        command.plan_path.display(),
        scope,
        command.timeout.as_secs()
    );

    reporting::run_codex_report(
        &command.request,
        command.timeout,
        ReportSpec {
            command_name: "implementation-audit",
            saved_label: "audit d'implementation",
            final_label: "audit d'implementation",
            missing_message_label: "audit d'implementation",
            output_dir: &command.output_dir,
            save: save_audit,
            clean_detector: Some(reporting::detect_clean_implementation_audit),
        },
    );
}

pub fn parse_args(args: &[String]) -> Result<ImplementationAuditCommand, ParseOutcome> {
    let mut model = String::from(DEFAULT_MODEL);
    let mut reasoning_effort = DEFAULT_REASONING_EFFORT;
    let mut verbose = false;
    let mut resume_last = false;
    let mut plan_path: Option<PathBuf> = None;
    let mut implementation_path: Option<PathBuf> = None;
    let mut output_dir: Option<PathBuf> = None;
    let mut timeout = Duration::from_secs(900);

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--model" => {
                let value = next_value(args, index, "--model")?;
                model = value.to_owned();
                index += 2;
            }
            "--reasoning" => {
                let value = next_value(args, index, "--reasoning")?;
                reasoning_effort = value.parse().map_err(ParseOutcome::Error)?;
                index += 2;
            }
            "--plan" => {
                let value = next_value(args, index, "--plan")?;
                if plan_path.is_some() {
                    return Err(ParseOutcome::Error(
                        "le plan d'implementation a deja ete fourni".to_string(),
                    ));
                }
                plan_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--implementation" => {
                let value = next_value(args, index, "--implementation")?;
                if implementation_path.is_some() {
                    return Err(ParseOutcome::Error(
                        "le chemin d'implementation a deja ete fourni".to_string(),
                    ));
                }
                implementation_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--output-dir" => {
                let value = next_value(args, index, "--output-dir")?;
                output_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--timeout-seconds" => {
                let value = next_value(args, index, "--timeout-seconds")?;
                timeout = parse_timeout(value)?;
                index += 2;
            }
            "--verbose" => {
                verbose = true;
                index += 1;
            }
            "--continue-codex" => {
                resume_last = true;
                index += 1;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            value => {
                if plan_path.is_some() {
                    return Err(ParseOutcome::Error(format!(
                        "argument inattendu pour implementation-audit: {value}"
                    )));
                }
                plan_path = Some(PathBuf::from(value));
                index += 1;
            }
        }
    }

    let plan_path = plan_path.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande implementation-audit requiert un plan. Exemple: cargo run -p app -- implementation-audit .plan\\plan.md"
                .to_string(),
        )
    })?;
    let workspace_root = service_paths::current_workspace_root().map_err(ParseOutcome::Error)?;
    let plan_path = resolve_plan_file(plan_path).map_err(ParseOutcome::Error)?;
    let implementation_path = implementation_path
        .map(resolve_implementation_path)
        .transpose()
        .map_err(ParseOutcome::Error)?;
    let output_dir = service_paths::resolve_output_dir(output_dir, &workspace_root, ".audit");
    let prompt = build_prompt(
        &workspace_root,
        &plan_path,
        implementation_path.as_deref(),
        &output_dir,
    );

    Ok(ImplementationAuditCommand {
        request: CodexRequest::new(
            model,
            reasoning_effort,
            CodexMode::Exec,
            Some(prompt),
            verbose,
        )
        .with_resume_last(resume_last),
        workspace_root,
        plan_path,
        implementation_path,
        output_dir,
        timeout,
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
        let error =
            resolve_plan_file(std::env::temp_dir()).expect_err("un plan doit etre un fichier");

        assert!(error.contains("le chemin plan d'implementation doit etre un fichier"));
    }

    #[test]
    fn resolve_implementation_path_accepts_directory() {
        let path = resolve_implementation_path(std::env::temp_dir())
            .expect("implementation doit accepter un dossier");

        assert!(path.is_dir());
    }
}
