mod support;

use app::process_exit_code;
use std::cell::Cell;
use std::fs;
use std::path::Path;

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
fn returns_probe_result_when_git_answers_true() {
    let start = Path::new("C:\\repo");
    let called = Cell::new(false);

    let result = app::codex::repo::is_inside_git_repository_from_with_probe(start, |path| {
        called.set(true);
        assert_eq!(path, start);
        Ok(Some(true))
    });

    assert!(result.expect("probe"));
    assert!(called.get());
}

#[test]
fn falls_back_to_git_marker_when_git_is_unavailable() {
    let workspace = support::temp_dir("git_repo_fallback");
    let child = workspace.join("nested");
    fs::create_dir_all(&child).expect("creation du dossier");
    fs::create_dir(workspace.join(".git")).expect("creation du marqueur .git");

    let result =
        app::codex::repo::is_inside_git_repository_from_with_probe(&child, |_path| Ok(None));

    assert!(result.expect("fallback"));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn falls_back_to_git_file_marker_for_worktree_like_layouts() {
    let workspace = support::temp_dir("git_repo_worktree_like");
    let child = workspace.join("nested");
    fs::create_dir_all(&child).expect("creation du dossier");
    fs::write(workspace.join(".git"), "gitdir: ../.git/worktrees/test")
        .expect("creation du fichier .git");

    let result =
        app::codex::repo::is_inside_git_repository_from_with_probe(&child, |_path| Ok(None));

    assert!(result.expect("fallback .git file"));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn falls_back_to_false_when_no_git_marker_exists() {
    let workspace = support::temp_dir("git_repo_none");
    let child = workspace.join("nested");
    fs::create_dir_all(&child).expect("creation du dossier");

    let result =
        app::codex::repo::is_inside_git_repository_from_with_probe(&child, |_path| Ok(None));

    assert!(!result.expect("fallback"));
    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn read_final_message_returns_none_for_missing_file() {
    let missing = support::temp_dir("missing_final_message").join("missing.txt");

    let result = app::codex::runner::capture::read_final_message(&missing)
        .expect("la lecture doit gerer un fichier absent");

    assert_eq!(result, None);
}

#[test]
fn read_final_message_trims_and_deletes_file() {
    let path = support::temp_dir("capture").join("final.txt");
    fs::create_dir_all(path.parent().expect("parent")).expect("creation du dossier");
    fs::write(&path, "  reponse finale  \n").expect("ecriture du fichier temporaire");

    let result =
        app::codex::runner::capture::read_final_message(&path).expect("lecture du fichier");

    assert_eq!(result.as_deref(), Some("reponse finale"));
    assert!(!path.exists(), "le fichier temporaire doit etre supprime");
    let _ = fs::remove_dir_all(path.parent().expect("parent"));
}

#[test]
fn read_final_message_if_ready_preserves_empty_file() {
    let path = support::temp_dir("capture_ready_empty").join("final.txt");
    fs::create_dir_all(path.parent().expect("parent")).expect("creation du dossier");
    fs::write(&path, " \n\t ").expect("ecriture du fichier temporaire");

    let result = app::codex::runner::capture::read_final_message_if_ready(&path)
        .expect("lecture du fichier temporaire vide");

    assert_eq!(result, None);
    assert!(path.exists(), "le fichier vide doit rester disponible");
    let _ = fs::remove_dir_all(path.parent().expect("parent"));
}

#[test]
fn read_final_message_if_ready_trims_and_deletes_file() {
    let path = support::temp_dir("capture_ready").join("final.txt");
    fs::create_dir_all(path.parent().expect("parent")).expect("creation du dossier");
    fs::write(&path, "  reponse prete  \n").expect("ecriture du fichier temporaire");

    let result = app::codex::runner::capture::read_final_message_if_ready(&path)
        .expect("lecture du fichier temporaire");

    assert_eq!(result.as_deref(), Some("reponse prete"));
    assert!(!path.exists(), "le fichier temporaire doit etre supprime");
    let _ = fs::remove_dir_all(path.parent().expect("parent"));
}

#[test]
fn timeout_diagnostics_mentions_capture_files() {
    let message = app::codex::runner::capture::timeout_diagnostics(
        Path::new("last.txt"),
        Some(Path::new("stderr.txt")),
    );

    assert!(message.contains("last.txt"));
    assert!(message.contains("stderr.txt"));
}
