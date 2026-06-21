use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct ExecCapturePaths {
    output_file: PathBuf,
    stderr_file: Option<PathBuf>,
}

impl ExecCapturePaths {
    pub fn for_exec_run(verbose: bool) -> Self {
        Self {
            output_file: temp_output_file_path(),
            stderr_file: (!verbose).then(temp_stderr_file_path),
        }
    }

    pub fn output_file(&self) -> &Path {
        &self.output_file
    }

    pub fn stderr_file(&self) -> Option<&Path> {
        self.stderr_file.as_deref()
    }

    pub fn read_final_message(&self) -> io::Result<Option<String>> {
        read_final_message(&self.output_file)
    }

    pub fn read_final_message_if_ready(&self) -> io::Result<Option<String>> {
        read_final_message_if_ready(&self.output_file)
    }

    pub fn read_stderr(&self) -> io::Result<String> {
        read_optional_file(self.stderr_file())
    }

    pub fn read_stderr_without_removing(&self) -> io::Result<String> {
        read_optional_file_without_removing(self.stderr_file())
    }

    pub fn timeout_diagnostics(&self) -> String {
        timeout_diagnostics(&self.output_file, self.stderr_file())
    }
}

pub fn read_final_message(path: &Path) -> io::Result<Option<String>> {
    read_final_message_inner(path, true)
}

pub fn read_final_message_if_ready(path: &Path) -> io::Result<Option<String>> {
    read_final_message_inner(path, false)
}

fn read_final_message_inner(path: &Path, remove_if_empty: bool) -> io::Result<Option<String>> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };

    let trimmed = content.trim();
    if trimmed.is_empty() {
        if remove_if_empty {
            let _ = fs::remove_file(path);
        }
        Ok(None)
    } else {
        let _ = fs::remove_file(path);
        Ok(Some(trimmed.to_string()))
    }
}

fn temp_output_file_path() -> PathBuf {
    temp_codex_file_path("last_message")
}

fn temp_stderr_file_path() -> PathBuf {
    temp_codex_file_path("stderr")
}

fn temp_codex_file_path(kind: &str) -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    std::env::temp_dir().join(format!(
        "rust_agent_codex_{}_{}_{}.txt",
        std::process::id(),
        timestamp,
        kind
    ))
}

fn read_optional_file(path: Option<&Path>) -> io::Result<String> {
    let Some(path) = path else {
        return Ok(String::new());
    };

    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error),
    };

    let _ = fs::remove_file(path);
    Ok(content)
}

fn read_optional_file_without_removing(path: Option<&Path>) -> io::Result<String> {
    let Some(path) = path else {
        return Ok(String::new());
    };

    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error),
    }
}

pub fn timeout_diagnostics(output_file: &Path, stderr_file: Option<&Path>) -> String {
    let mut message = format!(
        "\nDiagnostics conserves pour inspection:\n- fichier message final: {}",
        output_file.display()
    );

    if let Some(stderr_file) = stderr_file {
        message.push_str(&format!(
            "\n- fichier stderr codex: {}",
            stderr_file.display()
        ));
    }

    message
}

// Tests locaux: la gestion des fichiers temporaires de capture est un detail
// du runner Codex, pas une API de comportement du binaire.
#[cfg(test)]
mod tests {
    use super::{read_final_message, read_final_message_if_ready, timeout_diagnostics};

    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "rust_agent_{prefix}_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ))
    }

    #[test]
    fn read_final_message_returns_none_for_missing_file() {
        let missing = temp_dir("missing_final_message").join("missing.txt");

        let result = read_final_message(&missing).expect("la lecture doit gerer un fichier absent");

        assert_eq!(result, None);
    }

    #[test]
    fn read_final_message_trims_and_deletes_file() {
        let path = temp_dir("capture").join("final.txt");
        fs::create_dir_all(path.parent().expect("parent")).expect("creation du dossier");
        fs::write(&path, "  reponse finale  \n").expect("ecriture du fichier temporaire");

        let result = read_final_message(&path).expect("lecture du fichier");

        assert_eq!(result.as_deref(), Some("reponse finale"));
        assert!(!path.exists(), "le fichier temporaire doit etre supprime");
        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[test]
    fn read_final_message_if_ready_preserves_empty_file() {
        let path = temp_dir("capture_ready_empty").join("final.txt");
        fs::create_dir_all(path.parent().expect("parent")).expect("creation du dossier");
        fs::write(&path, " \n\t ").expect("ecriture du fichier temporaire");

        let result =
            read_final_message_if_ready(&path).expect("lecture du fichier temporaire vide");

        assert_eq!(result, None);
        assert!(path.exists(), "le fichier vide doit rester disponible");
        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[test]
    fn read_final_message_if_ready_trims_and_deletes_file() {
        let path = temp_dir("capture_ready").join("final.txt");
        fs::create_dir_all(path.parent().expect("parent")).expect("creation du dossier");
        fs::write(&path, "  reponse prete  \n").expect("ecriture du fichier temporaire");

        let result = read_final_message_if_ready(&path).expect("lecture du fichier temporaire");

        assert_eq!(result.as_deref(), Some("reponse prete"));
        assert!(!path.exists(), "le fichier temporaire doit etre supprime");
        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[test]
    fn timeout_diagnostics_mentions_capture_files() {
        let message = timeout_diagnostics(Path::new("last.txt"), Some(Path::new("stderr.txt")));

        assert!(message.contains("last.txt"));
        assert!(message.contains("stderr.txt"));
    }
}
