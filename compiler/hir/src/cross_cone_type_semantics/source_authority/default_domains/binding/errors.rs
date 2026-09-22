use super::*;

#[derive(Debug)]
pub enum DefaultSourceTypeDomainBindingError {
    Resource(WireError),
    Foundation {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    Target {
        key: ProtectedDefaultTemplateKeyV1,
        index: u32,
        error: Box<DefaultSourceDomainError>,
    },
    Witness {
        key: ProtectedDefaultTemplateKeyV1,
        index: u32,
    },
    Encoding(scoop_wire::cbor::EncodeError),
}
impl BindingError {
    pub(super) fn target(key: ProtectedDefaultTemplateKeyV1, index: u32, error: Error) -> Self {
        match error {
            Error::Resource(error) => Self::Resource(error),
            error => Self::Target {
                key,
                index,
                error: Box::new(error),
            },
        }
    }
}
impl From<WireError> for BindingError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for BindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Encoding(e) => e.fmt(f),
            Self::Foundation { expected, actual } => write!(
                f,
                "default type domains for {expected} and declarations for {actual} do not share the same bound foundation"
            ),
            Self::Target { key, index, error } => write!(
                f,
                "default {key:?} type reference {index} has invalid source domain: {error}"
            ),
            Self::Witness { key, index } => write!(
                f,
                "default {key:?} type reference {index} target-domain witness differs from its source domain"
            ),
        }
    }
}
impl std::error::Error for BindingError {}
