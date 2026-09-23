use super::SourceAccessDomainResolutionError;
use super::{CallableDeclarationId, ExportDefaultCallDomainV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicDefaultWitnessError {
    Owner {
        expected: CallableDeclarationId,
        actual: CallableDeclarationId,
    },
    CallDomain {
        expected: ExportDefaultCallDomainV1,
        actual: Option<ExportDefaultCallDomainV1>,
    },
    RestrictedTarget,
}
impl std::fmt::Display for PublicDefaultWitnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owner { expected, actual } => write!(
                f,
                "default witness owner {actual:?} differs from publisher {expected:?}"
            ),
            Self::CallDomain { expected, actual } => write!(
                f,
                "default witness call domain {actual:?} differs from public domain {expected:?}"
            ),
            Self::RestrictedTarget => {
                f.write_str("a public default reference has a restricted target domain")
            }
        }
    }
}
impl std::error::Error for PublicDefaultWitnessError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportDefaultAccessWitnessBuildError {
    AccessorOwner,
    SlotForConstructor,
}
impl std::fmt::Display for ExportDefaultAccessWitnessBuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AccessorOwner => f.write_str("a property accessor cannot own a default witness"),
            Self::SlotForConstructor => {
                f.write_str("a constructor default cannot have a slot domain")
            }
        }
    }
}
impl std::error::Error for ExportDefaultAccessWitnessBuildError {}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultAccessWitnessResolutionError<E> {
    Owner(E),
    Domain(SourceAccessDomainResolutionError<E>),
    Build(ExportDefaultAccessWitnessBuildError),
}
impl<E> From<SourceAccessDomainResolutionError<E>>
    for ExportDefaultAccessWitnessResolutionError<E>
{
    fn from(error: SourceAccessDomainResolutionError<E>) -> Self {
        Self::Domain(error)
    }
}
impl<E: std::fmt::Display> std::fmt::Display for ExportDefaultAccessWitnessResolutionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Owner(error) => write!(f, "invalid default witness owner: {error}"),
            Self::Domain(error) => error.fmt(f),
            Self::Build(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultAccessWitnessResolutionError<E>
{
}
