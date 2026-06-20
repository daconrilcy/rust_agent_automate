mod artifact;
mod audit;
mod automate;
mod codex;
mod command_registry;
mod fix_loop;
mod implementation_audit;
mod plan;
mod reporting;
mod review;
mod service_paths;

use std::env;
use std::path::{Path, PathBuf};
use std::process;
use std::time::Duration;

use audit::AuditCommand;
use automate::{AutomateCommand, RefactorAutomateCommand};
use codex::{CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT};
use fix_loop::FixLoopCommand;
use implementation_audit::ImplementationAuditCommand;
use plan::PlanCommand;
use review::ReviewCommand;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match parse_args(&args) {
        Ok(CliCommand::Run(request)) => run_request(&request),
        Ok(CliCommand::Audit(request)) => audit::run(&request),
        Ok(CliCommand::Plan(request)) => plan::run(&request),
        Ok(CliCommand::ImplementationAudit(request)) => implementation_audit::run(&request),
        Ok(CliCommand::Review(request)) => review::run(&request),
        Ok(CliCommand::FixLoop(request)) => fix_loop::run(&request),
        Ok(CliCommand::Automate(request)) => run_automate(&request),
        Ok(CliCommand::RefactorAutomate(request)) => run_refactor_automate(&request),
        Err(ParseOutcome::Help) => print_help(),
        Err(ParseOutcome::Error(message)) => {
            eprintln!("{message}");
            eprintln!();
            print_help();
            process::exit(1);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum CliCommand {
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
pub(crate) enum ParseOutcome {
    Help,
    Error(String),
}

fn parse_args(args: &[String]) -> Result<CliCommand, ParseOutcome> {
    match command_registry::canonical_name(args.first().map(String::as_str)) {
        Some("audit") => return audit::parse_args(&args[1..]).map(CliCommand::Audit),
        Some("plan") => return plan::parse_args(&args[1..]).map(CliCommand::Plan),
        Some("implementation-audit") => {
            return implementation_audit::parse_args(&args[1..])
                .map(CliCommand::ImplementationAudit);
        }
        Some("review") => return review::parse_args(&args[1..]).map(CliCommand::Review),
        Some("fix-loop") => return fix_loop::parse_args(&args[1..]).map(CliCommand::FixLoop),
        Some("automate") => return parse_automate_args(&args[1..]).map(CliCommand::Automate),
        Some("refactor-automate") => {
            return parse_refactor_automate_args(&args[1..]).map(CliCommand::RefactorAutomate);
        }
        Some(_) | None => {}
    }

    parse_run_args(args).map(CliCommand::Run)
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

fn parse_automate_args(args: &[String]) -> Result<AutomateCommand, ParseOutcome> {
    let mut workflow_path: Option<PathBuf> = None;
    let mut prompt_parts: Vec<String> = Vec::new();

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--workflow" => {
                let value = next_value(args, index, "--workflow")?;
                if workflow_path.is_some() {
                    return Err(ParseOutcome::Error(
                        "le workflow automate a deja ete fourni".to_string(),
                    ));
                }
                workflow_path = Some(PathBuf::from(value));
                index += 2;
            }
            value if value.starts_with("--") => {
                return Err(ParseOutcome::Error(format!("option inconnue: {value}")));
            }
            value => {
                if workflow_path.is_none() {
                    workflow_path = Some(PathBuf::from(value));
                    index += 1;
                    continue;
                }

                prompt_parts.extend_from_slice(&args[index..]);
                break;
            }
        }
    }

    let workflow_path = workflow_path.ok_or_else(|| {
        ParseOutcome::Error(
            "la commande automate requiert un workflow JSON. Exemple: cargo run -p app -- automate workflow.json \"Objectif\""
                .to_string(),
        )
    })?;
    let workflow = automate::load_workflow(&workflow_path).map_err(ParseOutcome::Error)?;
    let initial_prompt = if prompt_parts.is_empty() {
        String::from("Executer le workflow automate fourni.")
    } else {
        prompt_parts.join(" ")
    };

    Ok(AutomateCommand {
        workflow_path,
        workflow,
        initial_prompt,
    })
}

fn parse_refactor_automate_args(args: &[String]) -> Result<RefactorAutomateCommand, ParseOutcome> {
    let mut workflow_path: Option<PathBuf> = None;
    let mut target_dir: Option<PathBuf> = None;
    let mut prompt_parts: Vec<String> = Vec::new();

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-h" | "--help" => return Err(ParseOutcome::Help),
            "--workflow" => {
                let value = next_value(args, index, "--workflow")?;
                workflow_path = Some(PathBuf::from(value));
                index += 2;
            }
            "--target" => {
                let value = next_value(args, index, "--target")?;
                target_dir = Some(PathBuf::from(value));
                index += 2;
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

    let workflow = match workflow_path {
        Some(path) => automate::load_workflow(&path).map_err(ParseOutcome::Error)?,
        None => automate::default_refactor_workflow(),
    };
    let workspace_root = env::current_dir().map_err(|error| {
        ParseOutcome::Error(format!("impossible de lire le repertoire courant: {error}"))
    })?;
    let target_dir = automate::resolve_target_dir(target_dir.unwrap_or(workspace_root))
        .map_err(ParseOutcome::Error)?;
    let initial_prompt = if prompt_parts.is_empty() {
        format!(
            "Refactorer {} pour ameliorer structure, maintenabilite, evolutivite et robustesse en respectant SOLID, YAGNI, KISS et DRY.",
            target_dir.display()
        )
    } else {
        prompt_parts.join(" ")
    };

    Ok(RefactorAutomateCommand {
        workflow,
        initial_prompt,
        target_dir,
    })
}

pub(crate) fn next_value<'a>(
    args: &'a [String],
    index: usize,
    option_name: &str,
) -> Result<&'a str, ParseOutcome> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| ParseOutcome::Error(format!("valeur manquante pour {option_name}")))
}

