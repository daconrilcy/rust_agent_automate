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
