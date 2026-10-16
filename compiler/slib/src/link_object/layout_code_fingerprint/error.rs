use super::*;

#[derive(Debug)]
pub enum LayoutCodeFingerprintError {
    LinkContributionEncoding(scoop_wire::cbor::EncodeError),
    NativeContracts(NativeExternalContractCodeSetBuildError),
    Hash(HashError),
}

impl fmt::Display for LayoutCodeFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "failed to compute layout code fingerprint: {self:?}"
        )
    }
}

impl std::error::Error for LayoutCodeFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LinkContributionEncoding(source) => Some(source),
            Self::NativeContracts(source) => Some(source),
            Self::Hash(source) => Some(source),
        }
    }
}
