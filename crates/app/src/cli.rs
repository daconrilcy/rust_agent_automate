use std::time::Duration;

use crate::automate::{AutomateCommand, RefactorAutomateCommand};
use crate::codex::{
    AgentContext, CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT,
};
use crate::command_registry;
use crate::service_command::ServiceCommandDispatch;

#[derive(Debug, PartialEq, Eq)]
pub enum CliCommand {
    Run(CodexRequest),
    Service(ServiceCommandDispatch),
    Automate(AutomateCommand),
    RefactorAutomate(RefactorAutomateCommand),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseOutcome {
    Help,
    Error(String),
}

pub enum ParseLoopControl {
    Continue(usize),
    CaptureRest,
}

impl CliCommand {
    pub fn service(&self) -> Option<&ServiceCommandDispatch> {
        match self {
            Self::Service(command) => Some(command),
            _ => None,
        }
    }

    pub fn as_audit(&self) -> Option<&crate::audit::AuditCommand> {
        self.service()?.as_audit()
    }

    pub fn as_plan(&self) -> Option<&crate::plan::PlanCommand> {
        self.service()?.as_plan()
    }

    pub fn as_implementation_audit(
        &self,
    ) -> Option<&crate::implementation_audit::ImplementationAuditCommand> {
        self.service()?.as_implementation_audit()
    }

    pub fn as_review(&self) -> Option<&crate::review::ReviewCommand> {
        self.service()?.as_review()
    }

    pub fn as_fix_loop(&self) -> Option<&crate::fix_loop::FixLoopCommand> {
        self.service()?.as_fix_loop()
    }

    pub fn execute(self) -> i32 {
        match self {
            Self::Run(request) => run_request(&request),
            Self::Service(command) => run_service_command(command.execute()),
            Self::Automate(command) => run_automate(&command),
            Self::RefactorAutomate(command) => run_refactor_automate(&command),
        }
    }
}

pub fn parse_args(args: &[String]) -> Result<CliCommand, ParseOutcome> {
    if args.is_empty() {
        return Err(ParseOutcome::Help);
    }

    // CLI routing stops at choosing direct-run versus a registered subcommand.
    // Subcommand-specific parsing stays in the registry and service modules.
    if let Some(result) = command_registry::parse_registered_subcommand(args) {
        return result;
    }

    parse_run_args(args).map(CliCommand::Run)
}

pub fn print_help() {
    println!("Usage:");
    for line in command_registry::DIRECT_RUN_USAGE {
        println!("  {line}");
    }
    for line in command_registry::usage_lines() {
        println!("  {line}");
    }
    println!();
    println!("Exemples:");
    for line in command_registry::DIRECT_RUN_EXAMPLES {
        println!("  {line}");
    }
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

pub fn mark_seen(seen: &mut bool, option_name: &str) -> Result<(), ParseOutcome> {
    mark_seen_with_message(seen, &format!("l'option {option_name} a deja ete fournie"))
}

pub fn mark_seen_with_message(seen: &mut bool, message: &str) -> Result<(), ParseOutcome> {
    if *seen {
        return Err(ParseOutcome::Error(message.to_string()));
    }

    *seen = true;
    Ok(())
}

pub fn scan_args<F>(args: &[String], mut handler: F) -> Result<Option<usize>, ParseOutcome>
where
    F: FnMut(usize, &str) -> Result<ParseLoopControl, ParseOutcome>,
{
    let mut index = 0;
    while index < args.len() {
        match handler(index, args[index].as_str())? {
            ParseLoopControl::Continue(consumed) => {
                if consumed == 0 {
                    return Err(ParseOutcome::Error(format!(
                        "le parseur partage doit consommer au moins un argument: {}",
                        args[index]
                    )));
                }
                index += consumed;
            }
            ParseLoopControl::CaptureRest => return Ok(Some(index)),
        }
    }

    Ok(None)
}

fn parse_run_args(args: &[String]) -> Result<CodexRequest, ParseOutcome> {
    let mut model = String::from(DEFAULT_MODEL);
    let mut reasoning_effort = DEFAULT_REASONING_EFFORT;
    let mut mode = CodexMode::Interactive;
    let mut verbose = false;
    let mut resume_last = false;
    let mut agent_context = AgentContext::default();
    let mut seen_model = false;
    let mut seen_reasoning = false;
    let mut seen_mode = false;
    let mut prompt_parts: Vec<String> = Vec::new();

    let prompt_start = scan_args(args, |index, value| match value {
        "-h" | "--help" => Err(ParseOutcome::Help),
        "--model" => {
            mark_seen(&mut seen_model, "--model")?;
            let value = next_value(args, index, "--model")?;
            model = value.to_owned();
            Ok(ParseLoopControl::Continue(2))
        }
        "--reasoning" => {
            mark_seen(&mut seen_reasoning, "--reasoning")?;
            let value = next_value(args, index, "--reasoning")?;
            reasoning_effort = value.parse().map_err(ParseOutcome::Error)?;
            Ok(ParseLoopControl::Continue(2))
        }
        "--mode" => {
            mark_seen(&mut seen_mode, "--mode")?;
            let value = next_value(args, index, "--mode")?;
            mode = value.parse().map_err(ParseOutcome::Error)?;
            Ok(ParseLoopControl::Continue(2))
        }
        "--verbose" => {
            verbose = true;
            Ok(ParseLoopControl::Continue(1))
        }
        "--continue-codex" => {
            resume_last = true;
            Ok(ParseLoopControl::Continue(1))
        }
        value if agent_context.apply_cli_flag(value) => Ok(ParseLoopControl::Continue(1)),
        value if value.starts_with("--") => {
            Err(ParseOutcome::Error(format!("option inconnue: {value}")))
        }
        _ => Ok(ParseLoopControl::CaptureRest),
    })?;

    if let Some(index) = prompt_start {
        prompt_parts.extend_from_slice(&args[index..]);
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
            .with_resume_last(resume_last)
            .with_agent_context(agent_context),
    )
}

fn run_service_command(
    result: Result<crate::reporting::CompletedReport, crate::reporting::ReportFailure>,
) -> i32 {
    match result {
        Ok(report) => crate::codex::process_exit_code(Some(report.status_code)),
        Err(error) => service_command_exit_code(&error),
    }
}

fn service_command_exit_code(error: &crate::reporting::ReportFailure) -> i32 {
    crate::reporting::command_failure_exit_code(error)
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
        &command.agent_context,
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
        &command.agent_context,
    )
}

fn run_automate_workflow(
    workflow: &crate::automate::Workflow,
    initial_prompt: &str,
    workspace_root: &std::path::Path,
    target_dir: &std::path::Path,
    agent_context: &AgentContext,
) -> i32 {
    match crate::automate::run_workflow(
        workflow,
        initial_prompt,
        workspace_root,
        target_dir,
        agent_context,
    ) {
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
