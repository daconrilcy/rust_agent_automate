use std::fs;
use std::io;
use std::path::Path;

use crate::codex;
use crate::reporting::CommandOutcome;

pub fn decode_command_outcome(
    path: &Path,
) -> Result<Option<CommandOutcome>, crate::automate::AutomationError> {
    let content = match fs::read(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(crate::automate::AutomationError::structured_result_decode(
                error.to_string(),
            ));
        }
    };
    let _ = fs::remove_file(path);

    serde_json::from_slice(&content).map(Some).map_err(|error| {
        crate::automate::AutomationError::structured_result_decode(format!(
            "resultat structure invalide: {error}"
        ))
    })
}

pub fn normalized_status_code(status_code: Option<i32>) -> Option<i32> {
    status_code.map(|code| codex::process_exit_code(Some(code)))
}

// Tests locaux: le decodage du fichier de transport est un invariant prive
// entre processus internes et ne doit pas imposer un export public.
#[cfg(test)]
mod tests {
    use super::decode_command_outcome;

    use std::fs;
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
    fn decode_command_outcome_rejects_invalid_json() {
        let path = temp_dir("invalid_command_outcome").join("outcome.json");
        fs::create_dir_all(path.parent().expect("parent")).expect("creation du dossier");
        fs::write(&path, b"{ invalid json").expect("ecriture du resultat invalide");

        let error = decode_command_outcome(&path).expect_err("le JSON invalide doit echouer");

        assert!(error.to_string().contains("resultat structure invalide"));
        let _ = fs::remove_file(&path);
        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }
}
