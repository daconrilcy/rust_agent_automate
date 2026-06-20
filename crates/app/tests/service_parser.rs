mod support;

pub use app::{ParseOutcome, parse_timeout};

use std::path::Path;
use std::time::Duration;

use app::{
    CliCommand, CodexMode, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ReasoningEffort,
    ServiceCommandDispatch, ServiceCommandOptions, parse_with_common_options,
};
use support::{command_kind, normalize_path, parse, request_for};

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
    for spec in app::registered_commands() {
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
    assert!(matches!(
        impl_alias,
        CliCommand::Service(ServiceCommandDispatch::ImplementationAudit(_))
    ));

    let loop_alias = parse(&["loop", "implementation", "."]).expect("alias loop");
    assert!(matches!(
        loop_alias,
        CliCommand::Service(ServiceCommandDispatch::FixLoop(_))
    ));
}

#[test]
fn every_registered_subcommand_parses_to_its_expected_variant() {
    let workflow_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("workflows")
        .join("refactor.json");
    let workflow_path = workflow_path.to_string_lossy().into_owned();
    let cases = [
        ("audit", vec!["audit".to_string()]),
        ("plan", vec!["plan".to_string(), "Cargo.toml".to_string()]),
        (
            "implementation-audit",
            vec!["implementation-audit".to_string(), "Cargo.toml".to_string()],
        ),
        (
            "review",
            vec![
                "review".to_string(),
                "implementation".to_string(),
                ".".to_string(),
            ],
        ),
        (
            "fix-loop",
            vec![
                "fix-loop".to_string(),
                "implementation".to_string(),
                ".".to_string(),
            ],
        ),
        (
            "automate",
            vec!["automate".to_string(), workflow_path.clone()],
        ),
        (
            "refactor-automate",
            vec![
                "refactor-automate".to_string(),
                "--target".to_string(),
                ".".to_string(),
                "Durcir".to_string(),
            ],
        ),
    ];

    for (expected, args) in cases {
        let command = parse(&args.iter().map(String::as_str).collect::<Vec<_>>())
            .expect("commande enregistree");
        assert_eq!(command_kind(&command), expected);
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
fn rejects_duplicate_run_model_option() {
    let error = parse(&["--model", "gpt-5.4", "--model", "gpt-5.5"])
        .expect_err("run ne doit pas accepter deux modeles");

    assert_eq!(
        error,
        ParseOutcome::Error("l'option --model a deja ete fournie".to_string())
    );
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
fn rejects_duplicate_automate_workflow_option() {
    let error = parse(&["automate", "--workflow", "a.json", "--workflow", "b.json"])
        .expect_err("automate ne doit pas accepter deux workflows nommes");

    assert_eq!(
        error,
        ParseOutcome::Error("le workflow automate a deja ete fourni".to_string())
    );
}

#[test]
fn rejects_duplicate_refactor_automate_target_option() {
    let error = parse(&[
        "refactor-automate",
        "--target",
        ".",
        "--target",
        ".",
        "Durcir",
    ])
    .expect_err("refactor-automate ne doit pas accepter deux cibles");

    assert_eq!(
        error,
        ParseOutcome::Error("la cible de refactor-automate a deja ete fournie".to_string())
    );
}

#[test]
fn rejects_duplicate_fix_loop_type_argument() {
    let error = parse(&[
        "fix-loop",
        "--type",
        "plan",
        "--type",
        "audit",
        "Cargo.toml",
    ])
    .expect_err("fix-loop ne doit pas accepter deux types");

    assert_eq!(
        error,
        ParseOutcome::Error("le type de fix-loop a deja ete fourni".to_string())
    );
}

#[test]
fn rejects_duplicate_fix_loop_artifact_argument() {
    let error = parse(&[
        "fix-loop",
        "--type",
        "plan",
        "--artifact",
        "Cargo.toml",
        "--artifact",
        "README.md",
    ])
    .expect_err("fix-loop ne doit pas accepter deux artefacts");

    assert_eq!(
        error,
        ParseOutcome::Error("l'artefact de fix-loop a deja ete fourni".to_string())
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
