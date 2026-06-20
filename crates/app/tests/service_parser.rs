pub use app::cli::{ParseOutcome, parse_args, parse_timeout};

use std::path::Path;
use std::time::Duration;

use app::cli::CliCommand;
use app::codex::{
    CodexMode, CodexRequest, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ReasoningEffort,
};
use app::command_registry;
use app::review::ReviewSubject;
use app::service_command::{ServiceCommandOptions, parse_with_common_options};

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
fn shared_parser_handles_named_and_positional_inputs() {
    let args = [
        "plan.md",
        "--artifact",
        "impl",
        "--timeout-seconds",
        "42",
        "--verbose",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    let mut common = ServiceCommandOptions::new(Duration::from_secs(900));
    let mut artifact = None;

    parse_with_common_options(&args, &mut common, |index, value| {
        if value == "--artifact" {
            artifact = Some(args[index + 1].clone());
            return Ok(2);
        }
        Ok(1)
    })
    .expect("parse");

    assert_eq!(artifact.as_deref(), Some("impl"));
    assert_eq!(common.timeout, Duration::from_secs(42));
    assert!(common.verbose);
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
fn every_registered_subcommand_routes_help() {
    for spec in command_registry::COMMANDS {
        let command = vec![spec.name, "--help"];
        let result = parse(&command);
        assert_eq!(
            result,
            Err(ParseOutcome::Help),
            "route help manquante pour {}",
            spec.name
        );

        for alias in spec.aliases {
            let alias_command = vec![*alias, "--help"];
            let result = parse(&alias_command);
            assert_eq!(
                result,
                Err(ParseOutcome::Help),
                "route help manquante pour {}",
                alias
            );
        }
    }
}

#[test]
fn registered_aliases_resolve_to_expected_command_variants() {
    let workflow_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("workflows")
        .join("refactor.json");
    let workflow_path = workflow_path.to_string_lossy().into_owned();

    let automate = parse(&["automate", workflow_path.as_str()]).expect("automate doit etre parse");
    assert!(matches!(automate, CliCommand::Automate(_)));

    let refactor_alias =
        parse(&["refactor-auto", "--target", ".", "Durcir"]).expect("alias refactor");
    assert!(matches!(refactor_alias, CliCommand::RefactorAutomate(_)));

    let impl_alias = parse(&["impl-audit", "Cargo.toml"]).expect("alias impl-audit");
    assert!(matches!(impl_alias, CliCommand::ImplementationAudit(_)));

    let loop_alias = parse(&["loop", "implementation", "."]).expect("alias loop");
    assert!(matches!(loop_alias, CliCommand::FixLoop(_)));
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
    let command = parse(&["--mode", "exec", "Resume", "ce", "projet"]).expect("prompt reconstruit");

    let CliCommand::Run(request) = command else {
        panic!("la commande attendue est run");
    };

    assert_eq!(request.prompt.as_deref(), Some("Resume ce projet"));
}

#[test]
fn parses_verbose_flag() {
    let command = parse(&["--verbose", "--mode", "exec", "Resume"]).expect("flag verbose parse");

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
}

#[test]
fn parses_audit_timeout_argument() {
    let command = parse(&["audit", "--timeout-seconds", "42"]).expect("audit timeout configurable");

    let CliCommand::Audit(audit) = command else {
        panic!("la commande attendue est audit");
    };

    assert_eq!(audit.timeout, Duration::from_secs(42));
}

#[test]
fn rejects_zero_audit_timeout() {
    let error = parse(&["audit", "--timeout-seconds", "0"]).expect_err("timeout nul refuse");

    assert_eq!(
        error,
        ParseOutcome::Error(
            "timeout invalide: 0. Valeur attendue: nombre de secondes positif".to_string()
        )
    );
}

#[test]
fn parses_audit_target_argument() {
    let command = parse(&["audit", "--target", "."]).expect("audit cible");

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
    .expect("impl-audit options nommees");

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
}

#[test]
fn parses_review_command_with_positional_type_and_artifact() {
    let command = parse(&["review", "implementation", "."]).expect("review parse");

    let CliCommand::Review(review) = command else {
        panic!("la commande attendue est review");
    };

    assert_eq!(review.request.mode, CodexMode::Exec);
    assert_eq!(review.subject, ReviewSubject::Implementation);
    assert!(review.output_dir.ends_with(".review"));
    assert_eq!(review.timeout, Duration::from_secs(900));
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
    .expect("review options nommees");

    let CliCommand::Review(review) = command else {
        panic!("la commande attendue est review");
    };

    assert_eq!(review.subject, ReviewSubject::Audit);
    assert_eq!(review.timeout, Duration::from_secs(42));
    assert!(review.request.verbose);
}

#[test]
fn parses_fix_loop_command_with_positional_type_and_artifact() {
    let command = parse(&["fix-loop", "plan", "Cargo.toml"]).expect("fix-loop parse");

    let CliCommand::FixLoop(fix_loop) = command else {
        panic!("la commande attendue est fix-loop");
    };

    assert_eq!(fix_loop.request.mode, CodexMode::Exec);
    assert_eq!(fix_loop.input_kind, ReviewSubject::Plan);
    assert!(fix_loop.output_dir.ends_with(".fix-loop"));
    assert_eq!(fix_loop.timeout, Duration::from_secs(1800));
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
    .expect("loop options nommees");

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
        .expect("refactor-automate parse");

    let CliCommand::RefactorAutomate(command) = command else {
        panic!("la commande attendue est refactor-automate");
    };

    assert_eq!(
        normalize_path(&command.output_root),
        normalize_path(&command.target_dir)
    );
    assert_eq!(command.initial_prompt, "Durcir le code");
    assert_eq!(command.workflow.steps[0].name, "audit");
}

#[test]
fn parses_refactor_automate_alias() {
    let command = parse(&["refactor-auto", "--target", ".", "Durcir", "le", "code"])
        .expect("refactor-auto parse");

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
    let error = parse(&["fix-loop"]).expect_err("fix-loop sans type");

    assert_eq!(
        error,
        ParseOutcome::Error(
            "la commande fix-loop requiert un type: plan, audit ou implementation".to_string()
        )
    );
}

#[test]
fn rejects_fix_loop_without_artifact() {
    let error = parse(&["fix-loop", "audit"]).expect_err("fix-loop sans artefact");

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
    let error = parse(&["fix-loop", "design", "Cargo.toml"]).expect_err("type invalide");

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
    let error = parse(&["review"]).expect_err("review sans type");

    assert_eq!(
        error,
        ParseOutcome::Error(
            "la commande review requiert un type: plan, audit ou implementation".to_string()
        )
    );
}

#[test]
fn rejects_review_without_artifact() {
    let error = parse(&["review", "plan"]).expect_err("review sans artefact");

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
    let error = parse(&["plan"]).expect_err("plan sans audit");

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
    let error = parse(&["implementation-audit"]).expect_err("implementation-audit sans plan");

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
    let error = parse(&["audit", "foo"]).expect_err("audit sans argument libre");

    assert_eq!(
        error,
        ParseOutcome::Error("argument inattendu pour audit: foo".to_string())
    );
}
