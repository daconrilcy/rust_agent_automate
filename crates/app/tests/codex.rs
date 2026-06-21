mod support;

use app::process_exit_code;
use std::fs;

#[test]
fn process_exit_code_maps_non_portable_child_statuses_to_generic_failure() {
    assert_eq!(process_exit_code(Some(0)), 0);
    assert_eq!(process_exit_code(Some(7)), 7);
    assert_eq!(process_exit_code(Some(-1)), 1);
    assert_eq!(process_exit_code(Some(256)), 1);
    assert_eq!(process_exit_code(None), 1);
}

#[test]
fn exec_mode_uses_requested_working_directory_and_resume_flags() {
    let workspace = support::temp_dir("codex_runner");
    let repo_dir = workspace.join("repo");
    let bin = support::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    fs::create_dir_all(repo_dir.join(".git")).expect("creation du depot");

    let mut command = support::build_command();
    command
        .current_dir(&repo_dir)
        .env(
            "PATH",
            support::join_path_dirs([bin.parent().expect("bin parent").to_path_buf()]),
        )
        .env("USERPROFILE", &workspace)
        .env("FAKE_CODEX_LOG", &log_path)
        .env("FAKE_CODEX_MESSAGE", "final message")
        .args([
            "--mode",
            "exec",
            "--model",
            "gpt-5.5",
            "--reasoning",
            "high",
            "--continue-codex",
            "Continue",
        ]);

    let output = command.output().expect("execution codex");

    assert!(output.status.success());
    let logged = fs::read_to_string(&log_path).expect("lecture du log codex");
    assert!(logged.contains(&format!("cwd={}", repo_dir.display())));
    assert!(logged.contains("args=exec resume --last"));
    assert!(logged.contains("--model gpt-5.5"));
    assert!(logged.contains("model_reasoning_effort=\"high\""));
    assert!(logged.contains("developpement solo avec agents"));
    assert!(logged.contains("application locale Windows-only"));
    assert!(!logged.contains("--skip-git-repo-check"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn exec_mode_outside_git_adds_skip_repo_check() {
    let workspace = support::temp_dir("codex_runner_outside_git");
    let repo_dir = workspace.join("repo");
    let bin = support::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    fs::create_dir_all(&repo_dir).expect("creation du dossier");

    let mut command = support::build_command();
    command
        .current_dir(&repo_dir)
        .env(
            "PATH",
            support::join_path_dirs([bin.parent().expect("bin parent").to_path_buf()]),
        )
        .env("USERPROFILE", &workspace)
        .env("FAKE_CODEX_LOG", &log_path)
        .env("FAKE_CODEX_MESSAGE", "final message")
        .args([
            "--mode",
            "exec",
            "--model",
            "gpt-5.5",
            "--reasoning",
            "high",
            "Analyse ce repo",
        ]);

    let output = command.output().expect("execution codex");

    assert!(output.status.success());
    let logged = fs::read_to_string(&log_path).expect("lecture du log codex");
    assert!(logged.contains("--skip-git-repo-check"));
    assert!(logged.contains("developpement solo avec agents"));
    assert!(logged.contains("application locale Windows-only"));

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn exec_mode_places_permission_flags_before_exec_subcommand() {
    let workspace = support::temp_dir("codex_runner_permissions");
    let repo_dir = workspace.join("repo");
    let bin = support::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");
    fs::create_dir_all(repo_dir.join(".git")).expect("creation du depot");

    let mut command = support::build_command();
    command
        .current_dir(&repo_dir)
        .env(
            "PATH",
            support::join_path_dirs([bin.parent().expect("bin parent").to_path_buf()]),
        )
        .env("USERPROFILE", &workspace)
        .env("FAKE_CODEX_LOG", &log_path)
        .env("FAKE_CODEX_MESSAGE", "final message")
        .args([
            "--mode",
            "exec",
            "--sandbox",
            "danger-full-access",
            "--ask-for-approval",
            "never",
            "Analyse",
        ]);

    let output = command.output().expect("execution codex");

    assert!(output.status.success());
    let logged = fs::read_to_string(&log_path).expect("lecture du log codex");
    assert!(
        logged.contains("args=--sandbox danger-full-access --ask-for-approval never exec"),
        "{logged}"
    );

    let _ = fs::remove_dir_all(workspace);
}
