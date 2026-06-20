use std::time::Duration;

use crate::audit::AuditCommand;
use crate::automate::{AutomateCommand, RefactorAutomateCommand};
use crate::codex::{CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT};
use crate::command_registry;
use crate::fix_loop::FixLoopCommand;
use crate::implementation_audit::ImplementationAuditCommand;
use crate::plan::PlanCommand;
use crate::review::ReviewCommand;

#[derive(Debug, PartialEq, Eq)]
pub enum CliCommand {
    Run(CodexRequest),
    Audit(AuditCommand),
    Plan(PlanCommand),
    ImplementationAudit(ImplementationAuditCommand),
    Review(ReviewCommand),
    FixLoop(FixLoopCommand),
    Automate(AutomateCommand),
    RefactorAutomate(RefactorAutomateCommand),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseOutcome {
    Help,
    Error(String),
}

impl CliCommand {
    pub fn execute(self) -> i32 {
        match self {
            Self::Run(request) => run_request(&request),
            Self::Audit(command) => {
                run_service_command(crate::run_audit(&command));
                0
            }
            Self::Plan(command) => {
                run_service_command(crate::run_plan(&command));
                0
            }
            Self::ImplementationAudit(command) => {
                run_service_command(crate::run_implementation_audit(&command));
                0
            }
            Self::Review(command) => {
                run_service_command(crate::run_review(&command));
                0
            }
            Self::FixLoop(command) => {
                run_service_command(crate::run_fix_loop(&command));
                0
            }
            Self::Automate(command) => run_automate(&command),
            Self::RefactorAutomate(command) => run_refactor_automate(&command),
        }
    }
}

pub fn parse_args(args: &[String]) -> Result<CliCommand, ParseOutcome> {
    if let Some(result) = command_registry::parse_registered_subcommand(args) {
        return result;
    }

    parse_run_args(args).map(CliCommand::Run)
}

pub fn print_help() {
    println!("Usage:");
    println!(
        "  cargo run -p app -- [--model <nom>] [--reasoning <low|medium|high>] [--mode <interactive|exec>] [--verbose] [prompt]"
    );
    for line in command_registry::usage_lines() {
        println!("  {line}");
    }
    println!();
    println!("Exemples:");
    println!("  cargo run -q -p app -- --model gpt-5.4 --reasoning low");
    println!(
        "  cargo run -q -p app -- --mode exec --model gpt-5.4 --reasoning low \"Explique ce depot\""
    );
    println!(
        "  cargo run -q -p app -- --mode exec --verbose --model gpt-5.4 --reasoning low \"Explique ce depot\""
    );
    for line in command_registry::example_lines() {
        println!("  {line}");
    }
}

pub fn next_value<'a>(
    args: &'a [String],
    index: usize,
    option_name: &str,
) -> Result<&'a str, ParseOutcome> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| ParseOutcome::Error(format!("valeur manquante pour {option_name}")))
}

pub fn parse_timeout(value: &str) -> Result<Duration, ParseOutcome> {
    let seconds = value.parse::<u64>().map_err(|_| {
        ParseOutcome::Error(format!(
            "timeout invalide: {value}. Valeur attendue: nombre de secondes positif"
        ))
    })?;

    if seconds == 0 {
        return Err(ParseOutcome::Error(
            "timeout invalide: 0. Valeur attendue: nombre de secondes positif".to_string(),
        ));
    }

    Ok(Duration::from_secs(seconds))
}

fn parse_run_args(args: &[String]) -> Result<CodexRequest, ParseOutcome> {
    let mut model = String::from(DEFAULT_MODEL);
    let mut reasoning_effort = DEFAULT_REASONING_EFFORT;
    let mut mode = CodexMode::Interactive;
    let mut verbose = false;
    let mut resume_last = false;
    let mut prompt_parts: Vec<String> = Vec::new();

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
            "--mode" => {
                let value = next_value(args, index, "--mode")?;
                mode = value.parse().map_err(ParseOutcome::Error)?;
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
            _ => {
                prompt_parts.extend_from_slice(&args[index..]);
                break;
            }
        }
    }

    let prompt = if prompt_parts.is_empty() {
        None
    } else {
        Some(prompt_parts.join(" "))
    };

    if mode == CodexMode::Exec && prompt.is_none() {
        return Err(ParseOutcome::Error(
            "le mode exec requiert un prompt. Exemple: cargo run -p app -- --mode exec \"Ecris un resume du projet\""
                .to_string(),
        ));
    }

    Ok(
        CodexRequest::new(model, reasoning_effort, mode, prompt, verbose)
            .with_resume_last(resume_last),
    )
}

