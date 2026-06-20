mod support;

use std::fs;
use std::time::Duration;

use app::{CliCommand, CodexMode, ReviewSubject, ServiceCommandDispatch};

use support::parse;

#[test]
fn fix_loop_command_runs_end_to_end_and_saves_the_expected_artifact() {
    let workspace = support::temp_dir("fix_loop_lifecycle");
    let codex_bin = support::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    let audit_path = workspace.join("audit.md");
    fs::write(&audit_path, "# Audit\n\n- placeholder").expect("ecriture de l'audit");

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
            "fix-loop",
            "audit",
            audit_path.to_str().expect("audit path utf-8"),
        ]);

    let output = command.output().expect("execution de fix-loop");
    assert!(output.status.success(), "sortie inattendue: {:?}", output);

    let loop_dir = workspace.join(".fix-loop");
    let mut report_paths = fs::read_dir(&loop_dir)
        .expect("lecture du dossier .fix-loop")
        .map(|entry| entry.expect("entree valide").path())
        .collect::<Vec<_>>();
    report_paths.sort();
    assert_eq!(report_paths.len(), 1, "un seul rapport doit etre genere");

    let report = fs::read_to_string(&report_paths[0]).expect("lecture du rapport genere");
    assert!(report.contains("fake final message"));

    let logged = fs::read_to_string(&log_path).expect("lecture du log codex");
    assert!(logged.contains("$rust-review-fix-loop"));
    assert!(logged.contains("Input kind: audit"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn parses_fix_loop_command_with_positional_type_and_artifact() {
    let command = parse(&["fix-loop", "plan", "Cargo.toml"]).expect("fix-loop parse");

    let CliCommand::Service(ServiceCommandDispatch::FixLoop(fix_loop)) = command else {
        panic!("la commande attendue est fix-loop");
    };

    assert_eq!(fix_loop.service.request.mode, CodexMode::Exec);
    assert_eq!(fix_loop.input_kind, ReviewSubject::Plan);
    assert!(fix_loop.service.output_dir.ends_with(".fix-loop"));
    assert_eq!(fix_loop.service.timeout, Duration::from_secs(1800));
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

    let CliCommand::Service(ServiceCommandDispatch::FixLoop(fix_loop)) = command else {
        panic!("la commande attendue est fix-loop");
    };

    assert_eq!(fix_loop.input_kind, ReviewSubject::Implementation);
    assert_eq!(fix_loop.service.timeout, Duration::from_secs(42));
    assert!(fix_loop.service.request.verbose);
}

#[test]
fn rejects_fix_loop_without_type() {
    let error = parse(&["fix-loop"]).expect_err("fix-loop sans type");

    assert_eq!(
        error,
        app::ParseOutcome::Error(
            "la commande fix-loop requiert un type: plan, audit ou implementation".to_string()
        )
    );
}

#[test]
fn rejects_fix_loop_without_artifact() {
    let error = parse(&["fix-loop", "audit"]).expect_err("fix-loop sans artefact");

    assert_eq!(
        error,
        app::ParseOutcome::Error(
            "la commande fix-loop requiert un artefact. Exemple: cargo run -p app -- fix-loop plan .plan\\plan.md"
                .to_string()
        )
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
        app::ParseOutcome::Error("le type de fix-loop a deja ete fourni".to_string())
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
        app::ParseOutcome::Error("l'artefact de fix-loop a deja ete fourni".to_string())
    );
}

#[test]
fn rejects_invalid_fix_loop_input_kind_with_command_specific_error() {
    let error = parse(&["fix-loop", "design", "Cargo.toml"]).expect_err("type invalide");

    assert_eq!(
        error,
        app::ParseOutcome::Error(
            "type d'entree fix-loop invalide: design. Valeurs attendues: plan, audit, implementation"
                .to_string()
        )
    );
}
