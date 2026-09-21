use super::*;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportDefaultReferenceTargetTypeSiteV1 {
    CallableOwner,
    CallableTypeArgument { index: usize },
    BoundCallableReceiverParameter,
    BoundCallableBound,
    BoundCallableInstantiatedSignature,
    DerivedEqualityOwner,
    ConstructorOwner,
    TypeTarget,
    FieldOwner,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultReferenceValidationError<E> {
    WitnessOwner {
        expected: CallableTemplateOrigin,
        actual: CallableTemplateOrigin,
    },
    CallDomain {
        expected: ExportDefaultCallDomainV1,
        actual: ExportDefaultCallDomainV1,
    },
    DefinitionOrigin(ExportDefinitionSourceSemanticValidationError<E>),
    Type {
        site: ExportDefaultReferenceTargetTypeSiteV1,
        definition_origin: Box<ExportDefinitionSourceV1>,
        error: Box<SignatureTypeSemanticError<E>>,
    },
    Binder {
        site: ExportDefaultReferenceTargetTypeSiteV1,
        definition_origin: Box<ExportDefinitionSourceV1>,
        error: SignatureBinderScopeError,
    },
    BinderTypeTarget {
        depth: u32,
        index: u32,
    },
    Target(E),
    Resource(WireError),
}

impl<E: fmt::Display> fmt::Display for ExportDefaultReferenceValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WitnessOwner { expected, actual } => write!(
                formatter,
                "default reference witness owner {actual:?} differs from template owner {expected:?}"
            ),
            Self::CallDomain { expected, actual } => write!(
                formatter,
                "default reference call domain {actual:?} differs from owner domain {expected:?}"
            ),
            Self::DefinitionOrigin(error) => {
                write!(
                    formatter,
                    "invalid default reference definition origin: {error}"
                )
            }
            Self::Type { site, error, .. } => {
                write!(
                    formatter,
                    "invalid default reference type at {site:?}: {error}"
                )
            }
            Self::Binder { site, error, .. } => {
                write!(
                    formatter,
                    "invalid default reference binder at {site:?}: {error}"
                )
            }
            Self::BinderTypeTarget { depth, index } => write!(
                formatter,
                "default type reference cannot target provider binder ({depth}, {index})"
            ),
            Self::Target(error) => write!(formatter, "invalid default reference target: {error}"),
            Self::Resource(error) => {
                write!(
                    formatter,
                    "default reference validation resource failure: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultReferenceValidationError<E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefaultReferenceSetSemanticValidationError<E> {
    OwnerInterface {
        expected: CallableTemplateOrigin,
        actual: CallableTemplateOrigin,
    },
    Record {
        kind: ExportDefaultReferenceKindV1,
        index: usize,
        error: Box<ExportDefaultReferenceValidationError<E>>,
    },
}

impl<E: fmt::Display> fmt::Display for ExportDefaultReferenceSetSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnerInterface { expected, actual } => write!(
                formatter,
                "default-template owner {expected:?} does not match reference owner interface {actual:?}"
            ),
            Self::Record { kind, index, error } => {
                write!(
                    formatter,
                    "invalid {kind} default reference at index {index}: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for ExportDefaultReferenceSetSemanticValidationError<E>
{
}

impl<E> From<WireError> for ExportDefaultReferenceValidationError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