fn run_service_command(
    result: Result<crate::reporting::CompletedReport, crate::reporting::ReportFailure>,
) {
    if let Err(error) = result {
        let code = service_command_exit_code(&error);
        std::process::exit(code);
    }
}

fn service_command_exit_code(error: &crate::reporting::ReportFailure) -> i32 {
    match error {
        crate::reporting::ReportFailure::MissingFinalMessage { status_code, .. } => {
            crate::codex::process_exit_code(Some(*status_code))
        }
        _ => 1,
    }
}

fn run_request(request: &CodexRequest) -> i32 {
    match crate::codex::run(request) {
        Ok(result) => {
            if result.status.success() {
                if let Some(message) = result.final_message {
                    println!("{message}");
                }
                0
            } else {
                let stderr = result.stderr.trim();
                let stdout = result.stdout.trim();
                if !stderr.is_empty() {
                    eprintln!("{stderr}");
                } else if !stdout.is_empty() {
                    eprintln!("{stdout}");
                }
                crate::codex::process_exit_code(result.status.code())
            }
        }
        Err(error) => {
            eprintln!("echec lors de l'appel a codex: {error}");
            1
        }
    }
}

fn run_automate(command: &AutomateCommand) -> i32 {
    eprintln!(
        "Automate Codex depuis {} sur {}...",
        command.workflow_path.display(),
        command.workspace_root.display()
    );
    run_automate_workflow(
        &command.workflow,
        &command.initial_prompt,
        &command.workspace_root,
        &command.workspace_root,
    )
}

fn run_refactor_automate(command: &RefactorAutomateCommand) -> i32 {
    eprintln!(
        "Automate de refactoring depuis {} sur {}...",
        command.launch_workspace_root.display(),
        command.target_dir.display()
    );
    run_automate_workflow(
        &command.workflow,
        &command.initial_prompt,
        &command.output_root,
        &command.target_dir,
    )
}

fn run_automate_workflow(
    workflow: &crate::automate::Workflow,
    initial_prompt: &str,
    workspace_root: &std::path::Path,
    target_dir: &std::path::Path,
) -> i32 {
    match crate::automate::run_workflow(workflow, initial_prompt, workspace_root, target_dir) {
        Ok(report) => {
            println!(
                "Automate termine apres {} cycle(s){}.",
                report.completed_cycles,
                if report.clean_stop {
                    " (audit d'alignement sans correction actionnable detectee)"
                } else {
                    ""
                }
            );

            for result in report.step_results {
                let artifact = result
                    .artifact_path
                    .as_ref()
                    .map(|path| format!(" -> {}", path.display()))
                    .unwrap_or_default();
                println!(
                    "- cycle {} / {}: statut {:?}{}",
                    result.cycle, result.name, result.status_code, artifact
                );
            }
            0
        }
        Err(error) => {
            eprintln!("echec de l'automate: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::service_command_exit_code;
    use crate::reporting::ReportFailure;

    #[test]
    fn missing_final_message_uses_codex_status_code_mapping() {
        let code = service_command_exit_code(&ReportFailure::MissingFinalMessage {
            status_code: 7,
            stdout: String::new(),
            stderr: String::new(),
        });

        assert_eq!(code, 7);
    }

    #[test]
    fn missing_final_message_without_status_uses_generic_failure_code() {
        let code = service_command_exit_code(&ReportFailure::MissingFinalMessage {
            status_code: 1,
            stdout: String::new(),
            stderr: String::new(),
        });

        assert_eq!(code, 1);
    }

    #[test]
    fn codex_call_failures_exit_with_generic_failure_code() {
        let code = service_command_exit_code(&ReportFailure::CodexCall("boom".to_string()));

        assert_eq!(code, 1);
    }

    #[test]
    fn save_failures_exit_with_generic_failure_code() {
        let code = service_command_exit_code(&ReportFailure::Save {
            message: "rapport".to_string(),
            clean: Some(false),
            error: "disk full".to_string(),
        });

        assert_eq!(code, 1);
    }
}
