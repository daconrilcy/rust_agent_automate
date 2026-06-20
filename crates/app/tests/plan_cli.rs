mod support;

use std::fs;
use std::time::Duration;

use app::{CodexMode, DEFAULT_MODEL, DEFAULT_REASONING_EFFORT};

use support::parse;

#[test]
fn plan_prompt_mentions_skill_and_paths() {
    let command = parse(&["plan", "Cargo.toml"]).expect("plan parse");
    let plan = command.as_plan().expect("la commande attendue est plan");
    let prompt = plan.service.request.prompt.as_deref().expect("prompt plan");

    assert!(prompt.contains("$refactor-plan-from-audit"));
    assert!(prompt.contains("central Codex skill named refactor-plan-from-audit"));
    assert!(prompt.contains("references/plan-template.md"));
    assert!(prompt.contains("Cargo.toml"));
    assert!(prompt.contains("final plan will be saved by the wrapper"));
    assert!(prompt.contains("complete Markdown implementation handoff plan only"));
}

#[test]
fn plan_rejects_directory_audit_paths() {
    let error = parse(&["plan", "."]).expect_err("un audit doit etre un fichier");

    let app::ParseOutcome::Error(message) = error else {
        panic!("erreur de parse attendue");
    };
    assert!(message.contains("le chemin d'audit doit etre un fichier"));
}

#[test]
fn plan_command_runs_end_to_end_and_saves_the_expected_artifact() {
    let workspace = support::temp_dir("plan_lifecycle");
    let codex_bin = support::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    let audit_path = workspace.join("audit.md");
    fs::create_dir_all(&workspace).expect("creation du workspace");
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
        .args(["plan", audit_path.to_str().expect("audit path utf-8")]);

    let output = command.output().expect("execution du plan");
    assert!(output.status.success(), "sortie inattendue: {:?}", output);

    let plan_dir = workspace.join(".plan");
    let mut report_paths = fs::read_dir(&plan_dir)
        .expect("lecture du dossier .plan")
        .map(|entry| entry.expect("entree valide").path())
        .collect::<Vec<_>>();
    report_paths.sort();
    assert_eq!(report_paths.len(), 1, "un seul plan doit etre genere");

    let report = fs::read_to_string(&report_paths[0]).expect("lecture du plan genere");
    assert!(report.contains("fake final message"));

    let logged = fs::read_to_string(&log_path).expect("lecture du log codex");
    assert!(logged.contains("$refactor-plan-from-audit"));
    assert!(logged.contains("references/plan-template.md"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn parses_plan_command_with_positional_audit_path() {
    let command = parse(&["plan", "Cargo.toml"]).expect("plan doit etre parse");
    let plan = command.as_plan().expect("la commande attendue est plan");

    assert_eq!(plan.service.request.mode, CodexMode::Exec);
    assert_eq!(plan.service.request.model, DEFAULT_MODEL);
    assert_eq!(
        plan.service.request.reasoning_effort,
        DEFAULT_REASONING_EFFORT
    );
    assert!(plan.service.output_dir.ends_with(".plan"));
    assert_eq!(plan.service.timeout, Duration::from_secs(900));
}

#[test]
fn rejects_duplicate_plan_audit_argument() {
    let error = parse(&["plan", "--audit", "Cargo.toml", "--audit", "README.md"])
        .expect_err("plan ne doit pas accepter deux chemins d'audit");

    assert_eq!(
        error,
        app::ParseOutcome::Error("l'audit a deja ete fourni pour la commande plan".to_string())
    );
}

#[test]
fn rejects_plan_without_audit_path() {
    let error = parse(&["plan"]).expect_err("plan sans audit");

    assert_eq!(
        error,
        app::ParseOutcome::Error(
            "la commande plan requiert un chemin d'audit. Exemple: cargo run -p app -- plan .audit\\audit.md"
                .to_string()
        )
    );
}
