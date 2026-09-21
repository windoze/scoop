use std::fmt;

use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::ExportDefaultBodyV1;
use crate::{
    DefaultTemplateProviderShapeV1, ExportDefinitionSourceSemanticAuthority,
    ExportDefinitionSourceSemanticValidationError, ExportDefinitionSourceV1,
    NominalInterfaceShapeAuthority, SignatureBinderScopeError, SignatureTypeSemanticError,
};

mod walk;

impl ExportDefaultBodyV1 {
    /// Validates every provider-scoped type and inline definition origin in
    /// this body. Operation typing, local data flow, nested callable ABI, and
    /// the exact reference closure remain separate semantic passes.
    pub fn validate_provider_envelope_semantics<A, E>(
        &self,
        provider: DefaultTemplateProviderShapeV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>>
    where
        A: NominalInterfaceShapeAuthority<E> + ExportDefinitionSourceSemanticAuthority<E>,
    {
        walk::validate(self, provider, authority, meter, path)
    }

    /// Validates the complete provider type envelope without reinterpreting
    /// origins. Used only after artifact-bound source-origin validation.
    pub(crate) fn validate_provider_types_semantics<A: NominalInterfaceShapeAuthority<E>, E>(
        &self,
        provider: DefaultTemplateProviderShapeV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), DefaultBodyProviderEnvelopeSemanticValidationError<E>> {
        walk::validate_types(self, provider, authority, meter, path)
    }

    /// Visits every inline definition source using the same complete,
    /// resource-bounded traversal as provider-envelope validation.
    pub(crate) fn visit_definition_sources(
        &self,
        visitor: &mut dyn FnMut(&ExportDefinitionSourceV1, DefaultBodyOriginSiteV1),
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), WireError> {
        self.visit_definition_sources_metered(
            &mut |source, site, _, _| {
                visitor(source, site);
                Ok(())
            },
            meter,
            path,
        )
    }
    /// Shares the walk's meter with each fallible inline-origin consumer.
    pub(crate) fn visit_definition_sources_metered<V, E>(
        &self,
        visitor: &mut V,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), E>
    where
        V: FnMut(
            &ExportDefinitionSourceV1,
            DefaultBodyOriginSiteV1,
            &mut BudgetMeter,
            &WirePath,
        ) -> Result<(), E>,
        E: From<WireError>,
    {
        walk::visit_definition_sources(self, visitor, meter, path)
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