fn print_help() {
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
    let target_dir = match env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("impossible de lire le repertoire courant: {error}");
            process::exit(1);
        }
    };

    eprintln!(
        "Automate Codex depuis {} sur {}...",
        command.workflow_path.display(),
        target_dir.display()
    );
    run_automate_workflow(&command.workflow, &command.initial_prompt, &target_dir);
}

fn run_refactor_automate(command: &RefactorAutomateCommand) {
    eprintln!(
        "Automate de refactoring sur {}...",
        command.target_dir.display()
    );
    run_automate_workflow(
        &command.workflow,
        &command.initial_prompt,
        &command.target_dir,
    );
}

fn run_automate_workflow(workflow: &automate::Workflow, initial_prompt: &str, target_dir: &Path) {
    match automate::run_workflow(workflow, initial_prompt, target_dir) {
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

pub(crate) fn parse_timeout(value: &str) -> Result<Duration, ParseOutcome> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::ReasoningEffort;
    use crate::review::ReviewSubject;

    fn normalize_path(path: &Path) -> String {
        std::fs::canonicalize(path)
            .unwrap_or_else(|_| path.to_path_buf())
            .display()
            .to_string()
            .replace("\\\\?\\", "")
    }

    fn parse(input: &[&str]) -> Result<CliCommand, ParseOutcome> {
        let args = input
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>();
        parse_args(&args)
    }

    fn request_for(command: &CliCommand) -> &CodexRequest {
        match command {
            CliCommand::Run(request) => request,
            CliCommand::Audit(command) => &command.request,
            CliCommand::Plan(command) => &command.request,
            CliCommand::ImplementationAudit(command) => &command.request,
            CliCommand::Review(command) => &command.request,
            CliCommand::FixLoop(command) => &command.request,
            CliCommand::Automate(_) | CliCommand::RefactorAutomate(_) => {
                panic!("les automates ne portent pas de requete Codex directe")
            }
        }
    }

    #[test]
    fn parses_defaults() {
        let command = parse(&[]).expect("la configuration par defaut doit etre valide");

        let CliCommand::Run(request) = command else {
            panic!("la commande par defaut doit etre le mode run");
        };

        assert_eq!(request.model, DEFAULT_MODEL);
        assert_eq!(request.reasoning_effort, DEFAULT_REASONING_EFFORT);
        assert_eq!(request.mode, CodexMode::Interactive);
        assert_eq!(request.prompt, None);
        assert!(!request.verbose);
    }

    #[test]
    fn parses_all_options() {
        let command = parse(&[
            "--model",
            "gpt-5.5",
            "--reasoning",
            "high",
            "--mode",
            "exec",
            "Analyse",
            "ce",
            "repo",
        ])
        .expect("les options doivent etre parsees");

        let CliCommand::Run(request) = command else {
            panic!("la commande attendue est run");
        };

        assert_eq!(request.model, "gpt-5.5");
        assert_eq!(request.reasoning_effort, ReasoningEffort::High);
        assert_eq!(request.mode, CodexMode::Exec);
        assert_eq!(request.prompt.as_deref(), Some("Analyse ce repo"));
        assert!(!request.verbose);
    }

    #[test]
    fn codex_commands_accept_model_and_reasoning_overrides() {
        let cases: &[&[&str]] = &[
            &["--model", "gpt-5.6", "--reasoning", "medium"],
            &["audit", "--model", "gpt-5.6", "--reasoning", "medium"],
            &[
                "plan",
                "Cargo.toml",
                "--model",
                "gpt-5.6",
                "--reasoning",
                "medium",
            ],
            &[
                "implementation-audit",
                "Cargo.toml",
                "--model",
                "gpt-5.6",
                "--reasoning",
                "medium",
            ],
            &[
                "review",
                "audit",
                "Cargo.toml",
                "--model",
                "gpt-5.6",
                "--reasoning",
                "medium",
            ],
            &[
                "fix-loop",
                "plan",
                "Cargo.toml",
                "--model",
                "gpt-5.6",
                "--reasoning",
                "medium",
            ],
        ];

        for case in cases {
            let command = parse(case).expect("la commande doit accepter model et reasoning");
            let request = request_for(&command);

            assert_eq!(request.model, "gpt-5.6");
            assert_eq!(request.reasoning_effort, ReasoningEffort::Medium);
        }
    }

    #[test]
    fn rejects_exec_without_prompt() {
        let error = parse(&["--mode", "exec"]).expect_err("exec sans prompt doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "le mode exec requiert un prompt. Exemple: cargo run -p app -- --mode exec \"Ecris un resume du projet\""
                    .to_string()
            )
        );
    }

    #[test]
    fn keeps_prompt_spacing() {
        let command = parse(&["--mode", "exec", "Resume", "ce", "projet"])
            .expect("le prompt doit etre reconstruit");

        let CliCommand::Run(request) = command else {
            panic!("la commande attendue est run");
        };

        assert_eq!(request.prompt.as_deref(), Some("Resume ce projet"));
    }

    #[test]
    fn parses_verbose_flag() {
        let command = parse(&["--verbose", "--mode", "exec", "Resume"])
            .expect("le flag verbose doit etre parse");

        let CliCommand::Run(request) = command else {
            panic!("la commande attendue est run");
        };

        assert!(request.verbose);
    }

    #[test]
    fn parses_audit_command() {
        let command = parse(&["audit", "--verbose"]).expect("audit doit etre parse");

        let CliCommand::Audit(audit) = command else {
            panic!("la commande attendue est audit");
        };

        assert_eq!(audit.request.mode, CodexMode::Exec);
        assert_eq!(audit.request.model, DEFAULT_MODEL);
        assert_eq!(audit.request.reasoning_effort, DEFAULT_REASONING_EFFORT);
        assert!(audit.request.verbose);
        assert_eq!(
            normalize_path(&audit.target_dir),
            normalize_path(&audit.workspace_root)
        );
        assert!(audit.output_dir.ends_with(".audit"));
        assert_eq!(audit.timeout, Duration::from_secs(900));
        assert!(audit.request.prompt.as_deref().is_some_and(|prompt| {
            prompt.contains("$rust-refactor-audit")
                && prompt.contains("central Codex skill named rust-refactor-audit")
                && !prompt.contains(".agents/skills/rust-refactor-audit")
                && prompt.contains(&audit.target_dir.display().to_string())
                && prompt.contains(&audit.output_dir.display().to_string())
        }));
    }

    #[test]
    fn parses_audit_timeout_argument() {
        let command = parse(&["audit", "--timeout-seconds", "42"])
            .expect("audit doit accepter un timeout configurable");

        let CliCommand::Audit(audit) = command else {
            panic!("la commande attendue est audit");
        };

        assert_eq!(audit.timeout, Duration::from_secs(42));
    }

    #[test]
    fn rejects_zero_audit_timeout() {
        let error = parse(&["audit", "--timeout-seconds", "0"])
            .expect_err("un timeout nul doit etre refuse");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "timeout invalide: 0. Valeur attendue: nombre de secondes positif".to_string()
            )
        );
    }

    #[test]
    fn parses_audit_target_argument() {
        let command =
            parse(&["audit", "--target", "."]).expect("audit doit accepter un dossier cible");

        let CliCommand::Audit(audit) = command else {
            panic!("la commande attendue est audit");
        };

        assert_eq!(
            normalize_path(&audit.target_dir),
            normalize_path(&audit.workspace_root)
        );
    }

    #[test]
    fn parses_plan_command_with_positional_audit_path() {
        let command = parse(&["plan", "Cargo.toml"]).expect("plan doit etre parse");

        let CliCommand::Plan(plan) = command else {
            panic!("la commande attendue est plan");
        };

        assert_eq!(plan.request.mode, CodexMode::Exec);
        assert_eq!(plan.request.model, DEFAULT_MODEL);
        assert_eq!(plan.request.reasoning_effort, DEFAULT_REASONING_EFFORT);
        assert!(plan.output_dir.ends_with(".plan"));
        assert_eq!(plan.timeout, Duration::from_secs(900));
        assert!(plan.request.prompt.as_deref().is_some_and(|prompt| {
            prompt.contains("$refactor-plan-from-audit")
                && prompt.contains("central Codex skill named refactor-plan-from-audit")
                && prompt.contains("complete Markdown implementation handoff plan only")
        }));
    }

    #[test]
    fn parses_implementation_audit_command_with_positional_plan_path() {
        let command = parse(&["implementation-audit", "Cargo.toml"])
            .expect("implementation-audit doit etre parse");

        let CliCommand::ImplementationAudit(audit) = command else {
            panic!("la commande attendue est implementation-audit");
        };

        assert_eq!(audit.request.mode, CodexMode::Exec);
        assert_eq!(audit.request.model, DEFAULT_MODEL);
        assert_eq!(audit.request.reasoning_effort, DEFAULT_REASONING_EFFORT);
        assert!(audit.output_dir.ends_with(".audit"));
        assert_eq!(audit.timeout, Duration::from_secs(900));
        assert!(audit.implementation_path.is_none());
        assert!(audit.request.prompt.as_deref().is_some_and(|prompt| {
            prompt.contains("$rust-implementation-plan-audit")
                && prompt.contains("central Codex skill named rust-implementation-plan-audit")
                && prompt.contains("No explicit implementation path was provided")
                && prompt.contains("complete Markdown Rust Implementation Plan Audit report only")
        }));
    }

    #[test]
    fn parses_implementation_audit_command_with_named_options() {
        let command = parse(&[
            "impl-audit",
            "--plan",
            "Cargo.toml",
            "--implementation",
            ".",
            "--timeout-seconds",
            "42",
            "--verbose",
        ])
        .expect("impl-audit doit accepter les options nommees");

        let CliCommand::ImplementationAudit(audit) = command else {
            panic!("la commande attendue est implementation-audit");
        };

        assert_eq!(audit.timeout, Duration::from_secs(42));
        assert!(audit.request.verbose);
        assert!(
            audit
                .implementation_path
                .as_ref()
                .is_some_and(|path| path.is_dir())
        );
        assert!(
            audit
                .request
                .prompt
                .as_deref()
                .is_some_and(|prompt| { prompt.contains("Review the implementation evidence at") })
        );
    }

    #[test]
    fn parses_review_command_with_positional_type_and_artifact() {
        let command = parse(&["review", "implementation", "."]).expect("review doit etre parse");

        let CliCommand::Review(review) = command else {
            panic!("la commande attendue est review");
        };

        assert_eq!(review.request.mode, CodexMode::Exec);
        assert_eq!(review.request.model, DEFAULT_MODEL);
        assert_eq!(review.request.reasoning_effort, DEFAULT_REASONING_EFFORT);
        assert_eq!(review.subject, ReviewSubject::Implementation);
        assert!(review.output_dir.ends_with(".review"));
        assert_eq!(review.timeout, Duration::from_secs(900));
        assert!(review.request.prompt.as_deref().is_some_and(|prompt| {
            prompt.contains("$adversarial-review")
                && prompt.contains("central Codex skill named adversarial-review")
                && prompt.contains("Review mode: Implementation review")
                && prompt.contains("complete Markdown adversarial review only")
        }));
    }

    #[test]
    fn parses_review_command_with_named_type_and_artifact() {
        let command = parse(&[
            "review",
            "--type",
            "audit",
            "--artifact",
            "Cargo.toml",
            "--timeout-seconds",
            "42",
            "--verbose",
        ])
        .expect("review doit accepter les options nommees");

        let CliCommand::Review(review) = command else {
            panic!("la commande attendue est review");
        };

        assert_eq!(review.subject, ReviewSubject::Audit);
        assert_eq!(review.timeout, Duration::from_secs(42));
        assert!(review.request.verbose);
    }

    #[test]
    fn parses_fix_loop_command_with_positional_type_and_artifact() {
        let command = parse(&["fix-loop", "plan", "Cargo.toml"]).expect("fix-loop doit etre parse");

        let CliCommand::FixLoop(fix_loop) = command else {
            panic!("la commande attendue est fix-loop");
        };

        assert_eq!(fix_loop.request.mode, CodexMode::Exec);
        assert_eq!(fix_loop.request.model, DEFAULT_MODEL);
        assert_eq!(fix_loop.request.reasoning_effort, DEFAULT_REASONING_EFFORT);
        assert_eq!(fix_loop.input_kind, ReviewSubject::Plan);
        assert!(fix_loop.output_dir.ends_with(".fix-loop"));
        assert_eq!(fix_loop.timeout, Duration::from_secs(1800));
        assert!(fix_loop.request.prompt.as_deref().is_some_and(|prompt| {
            prompt.contains("$rust-review-fix-loop")
                && prompt.contains("central Codex skill named rust-review-fix-loop")
                && prompt.contains("Input kind: plan")
                && prompt.contains("rust-dev-solid")
                && prompt.contains("adversarial-review cycles until no actionable findings remain")
        }));
    }

    #[test]
    fn parses_fix_loop_alias_with_named_type_and_artifact() {
        let command = parse(&[
            "loop",
            "--type",
            "implementation",
            "--artifact",
            ".",
            "--timeout-seconds",
            "42",
            "--verbose",
        ])
        .expect("loop doit accepter les options nommees");

        let CliCommand::FixLoop(fix_loop) = command else {
            panic!("la commande attendue est fix-loop");
        };

        assert_eq!(fix_loop.input_kind, ReviewSubject::Implementation);
        assert_eq!(fix_loop.timeout, Duration::from_secs(42));
        assert!(fix_loop.request.verbose);
    }

    #[test]
    fn parses_refactor_automate_with_default_workflow() {
        let command = parse(&["refactor-automate", "--target", ".", "Durcir", "le", "code"])
            .expect("refactor-automate doit etre parse");

        let CliCommand::RefactorAutomate(command) = command else {
            panic!("la commande attendue est refactor-automate");
        };

        assert_eq!(
            normalize_path(&command.target_dir),
            normalize_path(&env::current_dir().expect("cwd"))
        );
        assert_eq!(command.initial_prompt, "Durcir le code");
        assert_eq!(command.workflow.steps[0].name, "audit");
    }

    #[test]
    fn parses_refactor_automate_alias() {
        let command = parse(&["refactor-auto", "--target", ".", "Durcir", "le", "code"])
            .expect("refactor-auto doit etre parse");

        let CliCommand::RefactorAutomate(command) = command else {
            panic!("la commande attendue est refactor-automate");
        };

        assert_eq!(command.initial_prompt, "Durcir le code");
        assert_eq!(command.workflow.steps[0].name, "audit");
    }

    #[test]
    fn rejects_automate_without_workflow() {
        let error = parse(&["automate"]).expect_err("automate sans workflow doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande automate requiert un workflow JSON. Exemple: cargo run -p app -- automate workflow.json \"Objectif\""
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_fix_loop_without_type() {
        let error = parse(&["fix-loop"]).expect_err("fix-loop sans type doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande fix-loop requiert un type: plan, audit ou implementation".to_string()
            )
        );
    }

    #[test]
    fn rejects_fix_loop_without_artifact() {
        let error = parse(&["fix-loop", "audit"]).expect_err("fix-loop sans artefact doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande fix-loop requiert un artefact. Exemple: cargo run -p app -- fix-loop plan .plan\\plan.md"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_invalid_fix_loop_input_kind_with_command_specific_error() {
        let error =
            parse(&["fix-loop", "design", "Cargo.toml"]).expect_err("type invalide doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "type d'entree fix-loop invalide: design. Valeurs attendues: plan, audit, implementation"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_review_without_type() {
        let error = parse(&["review"]).expect_err("review sans type doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande review requiert un type: plan, audit ou implementation".to_string()
            )
        );
    }

    #[test]
    fn rejects_review_without_artifact() {
        let error = parse(&["review", "plan"]).expect_err("review sans artefact doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande review requiert un artefact. Exemple: cargo run -p app -- review plan .plan\\plan.md"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_duplicate_plan_audit_argument() {
        let error = parse(&["plan", "--audit", "Cargo.toml", "--audit", "README.md"])
            .expect_err("plan ne doit pas accepter deux chemins d'audit");

        assert_eq!(
            error,
            ParseOutcome::Error("l'audit a deja ete fourni pour la commande plan".to_string())
        );
    }

    #[test]
    fn rejects_plan_without_audit_path() {
        let error = parse(&["plan"]).expect_err("plan sans audit doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande plan requiert un chemin d'audit. Exemple: cargo run -p app -- plan .audit\\audit.md"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_implementation_audit_without_plan_path() {
        let error = parse(&["implementation-audit"])
            .expect_err("implementation-audit sans plan doit echouer");

        assert_eq!(
            error,
            ParseOutcome::Error(
                "la commande implementation-audit requiert un plan. Exemple: cargo run -p app -- implementation-audit .plan\\plan.md"
                    .to_string()
            )
        );
    }

    #[test]
    fn rejects_positional_argument_for_audit() {
        let error =
            parse(&["audit", "foo"]).expect_err("audit ne doit pas accepter d'argument libre");

        assert_eq!(
            error,
            ParseOutcome::Error("argument inattendu pour audit: foo".to_string())
        );
    }
}
