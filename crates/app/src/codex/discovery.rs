use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn resolve_codex_executable() -> io::Result<PathBuf> {
    let mut candidates = executable_candidates();
    candidates.dedup();

    let path_env = std::env::var_os("PATH");
    let pathext_env = std::env::var_os("PATHEXT");

    candidates
        .into_iter()
        .find_map(|candidate| {
            resolve_codex_candidate(&candidate, path_env.as_deref(), pathext_env.as_deref())
        })
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, codex_not_found_message()))
}

pub fn configure_child_path(command: &mut Command) {
    let Some(path) = child_path_with_tool_dirs_first() else {
        return;
    };

    command.env("PATH", path);
}

pub(crate) fn resolve_codex_candidate(
    candidate: &Path,
    path_env: Option<&std::ffi::OsStr>,
    pathext_env: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    if candidate.is_file() {
        return Some(candidate.to_path_buf());
    }

    if candidate_has_explicit_path(candidate) {
        return None;
    }

    resolve_candidate_from_path(candidate, path_env, pathext_env)
}

pub(crate) fn executable_candidates_from_user_profile(
    user_profile: Option<&Path>,
    codex_cli_path: Option<PathBuf>,
) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(path) = codex_cli_path {
        candidates.push(path);
    }

    if let Some(user_profile) = user_profile {
        candidates.extend(vscode_extension_candidates(user_profile));
        candidates.push(user_profile.join("AppData\\Roaming\\npm\\codex.exe"));
        candidates.push(user_profile.join("AppData\\Roaming\\npm\\codex.cmd"));
    }

    candidates.extend([PathBuf::from("codex.exe"), PathBuf::from("codex")]);
    candidates.push(PathBuf::from("codex.cmd"));

    candidates
}

pub(crate) fn prepend_path_dirs(
    existing_path: std::ffi::OsString,
    front_dirs: &[PathBuf],
) -> Option<std::ffi::OsString> {
    let mut paths = Vec::new();

    for dir in front_dirs {
        push_unique_path(&mut paths, dir.clone());
    }

    for path in std::env::split_paths(&existing_path) {
        push_unique_path(&mut paths, path);
    }

    std::env::join_paths(paths).ok()
}

fn executable_candidates() -> Vec<PathBuf> {
    let user_profile = std::env::var_os("USERPROFILE");
    let codex_cli_path = std::env::var_os("CODEX_CLI_PATH").map(PathBuf::from);

    executable_candidates_from_user_profile(user_profile.as_deref().map(Path::new), codex_cli_path)
}

fn candidate_has_explicit_path(candidate: &Path) -> bool {
    candidate.is_absolute() || candidate.components().count() > 1
}

fn resolve_candidate_from_path(
    candidate: &Path,
    path_env: Option<&std::ffi::OsStr>,
    pathext_env: Option<&std::ffi::OsStr>,
) -> Option<PathBuf> {
    let path_env = path_env?;
    let path_dirs = std::env::split_paths(path_env).collect::<Vec<_>>();
    let path_candidates = path_lookup_candidates(candidate, pathext_env);

    path_dirs
        .into_iter()
        .flat_map(|dir| path_candidates.iter().map(move |entry| dir.join(entry)))
        .find(|path| path.is_file())
}

fn path_lookup_candidates(candidate: &Path, pathext_env: Option<&std::ffi::OsStr>) -> Vec<PathBuf> {
    if candidate.extension().is_some() {
        return vec![candidate.to_path_buf()];
    }

    let mut extensions = pathext_extensions(pathext_env);
    let mut candidates = Vec::with_capacity(extensionless_candidate_capacity(extensions.len()));

    #[cfg(not(windows))]
    candidates.push(candidate.to_path_buf());

    candidates.extend(
        extensions
            .drain(..)
            .map(|ext| candidate.with_extension(ext)),
    );
    candidates
}

fn extensionless_candidate_capacity(extension_count: usize) -> usize {
    #[cfg(windows)]
    {
        extension_count
    }

    #[cfg(not(windows))]
    {
        1 + extension_count
    }
}

fn pathext_extensions(pathext_env: Option<&std::ffi::OsStr>) -> Vec<String> {
    let default = ".COM;.EXE;.BAT;.CMD";
    let pathext = pathext_env
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_string());

    pathext
        .split(';')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.trim_start_matches('.').to_string())
        .collect()
}

fn child_path_with_tool_dirs_first() -> Option<std::ffi::OsString> {
    let existing_path = std::env::var_os("PATH")?;
    let tool_dirs = codex_tool_dirs();

    if tool_dirs.is_empty() {
        return None;
    }

    prepend_path_dirs(existing_path, &tool_dirs)
}

fn codex_tool_dirs() -> Vec<PathBuf> {
    let Some(user_profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) else {
        return Vec::new();
    };

    vscode_extension_tool_dirs(&user_profile)
}

