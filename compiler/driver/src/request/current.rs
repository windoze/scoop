use super::*;

/// Direct operands remain local; machine input already owns its expanded configuration.
#[derive(Debug)]
pub enum CurrentConeInput {
    Manifest {
        root: ManifestRootLocator,
    },
    ExpandedManifest {
        manifest: Box<scoop_manifest::LoadedConeManifest>,
    },
    SingleFile {
        source: SingleFileLocator,
    },
}

pub fn classify_current_cone_operand(
    path: impl Into<PathBuf>,
) -> Result<CurrentConeInput, CurrentConeOperandError> {
    Ok(match scoop_manifest::classify_current_cone_operand(path)? {
        scoop_manifest::CurrentConeInput::Manifest { root } => CurrentConeInput::Manifest { root },
        scoop_manifest::CurrentConeInput::SingleFile { source } => {
            CurrentConeInput::SingleFile { source }
        }
    })
}
