use std::fmt;

use scoop_identity::ConeIdentity;
use scoop_wire::{BudgetMeter, WirePath};

use super::{
    CrossConeHirExternalReferenceValidationError, CrossConeHirInterfaceSectionV1,
    CrossConeHirInternalClosureValidationError,
};
use crate::{
    CallableInterfaceSemanticAuthority, CallableInterfaceSetSemanticValidationError,
    CallableSourceInterfaceSemanticAuthority, CallableSourceInterfaceSetSemanticValidationError,
    CanonicalDirectPublicSurfaceV1, DefaultLocalDataFlowSemanticAuthority,
    DefaultNestedCallableSemanticAuthority, DefaultOperationTypingSemanticAuthority,
    DefaultReferenceSemanticAuthority, DefaultTemplateOriginSemanticAuthority,
    DefaultTemplateRootSemanticAuthority, ExportConstValueSemanticAuthority,
    ExportConstValueSetSemanticValidationError,
    ExportDefaultTemplateSetEnvelopeSemanticValidationError,
    ExportDefinitionSourceSetSemanticValidationError, ExternalHirReferenceSemanticAuthority,
    NominalInterfaceSemanticAuthority, NominalInterfaceSetSemanticValidationError,
    PropertyInterfaceSemanticAuthority, PropertyInterfaceSetSemanticValidationError,
    PublicExportBindingClosureValidationError, TypeAliasInterfaceSemanticAuthority,
    TypeAliasInterfaceSetSemanticValidationError,
};

/// Complete definition, source, route, and dependency authority required to
/// validate one resolved cross-Cone HIR interface section.
///
/// Implementations must answer from the current artifact's validated HIR
/// foundation plus the already validated dependency closure. A display name,
/// FQN, link symbol, or unrestricted artifact scan is not semantic authority.
pub trait CrossConeHirInterfaceSemanticAuthority<E>:
    NominalInterfaceSemanticAuthority<E>
    + CallableInterfaceSemanticAuthority<E>
    + PropertyInterfaceSemanticAuthority<E>
    + TypeAliasInterfaceSemanticAuthority<E>
    + CallableSourceInterfaceSemanticAuthority<E>
    + DefaultTemplateRootSemanticAuthority<E>
    + DefaultTemplateOriginSemanticAuthority<E>
    + DefaultLocalDataFlowSemanticAuthority<E>
    + DefaultOperationTypingSemanticAuthority<E>
    + DefaultNestedCallableSemanticAuthority<E>
    + DefaultReferenceSemanticAuthority<E>
    + ExportConstValueSemanticAuthority<E>
    + ExternalHirReferenceSemanticAuthority<E>
{
}

impl<A, E> CrossConeHirInterfaceSemanticAuthority<E> for A where
    A: NominalInterfaceSemanticAuthority<E>
        + CallableInterfaceSemanticAuthority<E>
        + PropertyInterfaceSemanticAuthority<E>
        + TypeAliasInterfaceSemanticAuthority<E>
        + CallableSourceInterfaceSemanticAuthority<E>
        + DefaultTemplateRootSemanticAuthority<E>
        + DefaultTemplateOriginSemanticAuthority<E>
        + DefaultLocalDataFlowSemanticAuthority<E>
        + DefaultOperationTypingSemanticAuthority<E>
        + DefaultNestedCallableSemanticAuthority<E>
        + DefaultReferenceSemanticAuthority<E>
        + ExportConstValueSemanticAuthority<E>
        + ExternalHirReferenceSemanticAuthority<E>
{
}

