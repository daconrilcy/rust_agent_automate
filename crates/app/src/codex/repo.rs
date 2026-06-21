use std::io;
use std::path::Path;
use std::process::Command;

pub fn is_inside_git_repository_at(start: Option<&Path>) -> io::Result<bool> {
    let start = match start {
        Some(path) => path.to_path_buf(),
        None => std::env::current_dir()?,
    };
    is_inside_git_repository_from(&start)
}

pub fn is_inside_git_repository_from(start: &Path) -> io::Result<bool> {
    is_inside_git_repository_from_with_probe(start, probe_git_rev_parse)
}

pub fn is_inside_git_repository_from_with_probe(
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
