use std::fs;
use std::path::{Path, PathBuf};

pub fn normalize_artifact_path(
    path: &Path,
    workspace_root: &Path,
) -> Result<PathBuf, crate::automate::AutomationError> {
    let workspace_root = fs::canonicalize(workspace_root).map_err(|error| {
        crate::automate::AutomationError::artifact_normalization(format!(
            "impossible de normaliser le workspace automate {}: {error}",
            workspace_root.display()
        ))
    })?;
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };

    let metadata = fs::metadata(&resolved).map_err(|error| {
        crate::automate::AutomationError::artifact_normalization(format!(
            "artefact introuvable ou inaccessible {}: {error}",
            resolved.display()
        ))
    })?;
    if !metadata.is_file() {
        return Err(crate::automate::AutomationError::artifact_normalization(
            format!(
                "l'artefact structure doit etre un fichier: {}",
                resolved.display()
            ),
        ));
    }

    let normalized = fs::canonicalize(&resolved).map_err(|error| {
        crate::automate::AutomationError::artifact_normalization(format!(
            "impossible de normaliser l'artefact {}: {error}",
            resolved.display()
        ))
    })?;
    if !normalized.starts_with(&workspace_root) {
        return Err(crate::automate::AutomationError::artifact_normalization(
            format!(
                "l'artefact structure doit rester dans le workspace automate {}: {}",
                workspace_root.display(),
                normalized.display()
            ),
        ));
    }

    Ok(normalized)
}
