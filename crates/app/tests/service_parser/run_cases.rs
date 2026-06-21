use super::*;

#[test]
fn shared_parser_handles_named_and_positional_inputs() {
    let command = parse(&[
        "review",
        "--type",
        "implementation",
        "Cargo.toml",
        "--timeout-seconds",
        "42",
        "--verbose",
    ])
    .expect("parse");
    let review = command.as_review().expect("review attendu");

    assert_eq!(review.subject, app::ReviewSubject::Implementation);
    assert_eq!(review.service.timeout, Duration::from_secs(42));
    assert!(review.service.request.verbose);
}

#[test]
fn empty_invocation_prints_help_instead_of_opening_codex() {
    let error = parse(&[]).expect_err("l'appel sans argument doit afficher l'aide");

    assert_eq!(error, ParseOutcome::Help);
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
fn parses_agent_context_extensions() {
    let command = parse(&[
        "--team",
        "--portable",
        "--docker",
        "--mode",
        "exec",
        "Analyse",
    ])
    .expect("contexte agentique parse");

    let CliCommand::Run(request) = command else {
        panic!("la commande attendue est run");
    };

    assert!(!request.agent_context.solo);
    assert!(!request.agent_context.windows_only);
    assert!(request.agent_context.portability);
    assert!(request.agent_context.docker);
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
            "implementation",
            "Cargo.toml",
            "--model",
            "gpt-5.6",
            "--reasoning",
            "medium",
        ],
        &[
            "fix-loop",
            "implementation",
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
fn service_commands_accept_agent_context_extensions() {
    let cases: &[&[&str]] = &[
        &["audit", "--team", "--portable", "--docker"],
        &["plan", "Cargo.toml", "--team", "--portable", "--docker"],
        &[
            "implementation-audit",
            "Cargo.toml",
            "--team",
            "--portable",
            "--docker",
        ],
        &[
            "review",
            "implementation",
            "Cargo.toml",
            "--team",
            "--portable",
            "--docker",
        ],
        &[
            "fix-loop",
            "implementation",
            "Cargo.toml",
            "--team",
            "--portable",
            "--docker",
        ],
    ];

    for case in cases {
        let command = parse(case).expect("la commande doit accepter le contexte agentique");
        let context = &request_for(&command).agent_context;

        assert!(!context.solo);
        assert!(!context.windows_only);
        assert!(context.portability);
        assert!(context.docker);
    }
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
fn rejects_duplicate_run_model_option() {
    let error = parse(&["--model", "gpt-5.4", "--model", "gpt-5.5"])
        .expect_err("run ne doit pas accepter deux modeles");

    assert_eq!(
        error,
        ParseOutcome::Error("l'option --model a deja ete fournie".to_string())
    );
}

#[test]
fn rejects_duplicate_run_reasoning_option() {
    let error = parse(&["--reasoning", "low", "--reasoning", "high"])
        .expect_err("run ne doit pas accepter deux niveaux de reasoning");

    assert_eq!(
        error,
        ParseOutcome::Error("l'option --reasoning a deja ete fournie".to_string())
    );
}
