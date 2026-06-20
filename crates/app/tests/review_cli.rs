mod support;

use std::fs;
use std::time::Duration;

use app::{CliCommand, CodexMode, ReviewSubject, ServiceCommandDispatch};

use support::parse;

#[test]
fn review_command_runs_end_to_end_and_saves_the_expected_artifact() {
    let workspace = support::temp_dir("review_lifecycle");
    let codex_bin = support::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    let plan_path = workspace.join("plan.md");
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
            "review",
            "plan",
            plan_path.to_str().expect("plan path utf-8"),
        ]);

    let output = command.output().expect("execution de review");
    assert!(output.status.success(), "sortie inattendue: {:?}", output);

    let review_dir = workspace.join(".review");
    let mut report_paths = fs::read_dir(&review_dir)
        .expect("lecture du dossier .review")
        .map(|entry| entry.expect("entree valide").path())
        .collect::<Vec<_>>();
    report_paths.sort();
    assert_eq!(report_paths.len(), 1, "un seul review doit etre genere");

    let report = fs::read_to_string(&report_paths[0]).expect("lecture du review genere");
    assert!(report.contains("fake final message"));

    let logged = fs::read_to_string(&log_path).expect("lecture du log codex");
    assert!(logged.contains("$adversarial-review"));
    assert!(logged.contains("Review mode: Plan review"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn parses_review_command_with_positional_type_and_artifact() {
    let command = parse(&["review", "implementation", "."]).expect("review parse");

    let CliCommand::Service(ServiceCommandDispatch::Review(review)) = command else {
        panic!("la commande attendue est review");
    };

    assert_eq!(review.service.request.mode, CodexMode::Exec);
    assert_eq!(review.subject, ReviewSubject::Implementation);
    assert!(review.service.output_dir.ends_with(".review"));
    assert_eq!(review.service.timeout, Duration::from_secs(900));
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

    let CliCommand::Service(ServiceCommandDispatch::Review(review)) = command else {
        panic!("la commande attendue est review");
    };

    assert_eq!(review.subject, ReviewSubject::Audit);
    assert_eq!(review.service.timeout, Duration::from_secs(42));
    assert!(review.service.request.verbose);
}

#[test]
fn rejects_duplicate_review_type_argument() {
    let error = parse(&["review", "--type", "plan", "--type", "audit", "Cargo.toml"])
        .expect_err("review ne doit pas accepter deux types");

    assert_eq!(
        error,
        app::ParseOutcome::Error("le type de review a deja ete fourni".to_string())
    );
}

#[test]
fn rejects_duplicate_review_artifact_argument() {
    let error = parse(&[
        "review",
        "--type",
        "plan",
        "--artifact",
        "Cargo.toml",
        "--artifact",
        "README.md",
    ])
    .expect_err("review ne doit pas accepter deux artefacts");

    assert_eq!(
        error,
        app::ParseOutcome::Error("l'artefact de review a deja ete fourni".to_string())
    );
}

#[test]
fn rejects_review_without_type() {
    let error = parse(&["review"]).expect_err("review sans type");

    assert_eq!(
        error,
        app::ParseOutcome::Error(
            "la commande review requiert un type: plan, audit ou implementation".to_string()
        )
    );
}

#[test]
fn rejects_review_without_artifact() {
    let error = parse(&["review", "plan"]).expect_err("review sans artefact");

    assert_eq!(
        error,
        app::ParseOutcome::Error(
            "la commande review requiert un artefact. Exemple: cargo run -p app -- review plan .plan\\plan.md"
                .to_string()
        )
    );
}
