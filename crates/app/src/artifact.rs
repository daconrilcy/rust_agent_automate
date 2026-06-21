use std::fs;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn save_timestamped_markdown(
    output_dir: &Path,
    prefix: &str,
    content: &str,
) -> io::Result<PathBuf> {
    fs::create_dir_all(output_dir)?;
    let output_dir = fs::canonicalize(output_dir)?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default();

    for attempt in 0..1000 {
        let path = artifact_path(&output_dir, prefix, timestamp, attempt);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(mut file) => {
                if let Err(error) = file.write_all(content.as_bytes()) {
                    let _ = fs::remove_file(&path);
                    return Err(error);
                }
                return Ok(path);
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "impossible de creer un artefact unique dans {}",
            output_dir.display()
        ),
    ))
}

fn artifact_path(output_dir: &Path, prefix: &str, timestamp: u64, attempt: u16) -> PathBuf {
    if attempt == 0 {
        return output_dir.join(format!("{prefix}-{timestamp}.md"));
    }

    output_dir.join(format!("{prefix}-{timestamp}-{attempt}.md"))
}
