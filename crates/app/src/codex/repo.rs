use std::io;
use std::path::Path;
use std::process::Command;

pub fn is_inside_git_repository() -> io::Result<bool> {
    let current = std::env::current_dir()?;
    is_inside_git_repository_from(&current)
}

pub fn is_inside_git_repository_from(start: &Path) -> io::Result<bool> {
    is_inside_git_repository_from_with_probe(start, probe_git_rev_parse)
}

fn is_inside_git_repository_from_with_probe(
    start: &Path,
    probe: impl Fn(&Path) -> io::Result<Option<bool>>,
) -> io::Result<bool> {
    if let Some(value) = probe(start)? {
        return Ok(value);
    }

    Ok(has_git_marker_in_ancestors(start))
}

fn probe_git_rev_parse(start: &Path) -> io::Result<Option<bool>> {
    let output = Command::new("git")
        .args([
            "-C",
            &start.display().to_string(),
            "rev-parse",
            "--is-inside-work-tree",
        ])
        .output();

    let output = match output {
        Ok(output) => output,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    if !output.status.success() {
        return Ok(Some(false));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(Some(stdout.trim().eq_ignore_ascii_case("true")))
}

fn has_git_marker_in_ancestors(start: &Path) -> bool {
    let mut current = start.to_path_buf();

    loop {
        if current.join(".git").exists() {
            return true;
        }

        if !current.pop() {
            return false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();

        std::env::temp_dir().join(format!(
            "rust_agent_{label}_{}_{}",
            std::process::id(),
            nanos
        ))
    }

    #[test]
    fn returns_probe_result_when_git_answers_true() {
        let start = Path::new("C:\\repo");
        let called = Cell::new(false);

        let result = is_inside_git_repository_from_with_probe(start, |path| {
            called.set(true);
            assert_eq!(path, start);
            Ok(Some(true))
        });

        assert!(result.expect("probe"));
        assert!(called.get());
    }

    #[test]
    fn falls_back_to_git_marker_when_git_is_unavailable() {
        let workspace = unique_temp_dir("git_repo_fallback");
        let child = workspace.join("nested");
        fs::create_dir_all(&child).expect("creation du dossier");
        fs::create_dir(workspace.join(".git")).expect("creation du marqueur .git");

        let result = is_inside_git_repository_from_with_probe(&child, |_path| Ok(None));

        assert!(result.expect("fallback"));
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn falls_back_to_git_file_marker_for_worktree_like_layouts() {
        let workspace = unique_temp_dir("git_repo_worktree_like");
        let child = workspace.join("nested");
        fs::create_dir_all(&child).expect("creation du dossier");
        fs::write(workspace.join(".git"), "gitdir: ../.git/worktrees/test")
            .expect("creation du fichier .git");

        let result = is_inside_git_repository_from_with_probe(&child, |_path| Ok(None));

        assert!(result.expect("fallback .git file"));
        let _ = fs::remove_dir_all(workspace);
    }

    #[test]
    fn falls_back_to_false_when_no_git_marker_exists() {
        let workspace = unique_temp_dir("git_repo_none");
        let child = workspace.join("nested");
        fs::create_dir_all(&child).expect("creation du dossier");

        let result = is_inside_git_repository_from_with_probe(&child, |_path| Ok(None));

        assert!(!result.expect("fallback"));
        let _ = fs::remove_dir_all(workspace);
    }
}
