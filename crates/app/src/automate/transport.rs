use std::fs;
use std::io;
use std::path::Path;

use crate::codex;
use crate::reporting::{COMMAND_OUTCOME_SCHEMA_VERSION, CommandOutcome};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct CommandOutcomeEnvelope {
    schema_version: u16,
    outcome: CommandOutcome,
}

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

    let envelope: CommandOutcomeEnvelope = serde_json::from_slice(&content).map_err(|error| {
        crate::automate::AutomationError::structured_result_decode(format!(
            "resultat structure invalide: {error}"
        ))
    })?;

    if envelope.schema_version != COMMAND_OUTCOME_SCHEMA_VERSION {
        return Err(crate::automate::AutomationError::structured_result_decode(
            format!(
                "version de resultat structure non supportee: {} (attendue: {})",
                envelope.schema_version, COMMAND_OUTCOME_SCHEMA_VERSION
            ),
        ));
    }

    Ok(Some(envelope.outcome))
}

pub fn normalized_status_code(status_code: Option<i32>) -> Option<i32> {
    status_code.map(|code| codex::process_exit_code(Some(code)))
}

// Tests locaux: le decodage du fichier de transport est un invariant prive
// entre processus internes et ne doit pas imposer un export public.
#[cfg(test)]
mod tests {
    use super::decode_command_outcome;

    use crate::reporting::CommandOutcome;

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

    #[test]
    fn decode_command_outcome_reads_versioned_envelope() {
        let path = temp_dir("valid_command_outcome").join("outcome.json");
        fs::create_dir_all(path.parent().expect("parent")).expect("creation du dossier");
        fs::write(
            &path,
            br#"{"schema_version":1,"outcome":{"command_name":"audit","status_code":0,"final_message_present":true,"artifact_path":null,"clean":true}}"#,
        )
        .expect("ecriture du resultat");

        let outcome = decode_command_outcome(&path)
            .expect("decodage du resultat")
            .expect("resultat present");

        assert_eq!(
            outcome,
            CommandOutcome {
                command_name: "audit".to_string(),
                status_code: Some(0),
                final_message_present: true,
                artifact_path: None,
                clean: Some(true),
            }
        );
        assert!(!path.exists(), "le fichier consomme doit etre supprime");
        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[test]
    fn decode_command_outcome_rejects_unknown_schema_version() {
        let path = temp_dir("unsupported_command_outcome").join("outcome.json");
        fs::create_dir_all(path.parent().expect("parent")).expect("creation du dossier");
        fs::write(
            &path,
            br#"{"schema_version":2,"outcome":{"command_name":"audit","status_code":0,"final_message_present":true,"artifact_path":null,"clean":true}}"#,
        )
        .expect("ecriture du resultat");

        let error = decode_command_outcome(&path).expect_err("la version inconnue doit echouer");

        assert!(
            error
                .to_string()
                .contains("version de resultat structure non supportee")
        );
        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }
}