impl CrossConeHirInterfaceSectionV1 {
    /// Validates all ten tables as one semantic interface transaction.
    ///
    /// The order is fixed: section-internal exact closures, definition
    /// sources, declaration interfaces, source/default/const payloads,
    /// re-export routes, then the exact external-reference closure. Callers
    /// must not publish or import this section unless the whole pass succeeds.
    pub fn validate_semantics<A, E>(
        &self,
        current: ConeIdentity,
        direct_surface: &CanonicalDirectPublicSurfaceV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), CrossConeHirInterfaceSemanticValidationError<E>>
    where
        A: CrossConeHirInterfaceSemanticAuthority<E>,
    {
        self.validate_internal_closures(direct_surface, meter, path)
            .map_err(|error| {
                CrossConeHirInterfaceSemanticValidationError::Internal(Box::new(error))
            })?;
        self.definition_sources()
            .validate_semantics(authority)
            .map_err(|error| {
                CrossConeHirInterfaceSemanticValidationError::DefinitionSources(Box::new(error))
            })?;
        self.nominal_interfaces()
            .validate_semantics(authority)
            .map_err(|error| {
                CrossConeHirInterfaceSemanticValidationError::NominalInterfaces(Box::new(error))
            })?;
        self.callable_interfaces()
            .validate_semantics(authority)
            .map_err(|error| {
                CrossConeHirInterfaceSemanticValidationError::CallableInterfaces(Box::new(error))
            })?;
        self.property_interfaces()
            .validate_semantics(authority)
            .map_err(|error| {
                CrossConeHirInterfaceSemanticValidationError::PropertyInterfaces(Box::new(error))
            })?;
        self.type_aliases()
            .validate_semantics(authority)
            .map_err(|error| {
                CrossConeHirInterfaceSemanticValidationError::TypeAliases(Box::new(error))
            })?;
        self.source_interfaces()
            .validate_semantics(self.callable_interfaces(), authority)
            .map_err(|error| {
                CrossConeHirInterfaceSemanticValidationError::SourceInterfaces(Box::new(error))
            })?;
        self.default_templates()
            .validate_envelope_semantics(
                self.callable_interfaces(),
                self.source_interfaces(),
                authority,
                meter,
                &path.clone().field(7),
            )
            .map_err(|error| {
                CrossConeHirInterfaceSemanticValidationError::DefaultTemplates(Box::new(error))
            })?;
        self.constants()
            .validate_semantics(authority)
            .map_err(|error| {
                CrossConeHirInterfaceSemanticValidationError::Constants(Box::new(error))
            })?;
        self.public_bindings()
            .validate_route_closure(current, authority)
            .map_err(|error| {
                CrossConeHirInterfaceSemanticValidationError::Routes(Box::new(error))
            })?;
        self.validate_external_reference_closure(authority, meter, path)
            .map_err(|error| {
                CrossConeHirInterfaceSemanticValidationError::ExternalReferences(Box::new(error))
            })
    }
}

#[derive(Debug)]
pub enum CrossConeHirInterfaceSemanticValidationError<E> {
    Internal(Box<CrossConeHirInternalClosureValidationError>),
    DefinitionSources(Box<ExportDefinitionSourceSetSemanticValidationError<E>>),
    NominalInterfaces(Box<NominalInterfaceSetSemanticValidationError<E>>),
    CallableInterfaces(Box<CallableInterfaceSetSemanticValidationError<E>>),
    PropertyInterfaces(Box<PropertyInterfaceSetSemanticValidationError<E>>),
    TypeAliases(Box<TypeAliasInterfaceSetSemanticValidationError<E>>),
    SourceInterfaces(Box<CallableSourceInterfaceSetSemanticValidationError<E>>),
    DefaultTemplates(Box<ExportDefaultTemplateSetEnvelopeSemanticValidationError<E>>),
    Constants(Box<ExportConstValueSetSemanticValidationError<E>>),
    Routes(Box<PublicExportBindingClosureValidationError>),
    ExternalReferences(Box<CrossConeHirExternalReferenceValidationError<E>>),
}

impl<E: fmt::Display> fmt::Display for CrossConeHirInterfaceSemanticValidationError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (phase, error): (&str, &dyn fmt::Display) = match self {
            Self::Internal(error) => ("internal closure", error.as_ref()),
            Self::DefinitionSources(error) => ("definition sources", error.as_ref()),
            Self::NominalInterfaces(error) => ("nominal interfaces", error.as_ref()),
            Self::CallableInterfaces(error) => ("callable interfaces", error.as_ref()),
            Self::PropertyInterfaces(error) => ("property interfaces", error.as_ref()),
            Self::TypeAliases(error) => ("type aliases", error.as_ref()),
            Self::SourceInterfaces(error) => ("callable source interfaces", error.as_ref()),
            Self::DefaultTemplates(error) => ("default templates", error.as_ref()),
            Self::Constants(error) => ("constants", error.as_ref()),
            Self::Routes(error) => ("re-export routes", error.as_ref()),
            Self::ExternalReferences(error) => ("external references", error.as_ref()),
        };
        write!(
            formatter,
            "invalid complete cross-Cone HIR interface {phase}: {error}"
        )
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for CrossConeHirInterfaceSemanticValidationError<E>
{
}

#[cfg(test)]
mod tests;
