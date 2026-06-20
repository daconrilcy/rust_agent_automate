mod support;

use std::fs;
use std::time::Duration;

use app::{CodexMode, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT};

use support::parse;

#[test]
fn implementation_audit_prompt_mentions_skill_and_default_scope() {
    let command =
        parse(&["implementation-audit", "Cargo.toml"]).expect("implementation-audit parse");
    let audit = command
        .as_implementation_audit()
        .expect("la commande attendue est implementation-audit");
    let prompt = audit
        .service
        .request
        .prompt
        .as_deref()
        .expect("prompt implementation-audit");

    assert!(prompt.contains("$rust-implementation-plan-audit"));
    assert!(prompt.contains("central Codex skill named rust-implementation-plan-audit"));
    assert!(prompt.contains("Cargo.toml"));
    assert!(prompt.contains("No explicit implementation path was provided"));
    assert!(prompt.contains("final audit report will be saved by the wrapper"));
    assert!(prompt.contains("complete Markdown Rust Implementation Plan Audit report only"));
}

#[test]
fn implementation_audit_prompt_mentions_explicit_scope() {
    let command = parse(&[
        "implementation-audit",
        "Cargo.toml",
        "--implementation",
        ".",
    ])
    .expect("implementation-audit scope explicite");
    let audit = command
        .as_implementation_audit()
        .expect("la commande attendue est implementation-audit");
    let prompt = audit
        .service
        .request
        .prompt
        .as_deref()
        .expect("prompt implementation-audit");

    assert!(prompt.contains("Review the implementation evidence at"));
    assert!(prompt.contains("C:\\dev\\rust_agent"));
}

#[test]
fn implementation_audit_rejects_directory_plan_path() {
    let error = parse(&["implementation-audit", "."]).expect_err("un plan doit etre un fichier");

    let app::ParseOutcome::Error(message) = error else {
        panic!("erreur de parse attendue");
    };
    assert!(message.contains("le chemin plan d'implementation doit etre un fichier"));
}

#[test]
fn implementation_audit_command_runs_end_to_end_and_saves_the_expected_artifact() {
    let workspace = support::temp_dir("implementation_audit_lifecycle");
    let codex_bin = support::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    let plan_path = workspace.join("plan.md");
    fs::create_dir_all(workspace.join("crates").join("app")).expect("creation de l'implementation");
    fs::write(&plan_path, "# Plan\n\n- placeholder").expect("ecriture du plan");

    let mut command = support::build_command();
    command
        .current_dir(&workspace)
        .env_remove("RUST_AGENT_WORKSPACE_ROOT")
        .env_remove("RUST_AGENT_USE_WORKSPACE_ROOT")
        .env(
            "PATH",
            support::join_path_dirs([codex_bin.parent().expect("bin parent").to_path_buf()]),
        )
        .env("USERPROFILE", &workspace)
        .env("FAKE_CODEX_LOG", &log_path)
        .args([
            "implementation-audit",
            plan_path.to_str().expect("plan path utf-8"),
            "--implementation",
            "crates\\app",
        ]);

    let output = command
        .output()
        .expect("execution de l'implementation-audit");
    assert!(output.status.success(), "sortie inattendue: {:?}", output);

    let audit_dir = workspace.join(".audit");
    let mut report_paths = fs::read_dir(&audit_dir)
        .expect("lecture du dossier .audit")
        .map(|entry| entry.expect("entree valide").path())
        .collect::<Vec<_>>();
    report_paths.sort();
    assert_eq!(report_paths.len(), 1, "un seul audit doit etre genere");

    let report = fs::read_to_string(&report_paths[0]).expect("lecture de l'audit genere");
    assert!(report.contains("fake final message"));

    let logged = fs::read_to_string(&log_path).expect("lecture du log codex");
    assert!(logged.contains("$rust-implementation-plan-audit"));
    assert!(logged.contains("Review the implementation evidence at"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn parses_implementation_audit_command_with_positional_plan_path() {
    let command = parse(&["implementation-audit", "Cargo.toml"])
        .expect("implementation-audit doit etre parse");
    let audit = command
        .as_implementation_audit()
        .expect("la commande attendue est implementation-audit");

    assert_eq!(audit.service.request.mode, CodexMode::Exec);
    assert_eq!(audit.service.request.model, DEFAULT_MODEL);
    assert_eq!(
        audit.service.request.reasoning_effort,
        DEFAULT_REASONING_EFFORT
    );
    assert!(audit.service.output_dir.ends_with(".audit"));
    assert_eq!(audit.service.timeout, Duration::from_secs(900));
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
    let audit = command
        .as_implementation_audit()
        .expect("la commande attendue est implementation-audit");

    assert_eq!(audit.service.timeout, Duration::from_secs(42));
    assert!(audit.service.request.verbose);
    assert!(
        audit
            .implementation_path
            .as_ref()
            .is_some_and(|path| path.is_dir())
    );
}

#[test]
fn rejects_implementation_audit_without_plan_path() {
    let error = parse(&["implementation-audit"]).expect_err("implementation-audit sans plan");

    assert_eq!(
        error,
        app::ParseOutcome::Error(
            "la commande implementation-audit requiert un plan. Exemple: cargo run -p app -- implementation-audit .plan\\plan.md"
                .to_string()
        )
    );
}
