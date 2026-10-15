use scoop_identity::ConeIdentity;

#[derive(Debug)]
pub struct CrossConeLayoutLirProductionDataError {
    pub provider: ConeIdentity,
    pub source: scoop_lir::LinkDataError,
}
impl std::fmt::Display for CrossConeLayoutLirProductionDataError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid saved production data for {:?}: {}",
            self.provider, self.source
        )
    }
}
impl std::error::Error for CrossConeLayoutLirProductionDataError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
