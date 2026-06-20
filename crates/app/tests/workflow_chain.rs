mod common;

use std::fs;

#[test]
fn workflow_chain_persists_and_reuses_artifacts() {
    let workspace = common::temp_dir("workflow_chain");
    let codex_bin = common::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    let workflow_path = workspace.join("workflow.json");
    let target_dir = workspace.join("target");
    fs::create_dir_all(&target_dir).expect("creation du dossier cible");

    let workflow = r#"{
      "defaults": {
        "model": null,
        "reasoning": null,
        "timeout_seconds": 30
      },
      "steps": [
        {
          "name": "audit",
          "rust_command": ["audit", "--target", "{target}"],
          "fresh_codex_call": true
        },
        {
          "name": "plan",
          "rust_command": ["plan", "{artifact:audit}"],
          "fresh_codex_call": true
        }
      ],
      "loop_policy": {
        "audit_step": "audit",
        "max_cycles": 1,
        "clean_markers": ["fake final message"]
      }
    }"#;
    fs::write(&workflow_path, workflow).expect("ecriture du workflow");

    let mut command = common::build_command();
    command
        .current_dir(&workspace)
        .env(
            "PATH",
            common::join_path_dirs([codex_bin.parent().expect("bin parent").to_path_buf()]),
        )
        .env("USERPROFILE", &workspace)
        .env("FAKE_CODEX_LOG", &log_path)
        .args([
            "automate",
            workflow_path.to_str().expect("workflow path utf-8"),
            "Initial prompt",
        ]);

    let output = command.output().expect("execution de l'automate");
    assert!(output.status.success(), "sortie inattendue: {:?}", output);

    let audit_reports = fs::read_dir(workspace.join(".audit"))
        .expect("lecture du dossier .audit")
        .count();
    let plan_reports = fs::read_dir(workspace.join(".plan"))
        .expect("lecture du dossier .plan")
        .count();

    assert!(audit_reports >= 1, "un rapport d'audit doit etre genere");
    assert!(plan_reports >= 1, "un plan doit etre genere");

    let logged = fs::read_to_string(&log_path).expect("lecture du log codex");
    assert!(
        logged.contains(".audit"),
        "le prompt du plan doit reutiliser l'artefact d'audit"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn refactor_automate_uses_target_workspace_when_launched_from_other_cwd() {
    let runner_dir = common::temp_dir("workflow_runner");
    let workspace = common::temp_dir("workflow_target");
    let codex_bin = common::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    let workflow_path = runner_dir.join("workflow.json");
    fs::create_dir_all(&runner_dir).expect("creation du dossier runner");
    fs::create_dir_all(&workspace).expect("creation du dossier workspace");

    let workflow = r#"{
      "defaults": {
        "model": null,
        "reasoning": null,
        "timeout_seconds": 30
      },
      "steps": [
        {
          "name": "audit",
          "rust_command": ["audit", "--target", "{target}"],
          "fresh_codex_call": true
        },
        {
          "name": "plan",
          "rust_command": ["plan", "{artifact:audit}"],
          "fresh_codex_call": true
        }
      ],
      "loop_policy": {
        "audit_step": "audit",
        "max_cycles": 1,
        "clean_markers": ["fake final message"]
      }
    }"#;
    fs::write(&workflow_path, workflow).expect("ecriture du workflow");

    let mut command = common::build_command();
    command
        .current_dir(&runner_dir)
        .env(
            "PATH",
            common::join_path_dirs([codex_bin.parent().expect("bin parent").to_path_buf()]),
        )
        .env("USERPROFILE", &workspace)
        .env("FAKE_CODEX_LOG", &log_path)
        .args([
            "refactor-automate",
            "--target",
            workspace.to_str().expect("workspace utf-8"),
            "--workflow",
            workflow_path.to_str().expect("workflow path utf-8"),
            "Initial prompt",
        ]);

    let output = command.output().expect("execution du refactor automate");
    assert!(output.status.success(), "sortie inattendue: {:?}", output);

    let audit_reports = fs::read_dir(workspace.join(".audit"))
        .expect("lecture du dossier .audit")
        .count();
    let plan_reports = fs::read_dir(workspace.join(".plan"))
        .expect("lecture du dossier .plan")
        .count();

    assert!(
        audit_reports >= 1,
        "un rapport d'audit doit etre genere dans la cible"
    );
    assert!(plan_reports >= 1, "un plan doit etre genere dans la cible");
    assert!(
        !runner_dir.join(".audit").exists(),
        "le cwd de lancement ne doit pas recevoir les artefacts"
    );

    let _ = fs::remove_dir_all(runner_dir);
    let _ = fs::remove_dir_all(workspace);
}
