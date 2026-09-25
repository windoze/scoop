use std::fmt;

use scoop_wire::{WireError, WirePath};

use super::ExportDefaultBodyV1;
use crate::{
    DefaultTemplateProviderShapeV1, ExportDefinitionSourceSemanticAuthority,
    ExportDefinitionSourceSemanticValidationError, ExportDefinitionSourceV1,
    NominalInterfaceShapeAuthority, SignatureBinderScopeError, SignatureTypeSemanticError,
};

mod walk;

/// Supplies a local function's own generic arity from its independently validated
/// Function/GenericFunction canonical declaration key. Descriptor signatures and
/// captured owner arguments must not be used to infer this fact. This query does
/// not replace the separate artifact ownership, parent, or nested ABI proofs.
pub trait DefaultLocalFunctionSignatureAuthority<E> {
    fn default_local_function_own_binder_arity(
        &mut self,
        declaration: scoop_identity::CallableTemplateOrigin,
    ) -> Result<u32, E>;
}

impl ExportDefaultBodyV1 {
    /// Validates every provider-scoped type and inline definition origin in
    /// this body. Operation typing, local data flow, nested callable ABI, and
    /// the exact reference closure remain separate semantic passes.
    pub fn validate_provider_envelope_semantics<A, E>(
        &self,
        provider: DefaultTemplateProviderShapeV1,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>>
    where
        A: NominalInterfaceShapeAuthority<E>
            + ExportDefinitionSourceSemanticAuthority<E>
            + DefaultLocalFunctionSignatureAuthority<E>,
    {
        walk::validate(self, provider, authority, path)
    }

    /// Validates the complete provider type envelope without reinterpreting
    /// origins. Used only after artifact-bound source-origin validation.
    pub(crate) fn validate_provider_types_semantics<
        A: NominalInterfaceShapeAuthority<E> + DefaultLocalFunctionSignatureAuthority<E>,
        E,
    >(
        &self,
        provider: DefaultTemplateProviderShapeV1,
        authority: &mut A,

        path: &WirePath,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        walk::validate_types(self, provider, authority, path)
    }

    /// Visits each inline definition source with its actual diagnostic path.
    pub(crate) fn visit_definition_sources<V, E>(
        &self,
        visitor: &mut V,

        path: &WirePath,
    ) -> Result<(), E>
    where
        V: FnMut(&ExportDefinitionSourceV1, DefaultBodyOriginSiteV1, &WirePath) -> Result<(), E>,
        E: From<WireError>,
    {
        walk::visit_definition_sources(self, visitor, path)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBodyOriginSiteV1 {
    Statement,
    Expression,
    WhenArm,
    Catch,
    BindingAction,
    IteratorConformance,
    IteratorNext,
    CaptureFirstUse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultBodyProviderTypeSiteV1 {
    ExpressionResult,
    StructConstructOwner,
    FunctionCoercionSource,
    FunctionCoercionTarget,
    SizeOfOperand,
    AlignOfOperand,
    InstanceCheck,
    ArrayAssemblyElement,
    ArrayAssemblyResult,
    CallableCallFunction,
    CallReceiver,
    CallableOwner,
    CallableTypeArgument { index: usize },
    BoundCallableBound,
    BoundCallableInstantiatedSignature,
    BoundCallableReceiverParameter,
    DerivedEqualityOwner,
    ConstructorOwner,
    EnumVariantOwner,
    EnumVariantFieldOwner,
    FieldOwner,
    NestedCallableFunction,
    NestedCallableBodyTypeArgument { index: usize },
    CaptureValue,
    PatternSubject,
    PatternStructOwner,
    WhenFallbackSubject,
    WhenFallbackEnumOwner,
    CatchValue,
    BindingTemporaryValue,
    BindingLeafValue,
    BindingShapeOwner,
    BindingProjectionOwner,
    IteratorInterface,
}

#[derive(Debug, Eq, PartialEq)]
pub enum DefaultBodyProviderEnvelopeSemanticValidationError<E> {
    LocalFunctionBinders {
        declaration: scoop_identity::CallableTemplateOrigin,
        definition_origin: Box<ExportDefinitionSourceV1>,
        error: E,
    },
    Type {
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: Box<ExportDefinitionSourceV1>,
        error: Box<SignatureTypeSemanticError<E>>,
    },
    Binder {
        site: DefaultBodyProviderTypeSiteV1,
        definition_origin: Box<ExportDefinitionSourceV1>,
        error: SignatureBinderScopeError,
    },
    Origin {
        site: DefaultBodyOriginSiteV1,
        definition_origin: Box<ExportDefinitionSourceV1>,
        error: Box<ExportDefinitionSourceSemanticValidationError<E>>,
    },
    Resource(WireError),
}

impl<E: fmt::Display> fmt::Display for DefaultBodyProviderEnvelopeSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalFunctionBinders {
                declaration, error, ..
            } => write!(
                formatter,
                "invalid local function binder authority for {declaration:?}: {error}"
            ),
            Self::Type { site, error, .. } => {
                write!(
                    formatter,
                    "invalid provider-scoped type at {site:?}: {error}"
                )
            }
            Self::Binder { site, error, .. } => {
                write!(formatter, "invalid provider binder at {site:?}: {error}")
            }
            Self::Origin { site, error, .. } => {
                write!(formatter, "invalid definition origin at {site:?}: {error}")
            }
            Self::Resource(error) => {
                write!(
                    formatter,
                    "default body validation resource failure: {error}"
                )
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for DefaultBodyProviderEnvelopeSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
