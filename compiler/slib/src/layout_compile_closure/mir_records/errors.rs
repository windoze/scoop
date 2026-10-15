use scoop_identity::ConeIdentity;

#[derive(Debug)]
pub enum SharedMirReadError {
    Resource(scoop_wire::WireError),
    Front(Box<crate::CrossConeLayoutMirFrontValidationError>),
    Transport(Box<scoop_mir::MirTypeBridgeSectionError>),
}

impl From<scoop_wire::WireError> for SharedMirReadError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<crate::CrossConeLayoutMirFrontValidationError> for SharedMirReadError {
    fn from(error: crate::CrossConeLayoutMirFrontValidationError) -> Self {
        Self::Front(Box::new(error))
    }
}
impl From<scoop_mir::MirTypeBridgeSectionError> for SharedMirReadError {
    fn from(error: scoop_mir::MirTypeBridgeSectionError) -> Self {
        Self::Transport(Box::new(error))
    }
}
impl std::fmt::Display for SharedMirReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid MIR records: {self:?}")
    }
}
impl std::error::Error for SharedMirReadError {}

#[derive(Debug)]
pub struct CrossConeLayoutMirReadError {
    pub provider: ConeIdentity,
    pub source: Box<SharedMirReadError>,
}
impl CrossConeLayoutMirReadError {
    pub(super) fn new(provider: ConeIdentity, source: SharedMirReadError) -> Self {
        Self {
            provider,
            source: Box::new(source),
        }
    }
}
impl std::fmt::Display for CrossConeLayoutMirReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid MIR records for {}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutMirReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
