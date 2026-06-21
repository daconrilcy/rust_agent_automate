use super::*;

#[test]
fn static_commands_reject_service_level_codex_options() {
    let workflow_path = refactor_workflow_path();

    let automate = parse(&["automate", "--model", "gpt-5.6", workflow_path.as_str()])
        .expect_err("automate ne doit pas accepter les options Codex des commandes service");
    assert_eq!(
        automate,
        ParseOutcome::Error("option inconnue: --model".to_string())
    );

    let refactor = parse(&["refactor-auto", "--model", "gpt-5.6", "Durcir"]).expect_err(
        "refactor-automate ne doit pas accepter les options Codex des commandes service",
    );
    assert_eq!(
        refactor,
        ParseOutcome::Error("option inconnue: --model".to_string())
    );
}

#[test]
fn registered_automate_aliases_resolve_to_expected_variants() {
    let workflow_path = refactor_workflow_path();

    let automate = parse(&["automate", workflow_path.as_str()]).expect("automate doit etre parse");
    assert!(matches!(automate, CliCommand::Automate(_)));

    let refactor_alias =
        parse(&["refactor-auto", "--target", ".", "Durcir"]).expect("alias refactor");
    assert!(matches!(refactor_alias, CliCommand::RefactorAutomate(_)));
}

#[test]
fn automate_commands_parse_to_expected_variants() {
    let workflow_path = refactor_workflow_path();
    let cases = [
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
    assert!(command.agent_context.solo);
    assert!(command.agent_context.windows_only);
    assert_eq!(command.workflow.steps[0].name, "audit");
}

#[test]
fn parses_refactor_automate_agent_context_extensions() {
    let command = parse(&[
        "refactor-automate",
        "--target",
        ".",
        "--team",
        "--portable",
        "--docker",
        "Durcir",
    ])
    .expect("refactor-automate parse avec contexte");

    let CliCommand::RefactorAutomate(command) = command else {
        panic!("la commande attendue est refactor-automate");
    };

    assert!(!command.agent_context.solo);
    assert!(!command.agent_context.windows_only);
    assert!(command.agent_context.portability);
    assert!(command.agent_context.docker);
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
