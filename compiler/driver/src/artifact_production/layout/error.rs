use super::*;

#[derive(Debug)]
pub enum LayoutArtifactProductionError {
    Objects(Box<ObjectError>),
    Layout(Box<slib::LayoutLinkClosureError>),
    Code(Box<slib::LayoutCodeFingerprintError>),
    Assembly(Box<slib::CrossConeLayoutArtifactWriteError>),
    TargetSelectionMismatch,
}

impl From<ObjectError> for LayoutArtifactProductionError {
    fn from(error: ObjectError) -> Self {
        Self::Objects(Box::new(error))
    }
}

impl std::fmt::Display for LayoutArtifactProductionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Objects(error) => error.fmt(f),
            Self::Layout(error) => error.fmt(f),
            Self::Code(error) => error.fmt(f),
            Self::Assembly(error) => error.fmt(f),
            Self::TargetSelectionMismatch => {
                f.write_str("the layout archive changed the emitted target selection")
            }
        }
    }
}

impl std::error::Error for LayoutArtifactProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Objects(error) => Some(error.as_ref()),
            Self::Layout(error) => Some(error.as_ref()),
            Self::Code(error) => Some(error.as_ref()),
            Self::Assembly(error) => Some(error.as_ref()),
            Self::TargetSelectionMismatch => None,
        }
    }
}
