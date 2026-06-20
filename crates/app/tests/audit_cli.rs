mod support;

use std::fs;
use std::time::Duration;

use app::{CliCommand, CodexMode, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT, ServiceCommandDispatch};

use support::{normalize_path, parse};

#[test]
fn audit_command_runs_end_to_end_and_executes_codex_from_the_workspace_root() {
    let workspace = support::temp_dir("audit_lifecycle");
    let codex_bin = support::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    fs::create_dir_all(&workspace).expect("creation du workspace");
    fs::create_dir_all(workspace.join("target-dir")).expect("creation du dossier cible");

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
        .args(["audit", "--target", "target-dir"]);

    let output = command.output().expect("execution de l'audit");
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
    assert!(logged.contains("$rust-refactor-audit"));
    assert!(logged.contains(&format!("cwd={}", normalize_path(&workspace))));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn parses_audit_command() {
    let command = parse(&["audit", "--verbose"]).expect("audit doit etre parse");

    let CliCommand::Service(ServiceCommandDispatch::Audit(audit)) = command else {
        panic!("la commande attendue est audit");
    };

    assert_eq!(audit.service.request.mode, CodexMode::Exec);
    assert_eq!(audit.service.request.model, DEFAULT_MODEL);
    assert_eq!(
        audit.service.request.reasoning_effort,
        DEFAULT_REASONING_EFFORT
    );
    assert!(audit.service.request.verbose);
    assert_eq!(
        normalize_path(&audit.target_dir),
        normalize_path(&audit.workspace_root)
    );
    assert!(audit.service.output_dir.ends_with(".audit"));
    assert_eq!(audit.service.timeout, Duration::from_secs(900));
}

#[test]
fn parses_audit_timeout_argument() {
    let command = parse(&["audit", "--timeout-seconds", "42"]).expect("audit timeout configurable");

    let CliCommand::Service(ServiceCommandDispatch::Audit(audit)) = command else {
        panic!("la commande attendue est audit");
    };

    assert_eq!(audit.service.timeout, Duration::from_secs(42));
}

#[test]
fn rejects_zero_audit_timeout() {
    let error = parse(&["audit", "--timeout-seconds", "0"]).expect_err("timeout nul refuse");

    assert_eq!(
        error,
        app::ParseOutcome::Error(
            "timeout invalide: 0. Valeur attendue: nombre de secondes positif".to_string()
        )
    );
}

#[test]
fn parses_audit_target_argument() {
    let command = parse(&["audit", "--target", "."]).expect("audit cible");

    let CliCommand::Service(ServiceCommandDispatch::Audit(audit)) = command else {
        panic!("la commande attendue est audit");
    };

    assert_eq!(
        normalize_path(&audit.target_dir),
        normalize_path(&audit.workspace_root)
    );
}

#[test]
fn rejects_positional_argument_for_audit() {
    let error = parse(&["audit", "foo"]).expect_err("audit sans argument libre");

    assert_eq!(
        error,
        app::ParseOutcome::Error("argument inattendu pour audit: foo".to_string())
    );
}

#[test]
fn rejects_duplicate_audit_model_option() {
    let error = parse(&["audit", "--model", "gpt-5.4", "--model", "gpt-5.5"])
        .expect_err("audit ne doit pas accepter deux modeles");

    assert_eq!(
        error,
        app::ParseOutcome::Error("l'option --model a deja ete fournie".to_string())
    );
}