fn vscode_extension_tool_dirs(user_profile: &Path) -> Vec<PathBuf> {
    let extensions_dir = user_profile.join(".vscode\\extensions");
    let Ok(entries) = fs::read_dir(extensions_dir) else {
        return Vec::new();
    };

    let mut dirs = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("openai.chatgpt-"))
        })
        .map(|path| path.join("bin\\windows-x86_64"))
        .filter(|path| path.join("rg.exe").is_file())
        .collect::<Vec<_>>();

    dirs.sort_by(|left, right| right.cmp(left));
    dirs
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if paths.iter().any(|existing| paths_equal(existing, &path)) {
        return;
    }

    paths.push(path);
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

fn codex_not_found_message() -> String {
    "impossible de trouver l'executable Codex. Verifie que Codex est installe ou definis la variable d'environnement CODEX_CLI_PATH".to_string()
}

fn vscode_extension_candidates(user_profile: &Path) -> Vec<PathBuf> {
    let extensions_dir = user_profile.join(".vscode\\extensions");
    let Ok(entries) = fs::read_dir(extensions_dir) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("openai.chatgpt-"))
        })
        .map(|path| path.join("bin\\windows-x86_64\\codex.exe"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepends_tool_dirs_without_duplicates() {
        let existing =
            std::env::join_paths([PathBuf::from("C:\\Windows"), PathBuf::from("C:\\Tools")])
                .expect("construction du PATH de test");
        let tool_dirs = vec![PathBuf::from("C:\\Tools"), PathBuf::from("C:\\Codex\\bin")];

        let updated = prepend_path_dirs(existing, &tool_dirs).expect("PATH valide");
        let paths = std::env::split_paths(&updated).collect::<Vec<_>>();

        assert_eq!(paths[0], PathBuf::from("C:\\Tools"));
        assert_eq!(paths[1], PathBuf::from("C:\\Codex\\bin"));
        assert_eq!(paths[2], PathBuf::from("C:\\Windows"));
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn resolve_codex_candidate_accepts_explicit_file_path() {
        let temp_dir = std::env::temp_dir().join(format!(
            "rust_agent_codex_candidate_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let executable = temp_dir.join("codex.exe");
        fs::create_dir_all(&temp_dir).expect("creation du dossier temporaire");
        fs::write(&executable, "fake codex").expect("creation du fichier exécutable");

        let resolved = resolve_codex_candidate(&executable, None, None)
            .expect("le chemin explicite doit etre accepte");

        assert_eq!(resolved, executable);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn resolve_codex_candidate_finds_bare_name_on_path() {
        let temp_dir = std::env::temp_dir().join(format!(
            "rust_agent_codex_path_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let bin_dir = temp_dir.join("bin");
        let executable = bin_dir.join("codex.cmd");
        fs::create_dir_all(&bin_dir).expect("creation du dossier PATH");
        fs::write(&executable, "fake codex").expect("creation du faux codex");

        let path_env = std::env::join_paths([bin_dir.clone()]).expect("PATH de test");
        let resolved = resolve_codex_candidate(
            Path::new("codex"),
            Some(path_env.as_os_str()),
            Some(std::ffi::OsStr::new(".CMD;.EXE")),
        )
        .expect("le PATH doit etre pris en compte");

        assert_eq!(
            resolved.to_string_lossy().to_ascii_lowercase(),
            executable.to_string_lossy().to_ascii_lowercase()
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    #[cfg(windows)]
    fn resolve_codex_candidate_ignores_extensionless_windows_path_entry() {
        let temp_dir = std::env::temp_dir().join(format!(
            "rust_agent_codex_windows_path_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let bin_dir = temp_dir.join("bin");
        let extensionless = bin_dir.join("codex");
        let executable = bin_dir.join("codex.cmd");
        fs::create_dir_all(&bin_dir).expect("creation du dossier PATH");
        fs::write(&extensionless, "not a win32 executable").expect("creation du faux codex");
        fs::write(&executable, "fake codex").expect("creation du faux codex cmd");

        let path_env = std::env::join_paths([bin_dir.clone()]).expect("PATH de test");
        let resolved = resolve_codex_candidate(
            Path::new("codex"),
            Some(path_env.as_os_str()),
            Some(std::ffi::OsStr::new(".CMD;.EXE")),
        )
        .expect("le PATH doit etre pris en compte");

        assert_eq!(
            resolved.to_string_lossy().to_ascii_lowercase(),
            executable.to_string_lossy().to_ascii_lowercase()
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn executable_candidates_prefers_vscode_native_executable_before_path_lookup() {
        let temp_dir = std::env::temp_dir().join(format!(
            "rust_agent_codex_candidates_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let extension_bin =
            temp_dir.join(".vscode\\extensions\\openai.chatgpt-test\\bin\\windows-x86_64");
        fs::create_dir_all(&extension_bin).expect("creation du dossier extension");
        let native_codex = extension_bin.join("codex.exe");
        fs::write(&native_codex, "fake codex").expect("creation du faux codex natif");
        fs::write(extension_bin.join("rg.exe"), "fake rg").expect("creation du faux rg");

        let candidates = executable_candidates_from_user_profile(Some(&temp_dir), None);

        assert_eq!(candidates.first(), Some(&native_codex));
        let path_lookup_position = candidates
            .iter()
            .position(|candidate| candidate == Path::new("codex"))
            .expect("candidat PATH attendu");
        assert!(path_lookup_position > 0);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
