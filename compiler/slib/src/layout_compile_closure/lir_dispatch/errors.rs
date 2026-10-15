use scoop_identity::ConeIdentity;

#[derive(Debug)]
pub enum SharedLirDispatchValidationError {
    Read(scoop_lir::LinkDataError),
    Resource(scoop_wire::WireError),
}
impl From<scoop_lir::LinkDataError> for SharedLirDispatchValidationError {
    fn from(error: scoop_lir::LinkDataError) -> Self {
        Self::Read(error)
    }
}
impl From<scoop_wire::WireError> for SharedLirDispatchValidationError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for SharedLirDispatchValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid dispatch records: {self:?}")
    }
}
impl std::error::Error for SharedLirDispatchValidationError {}

#[derive(Debug)]
pub struct CrossConeLayoutLirDispatchError {
    pub provider: ConeIdentity,
    pub source: Box<SharedLirDispatchValidationError>,
}
impl CrossConeLayoutLirDispatchError {
    pub(super) fn new(provider: ConeIdentity, source: SharedLirDispatchValidationError) -> Self {
        Self {
            provider,
            source: Box::new(source),
        }
    }
}
impl std::fmt::Display for CrossConeLayoutLirDispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid dispatch exports for {:?}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutLirDispatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.source.as_ref())
    }
}
