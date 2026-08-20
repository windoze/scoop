use super::*;

#[derive(Debug)]
pub enum LayoutCodeFingerprintError {
    NativeProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    TargetMismatch,
    DefinedSymbols(DefinedLinkSymbolOwnerBuildError),
    DefinedSymbolSetMismatch,
    UndefinedSymbolPartitionMismatch,
    ExternalShapeClosureMismatch,
    CrossConeLinkClosure(CrossConeLinkClosureBuildError),
    LayoutLinkClosure(LayoutLinkClosureError),
    LinkContributionEncoding(scoop_wire::cbor::EncodeError),
    NativeContracts(NativeExternalContractCodeSetBuildError),
    Hash(HashError),
    Resource(scoop_wire::WireError),
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
            Self::DefinedSymbols(source) => Some(source),
            Self::CrossConeLinkClosure(source) => Some(source),
            Self::LayoutLinkClosure(source) => Some(source),
            Self::LinkContributionEncoding(source) => Some(source),
            Self::NativeContracts(source) => Some(source),
            Self::Hash(source) => Some(source),
            Self::Resource(source) => Some(source),
            _ => None,
        }
    }
}
