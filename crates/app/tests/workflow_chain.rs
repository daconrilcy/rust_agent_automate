mod support;

use std::fs;

#[test]
fn workflow_chain_injects_resume_flags_for_direct_runs() {
    let workspace = support::temp_dir("workflow_chain_direct_run");
    let codex_bin = support::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    let workflow_path = workspace.join("workflow.json");

    let workflow = r#"{
      "defaults": {
        "model": "gpt-x",
        "reasoning": "high"
      },
      "steps": [
        {
          "name": "commit",
          "rust_command": ["--mode", "exec", "Commit"],
          "fresh_codex_call": false
        }
      ]
    }"#;
    fs::write(&workflow_path, workflow).expect("ecriture du workflow");

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
            "automate",
            workflow_path.to_str().expect("workflow path utf-8"),
            "Initial prompt",
        ]);

    let output = command.output().expect("execution de l'automate");
    assert!(output.status.success(), "sortie inattendue: {:?}", output);

    let logged = fs::read_to_string(&log_path).expect("lecture du log codex");
    assert!(logged.contains("exec resume --last"));
    assert!(logged.contains("gpt-x"));
    assert!(logged.contains("high"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn workflow_chain_persists_and_reuses_artifacts() {
    let workspace = support::temp_dir("workflow_chain");
    let codex_bin = support::create_fake_codex_bin(&workspace);
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
      ]
    }"#;
    fs::write(&workflow_path, workflow).expect("ecriture du workflow");

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
fn refactor_automate_writes_artifacts_to_launch_workspace_when_target_is_subdir() {
    let workspace = support::temp_dir("workflow_runner");
    let target_dir = workspace.join("crates").join("app");
    let codex_bin = support::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    let workflow_path = workspace.join("workflow.json");
    fs::create_dir_all(&target_dir).expect("creation du dossier cible");
    fs::create_dir(workspace.join(".git")).expect("creation du faux depot de lancement");

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
        },
        {
          "name": "fix_loop",
          "rust_command": ["fix-loop", "plan", "{artifact:plan}"],
          "fresh_codex_call": true
        }
      ]
    }"#;
    fs::write(&workflow_path, workflow).expect("ecriture du workflow");

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
            "refactor-automate",
            "--target",
            target_dir.to_str().expect("target utf-8"),
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
    let fix_loop_reports = fs::read_dir(workspace.join(".fix-loop"))
        .expect("lecture du dossier .fix-loop")
        .count();

    assert!(
        audit_reports >= 1,
        "un rapport d'audit doit etre genere dans le workspace de lancement"
    );
    assert!(
        plan_reports >= 1,
        "un plan doit etre genere dans le workspace de lancement"
    );
    assert!(
        fix_loop_reports >= 1,
        "un rapport fix-loop doit etre genere dans le workspace de lancement"
    );
    assert!(
        !target_dir.join(".audit").exists(),
        "la cible de refactoring ne doit pas recevoir les artefacts"
    );
    assert!(
        !target_dir.join(".plan").exists(),
        "la cible de refactoring ne doit pas recevoir les plans"
    );
    assert!(
        !target_dir.join(".fix-loop").exists(),
        "la cible de refactoring ne doit pas recevoir les rapports fix-loop"
    );

    let logged = fs::read_to_string(&log_path).expect("lecture du log codex");
    assert!(
        logged.contains(&format!("cwd={}", workspace.display())),
        "les appels codex enfants doivent s'executer depuis le workspace de lancement"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn workflow_stops_after_clean_implementation_audit_cycle() {
    let workspace = support::temp_dir("workflow_clean_stop");
    let codex_bin = support::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    let workflow_path = workspace.join("workflow.json");
    let target_dir = workspace.join("target");
    let plan_path = workspace.join(".plan").join("plan.md");
    fs::create_dir_all(&target_dir).expect("creation du dossier cible");
    fs::create_dir_all(plan_path.parent().expect("parent du plan"))
        .expect("creation du dossier plan");
    fs::write(&plan_path, "# plan").expect("ecriture du plan");

    let escaped_plan_path = plan_path.display().to_string().replace('\\', "\\\\");
    let workflow = format!(
        r#"{{
      "steps": [
        {{
          "name": "alignment_audit",
          "rust_command": ["implementation-audit", "{}"]
        }},
        {{
          "name": "commit",
          "rust_command": ["--mode", "exec", "Commit"]
        }}
      ],
      "loop_policy": {{"audit_step":"alignment_audit","max_cycles":3}}
    }}"#,
        escaped_plan_path
    );
    fs::write(&workflow_path, workflow).expect("ecriture du workflow");

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
        .env(
            "FAKE_CODEX_MESSAGE",
            "# Rust Implementation Plan Audit\n\n## Deviations\nNo actionable deviations found.\n",
        )
        .args([
            "automate",
            workflow_path.to_str().expect("workflow path utf-8"),
            "Initial prompt",
        ]);

    let output = command.output().expect("execution de l'automate");
    assert!(output.status.success(), "sortie inattendue: {:?}", output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Automate termine apres 1 cycle(s)"),
        "le workflow doit s'arreter apres un cycle propre: {stdout}"
    );

    assert!(
        !stdout.contains("cycle 2"),
        "aucun second cycle ne doit etre execute: {stdout}"
    );

    let _ = fs::remove_dir_all(workspace);
}
