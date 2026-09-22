use super::*;

#[derive(Debug)]
pub enum DefaultSourceProfileBindingError {
    Resource(WireError),
    Coverage(SourceInventoryError),
    Mismatch {
        key: ProtectedDefaultTemplateKeyV1,
        expected: ProtectedDefaultWitnessSourceProfileV1,
        actual: ProtectedDefaultWitnessSourceProfileV1,
    },
    MissingProfile(ProtectedDefaultTemplateKeyV1),
}
impl DefaultSourceProfileBindingError {
    pub(super) fn coverage(error: SourceInventoryError) -> Self {
        match error {
            SourceInventoryError::Resource(error) => Self::Resource(error),
            error => Self::Coverage(error),
        }
    }
}
impl From<WireError> for DefaultSourceProfileBindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for DefaultSourceProfileBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Coverage(error) => error.fmt(f),
            Self::Mismatch {
                key,
                expected,
                actual,
            } => write!(
                f,
                "default {key:?} source profile {actual:?} differs from replayed {expected:?}"
            ),
            Self::MissingProfile(key) => write!(f, "missing bound default source profile {key:?}"),
        }
    }
}
impl std::error::Error for DefaultSourceProfileBindingError {}
