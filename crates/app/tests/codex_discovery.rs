mod support;

use std::fs;

#[test]
fn codex_discovery_uses_path_lookup() {
    let workspace = support::temp_dir("codex_discovery");
    let codex_bin = support::create_fake_codex_bin(&workspace);
    let log_path = workspace.join("codex.log");

    let mut command = support::build_command();
    command
        .current_dir(&workspace)
        .env(
            "PATH",
            support::join_path_dirs([codex_bin.parent().expect("bin parent").to_path_buf()]),
        )
        .env("USERPROFILE", &workspace)
        .env("FAKE_CODEX_LOG", &log_path)
        .args(["--mode", "exec", "hello"]);

    let output = command.output().expect("execution de app");
    assert!(output.status.success(), "sortie inattendue: {:?}", output);
    assert!(String::from_utf8_lossy(&output.stdout).contains("fake final message"));
    assert!(log_path.exists(), "le faux codex doit etre appele");

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn codex_discovery_prefers_codex_cli_path() {
    let workspace = support::temp_dir("codex_cli_path");
    let preferred_dir = workspace.join("preferred");
    let fallback_dir = workspace.join("fallback");
    let preferred = support::create_fake_codex_bin(&preferred_dir);
    let fallback = support::create_fake_codex_bin(&fallback_dir);
    let log_path = workspace.join("codex.log");

    let mut command = support::build_command();
    command
        .current_dir(&workspace)
        .env(
            "PATH",
            support::join_path_dirs([fallback.parent().expect("fallback parent").to_path_buf()]),
        )
        .env("USERPROFILE", &workspace)
        .env("FAKE_CODEX_LOG", &log_path)
        .env("CODEX_CLI_PATH", &preferred)
        .args(["--mode", "exec", "hello"]);

    let output = command.output().expect("execution de app");
    assert!(output.status.success(), "sortie inattendue: {:?}", output);
    let logged = fs::read_to_string(&log_path).expect("lecture du log codex");
    assert!(
        logged.contains(preferred.to_string_lossy().as_ref()),
        "CODEX_CLI_PATH doit etre prioritaire"
    );

    let _ = fs::remove_dir_all(workspace);
}

#[test]
fn codex_discovery_accepts_explicit_codex_cli_path_without_path_lookup() {
    let workspace = support::temp_dir("codex_explicit_path");
    let preferred_dir = workspace.join("preferred");
    let fallback_dir = workspace.join("fallback");
    let preferred = support::create_fake_codex_bin(&preferred_dir);
    let _fallback = support::create_fake_codex_bin(&fallback_dir);
    let log_path = workspace.join("codex.log");

    let mut command = support::build_command();
    command
        .current_dir(&workspace)
        .env(
            "PATH",
            support::join_path_dirs([fallback_dir.join("missing-bin")]),
        )
        .env("USERPROFILE", &workspace)
        .env("FAKE_CODEX_LOG", &log_path)
        .env("CODEX_CLI_PATH", &preferred)
        .args(["--mode", "exec", "hello"]);

    let output = command.output().expect("execution de app");
    assert!(output.status.success(), "sortie inattendue: {:?}", output);

    let logged = fs::read_to_string(&log_path).expect("lecture du log codex");
    assert!(
        logged.contains(preferred.to_string_lossy().as_ref()),
        "le chemin explicite doit etre utilise tel quel"
    );

    let _ = fs::remove_dir_all(workspace);
}
