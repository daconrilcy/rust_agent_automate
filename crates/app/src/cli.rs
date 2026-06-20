use std::time::Duration;

use crate::audit::AuditCommand;
use crate::automate::{self, AutomateCommand, RefactorAutomateCommand};
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

pub fn parse_args(args: &[String]) -> Result<CliCommand, ParseOutcome> {
    match command_registry::canonical_name(args.first().map(String::as_str)) {
        Some("audit") => return crate::audit::parse_args(&args[1..]).map(CliCommand::Audit),
        Some("plan") => return crate::plan::parse_args(&args[1..]).map(CliCommand::Plan),
        Some("implementation-audit") => {
            return crate::implementation_audit::parse_args(&args[1..])
                .map(CliCommand::ImplementationAudit);
        }
        Some("review") => return crate::review::parse_args(&args[1..]).map(CliCommand::Review),
        Some("fix-loop") => {
            return crate::fix_loop::parse_args(&args[1..]).map(CliCommand::FixLoop);
        }
        Some("automate") => {
            return automate::parse_automate_args(&args[1..]).map(CliCommand::Automate);
        }
        Some("refactor-automate") => {
            return automate::parse_refactor_automate_args(&args[1..])
                .map(CliCommand::RefactorAutomate);
        }
        Some(_) | None => {}
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
