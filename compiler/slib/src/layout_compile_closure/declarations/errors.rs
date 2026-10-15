use scoop_identity::ConeIdentity;
use scoop_wire::WireError;

#[derive(Debug)]
pub struct CrossConeLayoutHirDeclarationError {
    pub provider: ConeIdentity,
    pub source: Box<CrossConeHirDeclarationValidationError>,
}

#[derive(Debug)]
pub enum CrossConeHirDeclarationValidationError {
    Resource(WireError),
    AliasReferences(crate::CrossConeHirAliasReferenceError),
    AliasExpansion(scoop_hir::TypeAliasExpansionError),
}

impl std::fmt::Display for CrossConeLayoutHirDeclarationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid HIR declarations for {}: {}",
            self.provider, self.source
        )
    }
}

impl std::error::Error for CrossConeLayoutHirDeclarationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}

impl std::fmt::Display for CrossConeHirDeclarationValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::AliasReferences(error) => error.fmt(f),
            Self::AliasExpansion(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for CrossConeHirDeclarationValidationError {}
