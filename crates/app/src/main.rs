use std::process;

use app::automate::{self, AutomateCommand};
use app::cli::{self, CliCommand};
use app::codex::{self, CodexRequest};
use app::{audit, fix_loop, implementation_audit, plan, review};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match cli::parse_args(&args) {
        Ok(CliCommand::Run(request)) => run_request(&request),
        Ok(CliCommand::Audit(request)) => run_service_command(audit::run(&request)),
        Ok(CliCommand::Plan(request)) => run_service_command(plan::run(&request)),
        Ok(CliCommand::ImplementationAudit(request)) => {
            run_service_command(implementation_audit::run(&request))
        }
        Ok(CliCommand::Review(request)) => run_service_command(review::run(&request)),
        Ok(CliCommand::FixLoop(request)) => run_service_command(fix_loop::run(&request)),
        Ok(CliCommand::Automate(request)) => run_automate(&request),
        Ok(CliCommand::RefactorAutomate(request)) => run_refactor_automate(&request),
        Err(cli::ParseOutcome::Help) => cli::print_help(),
        Err(cli::ParseOutcome::Error(message)) => {
            eprintln!("{message}");
            eprintln!();
            cli::print_help();
            process::exit(1);
        }
    }
}

fn run_service_command(
    result: Result<app::reporting::CompletedReport, app::reporting::ReportFailure>,
) {
    if let Err(error) = result {
        let code = service_command_exit_code(&error);
        process::exit(code);
    }
}

fn service_command_exit_code(error: &app::reporting::ReportFailure) -> i32 {
    match error {
        app::reporting::ReportFailure::MissingFinalMessage { status_code, .. } => {
            codex::process_exit_code(Some(*status_code))
        }
        _ => 1,
    }
}

fn run_request(request: &CodexRequest) {
    match codex::run(request) {
        Ok(result) => {
            if result.status.success() {
                if let Some(message) = result.final_message {
                    println!("{message}");
                }
            } else {
                let stderr = result.stderr.trim();
                let stdout = result.stdout.trim();
                if !stderr.is_empty() {
                    eprintln!("{stderr}");
                } else if !stdout.is_empty() {
                    eprintln!("{stdout}");
                }
                process::exit(codex::process_exit_code(result.status.code()));
            }
        }
        Err(error) => {
            eprintln!("echec lors de l'appel a codex: {error}");
            process::exit(1);
        }
    }
}

fn run_automate(command: &AutomateCommand) {
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
    );
}

fn run_refactor_automate(command: &automate::RefactorAutomateCommand) {
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
    );
}

fn run_automate_workflow(
    workflow: &automate::Workflow,
    initial_prompt: &str,
    workspace_root: &std::path::Path,
    target_dir: &std::path::Path,
) {
    match automate::run_workflow(workflow, initial_prompt, workspace_root, target_dir) {
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
        }
        Err(error) => {
            eprintln!("echec de l'automate: {error}");
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::service_command_exit_code;
    use app::reporting::ReportFailure;

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
