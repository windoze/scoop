//! Typed failures for strong-profile Compile validation.

use super::*;

#[derive(Debug)]
pub enum StrongProfileFoundationError {
    HirStructure(HirFoundationValidationError),
    MirStructure(MirFoundationValidationError),
    LirStructure(LirFoundationValidationError),
    HirOdr(OdrFreeHirFoundationError),
    MirOdr(OdrFreeMirFoundationError),
    LirOdr(ConeLirFoundationError),
}

impl fmt::Display for StrongProfileFoundationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong-profile foundation: {self:?}")
    }
}

impl std::error::Error for StrongProfileFoundationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::HirStructure(error) => error,
            Self::MirStructure(error) => error,
            Self::LirStructure(error) => error,
            Self::HirOdr(error) => error,
            Self::MirOdr(error) => error,
            Self::LirOdr(error) => error,
        })
    }
}
