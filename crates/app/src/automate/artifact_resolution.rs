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

// Tests locaux: la normalisation d'artefact est un detail interne du runner
// automate, pas un contrat a consommer depuis les tests d'integration.
#[cfg(test)]
mod tests {
    use super::normalize_artifact_path;

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
    fn normalizes_relative_artifact_path_from_workspace_root() {
        let workspace = temp_dir("workflow_runner_relative_artifact");
        let artifact = workspace.join(".audit").join("audit.md");
        fs::create_dir_all(artifact.parent().expect("parent")).expect("creation du dossier");
        fs::write(&artifact, "audit").expect("ecriture de l'artefact");

        let normalized = normalize_artifact_path(Path::new(".audit\\audit.md"), &workspace)
            .expect("normalisation");

        assert_eq!(
            normalized,
            fs::canonicalize(&artifact).expect("chemin canonical")
        );

        let _ = fs::remove_dir_all(workspace);
    }
}
