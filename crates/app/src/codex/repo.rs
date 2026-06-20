use std::io;
use std::path::Path;

pub fn is_inside_git_repository() -> io::Result<bool> {
    let current = std::env::current_dir()?;
    is_inside_git_repository_from(&current)
}

pub fn is_inside_git_repository_from(start: &Path) -> io::Result<bool> {
    let mut current = start.to_path_buf();

    loop {
        if current.join(".git").exists() {
            return Ok(true);
        }

        if !current.pop() {
            return Ok(false);
        }
    }
}
