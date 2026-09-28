use std::fmt;

use scoop_wire::{WireError, WirePath};

use super::ExportDefaultBodyV1;
use crate::{
    DefaultTemplateProviderShapeV1, ExportDefinitionSourceV1, NominalInterfaceShapeAuthority,
    SignatureBinderScopeError, SignatureTypeSemanticError,
};

mod walk;

/// Supplies a local function's own generic arity from its independently validated
/// Function/GenericFunction canonical declaration key. Descriptor signatures and
/// captured owner arguments must not be used to infer this fact. The query
/// supplements the resolved declaration, parent and body-argument records.
pub trait DefaultLocalFunctionSignatureAuthority<E> {
    fn default_local_function_own_binder_arity(
        &mut self,
        declaration: scoop_identity::CallableTemplateOrigin,
    ) -> Result<u32, E>;
}

impl ExportDefaultBodyV1 {
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
        walk::visit_definition_sources(self, visitor, &mut |_, _| Ok(()), path)
    }
}

impl crate::ExportGenericCallableBodyV1 {
    /// Visits the actual definition locations in a callable template root.
    pub fn visit_definition_sources<V, E>(&self, visitor: &mut V, path: &WirePath) -> Result<(), E>
    where
        V: FnMut(&ExportDefinitionSourceV1) -> Result<(), E>,
        E: From<WireError>,
    {
        visitor(self.definition_origin())?;
        for local in self.locals().records() {
            if let crate::TemplateLocalDefinitionV1::Source(origin) = local.definition() {
                visitor(origin)?;
            }
        }
        walk::visit_statement_sources(
            self.statements(),
            &mut |source, _, _| visitor(source),
            &mut |_, _| Ok(()),
            &path.clone().field(4),
        )
    }
}

impl crate::ExportTemplateFragmentV1 {
    pub fn visit_definition_sources<V, E>(&self, visitor: &mut V, path: &WirePath) -> Result<(), E>
    where
        V: FnMut(&ExportDefinitionSourceV1) -> Result<(), E>,
        E: From<WireError>,
    {
        for local in self.locals().records() {
            if let crate::TemplateLocalDefinitionV1::Source(origin) = local.definition() {
                visitor(origin)?;
            }
        }
        walk::visit_fragment_sources(
            self,
            &mut |source, _, _| visitor(source),
            &mut |_, _| Ok(()),
            path,
        )
    }
}

impl ExportDefaultBodyV1 {
    pub(crate) fn visit_evaluation_origins<V, E>(
        &self,
        visitor: &mut V,
        path: &WirePath,
    ) -> Result<(), E>
    where
        V: FnMut(&scoop_identity::EvaluationOrigin) -> Result<(), E>,
        E: From<WireError>,
    {
        walk::visit_definition_sources(
            self,
            &mut |_, _, _| Ok(()),
            &mut |source, _| visitor(source),
            path,
        )
    }
}

impl crate::ExportGenericCallableBodyV1 {
    pub(crate) fn visit_evaluation_origins<V, E>(
        &self,
        visitor: &mut V,
        path: &WirePath,
    ) -> Result<(), E>
    where
        V: FnMut(&scoop_identity::EvaluationOrigin) -> Result<(), E>,
        E: From<WireError>,
    {
        walk::visit_statement_sources(
            self.statements(),
            &mut |_, _, _| Ok(()),
            &mut |source, _| visitor(source),
            path,
        )
    }
}

impl crate::ExportTemplateFragmentV1 {
    pub(crate) fn visit_evaluation_origins<V, E>(
        &self,
        visitor: &mut V,
        path: &WirePath,
    ) -> Result<(), E>
    where
        V: FnMut(&scoop_identity::EvaluationOrigin) -> Result<(), E>,
        E: From<WireError>,
    {
        walk::visit_fragment_sources(
            self,
            &mut |_, _, _| Ok(()),
            &mut |source, _| visitor(source),
            path,
        )
    }
}

impl crate::CrossConeHirInterfaceSectionV1 {
    pub fn visit_template_evaluation_origins<V, E>(
        &self,
        visitor: &mut V,
        path: &WirePath,
    ) -> Result<(), E>
    where
        V: FnMut(&scoop_identity::EvaluationOrigin) -> Result<(), E>,
        E: From<WireError>,
    {
        for template in self.default_templates().records() {
            template
                .body()
                .visit_evaluation_origins(visitor, &path.clone().field(7))?;
        }
        for body in self.generic_callable_bodies().records() {
            body.visit_evaluation_origins(visitor, &path.clone().field(11))?;
        }
        for initialization in self.generic_initializations().records() {
            for fragment in initialization.fragments() {
                fragment.visit_evaluation_origins(visitor, &path.clone().field(12))?;
            }
        }
        Ok(())
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
