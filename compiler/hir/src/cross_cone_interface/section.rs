use crate::{
    CanonicalExportGenericDelegatesV1, DecodedCanonicalExportGenericDelegatesV1,
    GenericDelegateTemplateResolutionError, IndexedExportGenericDelegatesV1,
};
use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    CallableInterfaceRecordResolver, CallableInterfaceSetValidationError,
    CallableSourceInterfaceSetIndexError, CallableSourceInterfaceSetValidationError,
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1,
    CanonicalExportDefinitionSourcesV1, CanonicalExternalHirReferencesV1,
    CanonicalNominalInterfacesV1, CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1,
    CanonicalTypeAliasInterfacesV1, DecodedCanonicalCallableInterfacesV1,
    DecodedCanonicalCallableSourceInterfacesV1, DecodedCanonicalExportConstValuesV1,
    DecodedCanonicalExportDefaultTemplatesV1, DecodedCanonicalExportDefinitionSourcesV1,
    DecodedCanonicalExternalHirReferencesV1, DecodedCanonicalNominalInterfacesV1,
    DecodedCanonicalPropertyInterfacesV1, DecodedCanonicalPublicExportBindingsV1,
    DecodedCanonicalTypeAliasInterfacesV1, DefaultStatementReferenceResolver,
    ExportConstValueResolver, ExportConstValueSetValidationError, ExportDefaultTemplateLookupError,
    ExportDefaultTemplateSetIndexError, ExportDefaultTemplateSetValidationError,
    ExportDefinitionSourceSetValidationError, ExternalHirReferenceResolver,
    ExternalHirReferenceSetValidationError, IndexedCanonicalCallableSourceInterfacesV1,
    IndexedCanonicalExportDefaultTemplatesV1, NominalInterfaceRecordResolver,
    NominalInterfaceSetValidationError, PropertyInterfaceRecordResolver,
    PropertyInterfaceSetValidationError, PublicExportBindingResolver,
    PublicExportBindingSetValidationError, TypeAliasInterfaceRecordResolver,
    TypeAliasInterfaceSetValidationError,
};

use crate::{
    CanonicalExportGenericCallableBodiesV1, CanonicalExportGenericInitializationsV1,
    DecodedCanonicalExportGenericCallableBodiesV1, DecodedCanonicalExportGenericInitializationsV1,
    GenericCallableBodiesResolutionError, GenericCallableBodyIndexError,
    GenericInitializationResolutionError, IndexedExportGenericCallableBodiesV1,
    IndexedExportGenericInitializationsV1, TemplateFragmentIndexError,
};

mod alias_reference_closure;
mod const_type_reference_closure;
mod default_reference_closure;
mod definition_source_closure;
mod external_reference_closure;
mod inheritance_reference_closure;
pub use inheritance_reference_closure::ExternalHirInheritanceClosureValidationError;
mod internal_closures;
pub(crate) mod signature_nominal_walk;
mod signature_reference_closure;

pub use alias_reference_closure::{
    ExternalHirAliasClosureValidationError, ExternalHirAliasUseSiteV1,
};
pub use const_type_reference_closure::ExternalHirConstTypeClosureValidationError;
pub use default_reference_closure::{
    ExternalHirDefaultClosureValidationError, ExternalHirDefaultOriginMismatch,
    ExternalHirDefaultUseSiteV1,
};
pub use definition_source_closure::{
    ExportDefinitionSourceClosureValidationError, ExportDefinitionSourceUseSiteV1,
};
pub use external_reference_closure::CrossConeHirExternalReferenceValidationError;
pub use internal_closures::CrossConeHirInternalClosureValidationError;
pub use signature_reference_closure::{
    ExternalHirSignatureClosureValidationError, ExternalHirSignatureOriginMismatch,
    ExternalHirSignatureUseSiteV1,
};

/// The complete canonical HIR interface exported across a Cone boundary.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CrossConeHirInterfaceSectionV1 {
    public_bindings: CanonicalPublicExportBindingsV1,
    nominal_interfaces: CanonicalNominalInterfacesV1,
    callable_interfaces: CanonicalCallableInterfacesV1,
    property_interfaces: CanonicalPropertyInterfacesV1,
    type_aliases: CanonicalTypeAliasInterfacesV1,
    source_interfaces: CanonicalCallableSourceInterfacesV1,
    default_templates: CanonicalExportDefaultTemplatesV1,
    constants: CanonicalExportConstValuesV1,
    definition_sources: CanonicalExportDefinitionSourcesV1,
    external_references: CanonicalExternalHirReferencesV1,
    generic_callable_bodies: CanonicalExportGenericCallableBodiesV1,
    generic_initializations: CanonicalExportGenericInitializationsV1,
    generic_delegates: CanonicalExportGenericDelegatesV1,
}

impl CrossConeHirInterfaceSectionV1 {
    /// An interface containing no exported declarations.
    pub fn empty() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        public_bindings: CanonicalPublicExportBindingsV1,
        nominal_interfaces: CanonicalNominalInterfacesV1,
        callable_interfaces: CanonicalCallableInterfacesV1,
        property_interfaces: CanonicalPropertyInterfacesV1,
        type_aliases: CanonicalTypeAliasInterfacesV1,
        source_interfaces: CanonicalCallableSourceInterfacesV1,
        default_templates: CanonicalExportDefaultTemplatesV1,
        constants: CanonicalExportConstValuesV1,
        definition_sources: CanonicalExportDefinitionSourcesV1,
        external_references: CanonicalExternalHirReferencesV1,
        generic_callable_bodies: CanonicalExportGenericCallableBodiesV1,
        generic_initializations: CanonicalExportGenericInitializationsV1,
        generic_delegates: CanonicalExportGenericDelegatesV1,
    ) -> Self {
        Self {
            public_bindings,
            nominal_interfaces,
            callable_interfaces,
            property_interfaces,
            type_aliases,
            source_interfaces,
            default_templates,
            constants,
            definition_sources,
            external_references,
            generic_callable_bodies,
            generic_initializations,
            generic_delegates,
        }
    }

    pub const fn public_bindings(&self) -> &CanonicalPublicExportBindingsV1 {
        &self.public_bindings
    }

    pub const fn nominal_interfaces(&self) -> &CanonicalNominalInterfacesV1 {
        &self.nominal_interfaces
    }

    pub const fn callable_interfaces(&self) -> &CanonicalCallableInterfacesV1 {
        &self.callable_interfaces
    }

    pub const fn property_interfaces(&self) -> &CanonicalPropertyInterfacesV1 {
        &self.property_interfaces
    }

    pub const fn type_aliases(&self) -> &CanonicalTypeAliasInterfacesV1 {
        &self.type_aliases
    }

    pub const fn source_interfaces(&self) -> &CanonicalCallableSourceInterfacesV1 {
        &self.source_interfaces
    }

    pub const fn default_templates(&self) -> &CanonicalExportDefaultTemplatesV1 {
        &self.default_templates
    }

    pub const fn constants(&self) -> &CanonicalExportConstValuesV1 {
        &self.constants
    }

    pub const fn definition_sources(&self) -> &CanonicalExportDefinitionSourcesV1 {
        &self.definition_sources
    }

    pub const fn external_references(&self) -> &CanonicalExternalHirReferencesV1 {
        &self.external_references
    }

    pub const fn generic_callable_bodies(&self) -> &CanonicalExportGenericCallableBodiesV1 {
        &self.generic_callable_bodies
    }

    pub const fn generic_initializations(&self) -> &CanonicalExportGenericInitializationsV1 {
        &self.generic_initializations
    }

    pub const fn generic_delegates(&self) -> &CanonicalExportGenericDelegatesV1 {
        &self.generic_delegates
    }

    pub fn index_for_wire(
        &mut self,
    ) -> Result<IndexedCrossConeHirInterfaceSectionV1<'_>, CrossConeHirInterfaceIndexError> {
        let source_interfaces = self
            .source_interfaces
            .index_templates(&mut self.default_templates)
            .map_err(CrossConeHirInterfaceIndexError::SourceInterfaces)?;
        let default_templates = self
            .default_templates
            .index_locals()
            .map_err(CrossConeHirInterfaceIndexError::DefaultTemplates)?;
        let generic_callable_bodies = self
            .generic_callable_bodies
            .index_locals()
            .map_err(CrossConeHirInterfaceIndexError::GenericCallableBodies)?;
        let generic_initializations = self
            .generic_initializations
            .index_locals()
            .map_err(CrossConeHirInterfaceIndexError::GenericInitializations)?;
        let generic_delegates = self
            .generic_delegates
            .index_locals()
            .map_err(CrossConeHirInterfaceIndexError::GenericDelegates)?;
        Ok(IndexedCrossConeHirInterfaceSectionV1 {
            public_bindings: &self.public_bindings,
            nominal_interfaces: &self.nominal_interfaces,
            callable_interfaces: &self.callable_interfaces,
            property_interfaces: &self.property_interfaces,
            type_aliases: &self.type_aliases,
            source_interfaces,
            default_templates,
            constants: &self.constants,
            definition_sources: &self.definition_sources,
            external_references: &self.external_references,
            generic_callable_bodies,
            generic_initializations,
            generic_delegates,
        })
    }
}

pub struct IndexedCrossConeHirInterfaceSectionV1<'a> {
    public_bindings: &'a CanonicalPublicExportBindingsV1,
    nominal_interfaces: &'a CanonicalNominalInterfacesV1,
    callable_interfaces: &'a CanonicalCallableInterfacesV1,
    property_interfaces: &'a CanonicalPropertyInterfacesV1,
    type_aliases: &'a CanonicalTypeAliasInterfacesV1,
    source_interfaces: IndexedCanonicalCallableSourceInterfacesV1<'a>,
    default_templates: IndexedCanonicalExportDefaultTemplatesV1<'a>,
    constants: &'a CanonicalExportConstValuesV1,
    definition_sources: &'a CanonicalExportDefinitionSourcesV1,
    external_references: &'a CanonicalExternalHirReferencesV1,
    generic_callable_bodies: IndexedExportGenericCallableBodiesV1<'a>,
    generic_initializations: IndexedExportGenericInitializationsV1<'a>,
    generic_delegates: IndexedExportGenericDelegatesV1<'a>,
}

impl WireEncode for IndexedCrossConeHirInterfaceSectionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(13)?;
        encoder.field(1)?;
        self.public_bindings.encode(encoder)?;
        encoder.field(2)?;
        self.nominal_interfaces.encode(encoder)?;
        encoder.field(3)?;
        self.callable_interfaces.encode(encoder)?;
        encoder.field(4)?;
        self.property_interfaces.encode(encoder)?;
        encoder.field(5)?;
        self.type_aliases.encode(encoder)?;
        encoder.field(6)?;
        self.source_interfaces.encode(encoder)?;
        encoder.field(7)?;
        self.default_templates.encode(encoder)?;
        encoder.field(8)?;
        self.constants.encode(encoder)?;
        encoder.field(9)?;
        self.definition_sources.encode(encoder)?;
        encoder.field(10)?;
        self.external_references.encode(encoder)?;
        encoder.field(11)?;
        self.generic_callable_bodies.encode(encoder)?;
        encoder.field(12)?;
        self.generic_initializations.encode(encoder)?;
        encoder.field(13)?;
        self.generic_delegates.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCrossConeHirInterfaceSectionV1 {
    public_bindings: DecodedCanonicalPublicExportBindingsV1,
    nominal_interfaces: DecodedCanonicalNominalInterfacesV1,
    callable_interfaces: DecodedCanonicalCallableInterfacesV1,
    property_interfaces: DecodedCanonicalPropertyInterfacesV1,
    type_aliases: DecodedCanonicalTypeAliasInterfacesV1,
    source_interfaces: DecodedCanonicalCallableSourceInterfacesV1,
    default_templates: DecodedCanonicalExportDefaultTemplatesV1,
    constants: DecodedCanonicalExportConstValuesV1,
    definition_sources: DecodedCanonicalExportDefinitionSourcesV1,
    external_references: DecodedCanonicalExternalHirReferencesV1,
    generic_callable_bodies: DecodedCanonicalExportGenericCallableBodiesV1,
    generic_initializations: DecodedCanonicalExportGenericInitializationsV1,
    generic_delegates: DecodedCanonicalExportGenericDelegatesV1,
}

impl DecodedCrossConeHirInterfaceSectionV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CrossConeHirInterfaceSectionV1, CrossConeHirInterfaceResolutionError<E>>
    where
        R: CrossConeHirInterfaceResolver<E>,
    {
        let public_bindings = self.public_bindings.resolve(resolver).map_err(|error| {
            CrossConeHirInterfaceResolutionError::PublicBindings(Box::new(error))
        })?;
        let nominal_interfaces = self.nominal_interfaces.resolve(resolver).map_err(|error| {
            CrossConeHirInterfaceResolutionError::NominalInterfaces(Box::new(error))
        })?;
        let callable_interfaces = self
            .callable_interfaces
            .resolve(resolver)
            .map_err(|error| {
                CrossConeHirInterfaceResolutionError::CallableInterfaces(Box::new(error))
            })?;
        let property_interfaces = self
            .property_interfaces
            .resolve(resolver)
            .map_err(|error| {
                CrossConeHirInterfaceResolutionError::PropertyInterfaces(Box::new(error))
            })?;
        let type_aliases = self
            .type_aliases
            .resolve(resolver)
            .map_err(|error| CrossConeHirInterfaceResolutionError::TypeAliases(Box::new(error)))?;
        let mut default_templates = self
            .default_templates
            .resolve_at(resolver, &scoop_wire::WirePath::root().field(7))
            .map_err(|error| {
                CrossConeHirInterfaceResolutionError::DefaultTemplates(Box::new(error))
            })?;
        let source_interfaces = self
            .source_interfaces
            .resolve(resolver, &mut default_templates)
            .map_err(|error| {
                CrossConeHirInterfaceResolutionError::SourceInterfaces(Box::new(error))
            })?;
        let constants = self
            .constants
            .resolve(resolver)
            .map_err(|error| CrossConeHirInterfaceResolutionError::Constants(Box::new(error)))?;
        let definition_sources = self.definition_sources.resolve(resolver).map_err(|error| {
            CrossConeHirInterfaceResolutionError::DefinitionSources(Box::new(error))
        })?;
        let external_references = self
            .external_references
            .resolve_at(resolver, &scoop_wire::WirePath::root().field(10))
            .map_err(|error| {
                CrossConeHirInterfaceResolutionError::ExternalReferences(Box::new(error))
            })?;

        let generic_callable_bodies =
            self.generic_callable_bodies
                .resolve(resolver)
                .map_err(|error| {
                    CrossConeHirInterfaceResolutionError::GenericCallableBodies(Box::new(error))
                })?;
        let generic_initializations =
            self.generic_initializations
                .resolve(resolver)
                .map_err(|error| {
                    CrossConeHirInterfaceResolutionError::GenericInitializations(Box::new(error))
                })?;
        let generic_delegates = self.generic_delegates.resolve(resolver).map_err(|error| {
            CrossConeHirInterfaceResolutionError::GenericDelegates(Box::new(error))
        })?;
        Ok(CrossConeHirInterfaceSectionV1 {
            public_bindings,
            nominal_interfaces,
            callable_interfaces,
            property_interfaces,
            type_aliases,
            source_interfaces,
            default_templates,
            constants,
            definition_sources,
            external_references,
            generic_callable_bodies,
            generic_initializations,
            generic_delegates,
        })
    }
}

impl WireEncode for DecodedCrossConeHirInterfaceSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(13)?;
        encoder.field(1)?;
        self.public_bindings.encode(encoder)?;
        encoder.field(2)?;
        self.nominal_interfaces.encode(encoder)?;
        encoder.field(3)?;
        self.callable_interfaces.encode(encoder)?;
        encoder.field(4)?;
        self.property_interfaces.encode(encoder)?;
        encoder.field(5)?;
        self.type_aliases.encode(encoder)?;
        encoder.field(6)?;
        self.source_interfaces.encode(encoder)?;
        encoder.field(7)?;
        self.default_templates.encode(encoder)?;
        encoder.field(8)?;
        self.constants.encode(encoder)?;
        encoder.field(9)?;
        self.definition_sources.encode(encoder)?;
        encoder.field(10)?;
        self.external_references.encode(encoder)?;
        encoder.field(11)?;
        self.generic_callable_bodies.encode(encoder)?;
        encoder.field(12)?;
        self.generic_initializations.encode(encoder)?;
        encoder.field(13)?;
        self.generic_delegates.encode(encoder)
    }
}

impl WireDecode for DecodedCrossConeHirInterfaceSectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(13)?;
        Ok(Self {
            public_bindings: decoder.field(1, DecodedCanonicalPublicExportBindingsV1::decode)?,
            nominal_interfaces: decoder.field(2, DecodedCanonicalNominalInterfacesV1::decode)?,
            callable_interfaces: decoder.field(3, DecodedCanonicalCallableInterfacesV1::decode)?,
            property_interfaces: decoder.field(4, DecodedCanonicalPropertyInterfacesV1::decode)?,
            type_aliases: decoder.field(5, DecodedCanonicalTypeAliasInterfacesV1::decode)?,
            source_interfaces: decoder
                .field(6, DecodedCanonicalCallableSourceInterfacesV1::decode)?,
            default_templates: decoder
                .field(7, DecodedCanonicalExportDefaultTemplatesV1::decode)?,
            constants: decoder.field(8, DecodedCanonicalExportConstValuesV1::decode)?,
            definition_sources: decoder
                .field(9, DecodedCanonicalExportDefinitionSourcesV1::decode)?,
            external_references: decoder
                .field(10, DecodedCanonicalExternalHirReferencesV1::decode)?,
            generic_callable_bodies: decoder
                .field(11, DecodedCanonicalExportGenericCallableBodiesV1::decode)?,
            generic_initializations: decoder
                .field(12, DecodedCanonicalExportGenericInitializationsV1::decode)?,
            generic_delegates: decoder
                .field(13, DecodedCanonicalExportGenericDelegatesV1::decode)?,
        })
    }
}

pub trait CrossConeHirInterfaceResolver<E>:
    PublicExportBindingResolver<E>
    + NominalInterfaceRecordResolver<E>
    + CallableInterfaceRecordResolver<E>
    + PropertyInterfaceRecordResolver<E>
    + TypeAliasInterfaceRecordResolver<E>
    + DefaultStatementReferenceResolver<E>
    + ExportConstValueResolver<E>
    + ExternalHirReferenceResolver<E>
{
}

impl<R, E> CrossConeHirInterfaceResolver<E> for R where
    R: PublicExportBindingResolver<E>
        + NominalInterfaceRecordResolver<E>
        + CallableInterfaceRecordResolver<E>
        + PropertyInterfaceRecordResolver<E>
        + TypeAliasInterfaceRecordResolver<E>
        + DefaultStatementReferenceResolver<E>
        + ExportConstValueResolver<E>
        + ExternalHirReferenceResolver<E>
{
}

#[derive(Debug, Eq, PartialEq)]
pub enum CrossConeHirInterfaceIndexError {
    SourceInterfaces(CallableSourceInterfaceSetIndexError<ExportDefaultTemplateLookupError>),
    DefaultTemplates(ExportDefaultTemplateSetIndexError),
    GenericCallableBodies(GenericCallableBodyIndexError),
    GenericInitializations(TemplateFragmentIndexError),
    GenericDelegates(GenericCallableBodyIndexError),
}

impl fmt::Display for CrossConeHirInterfaceIndexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceInterfaces(error) => {
                write!(
                    formatter,
                    "cannot index callable source interfaces: {error}"
                )
            }
            Self::GenericCallableBodies(error) => {
                write!(formatter, "cannot index generic callable bodies: {error}")
            }
            Self::GenericDelegates(error) => {
                write!(formatter, "cannot index generic delegates: {error}")
            }
            Self::GenericInitializations(error) => {
                write!(formatter, "cannot index generic initializations: {error}")
            }
            Self::DefaultTemplates(error) => {
                write!(formatter, "cannot index default templates: {error}")
            }
        }
    }
}

impl std::error::Error for CrossConeHirInterfaceIndexError {}

#[derive(Debug)]
pub enum CrossConeHirInterfaceResolutionError<E> {
    PublicBindings(Box<PublicExportBindingSetValidationError<E>>),
    NominalInterfaces(Box<NominalInterfaceSetValidationError<E>>),
    CallableInterfaces(Box<CallableInterfaceSetValidationError<E>>),
    PropertyInterfaces(Box<PropertyInterfaceSetValidationError<E>>),
    TypeAliases(Box<TypeAliasInterfaceSetValidationError<E>>),
    SourceInterfaces(
        Box<CallableSourceInterfaceSetValidationError<E, ExportDefaultTemplateLookupError>>,
    ),
    DefaultTemplates(Box<ExportDefaultTemplateSetValidationError<E>>),
    GenericCallableBodies(Box<GenericCallableBodiesResolutionError<E>>),
    GenericInitializations(Box<GenericInitializationResolutionError<E>>),
    GenericDelegates(Box<GenericDelegateTemplateResolutionError<E>>),
    Constants(Box<ExportConstValueSetValidationError<E>>),
    DefinitionSources(Box<ExportDefinitionSourceSetValidationError<E>>),
    ExternalReferences(Box<ExternalHirReferenceSetValidationError<E>>),
}

impl<E: fmt::Display> fmt::Display for CrossConeHirInterfaceResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (field, error): (&str, &dyn fmt::Display) = match self {
            Self::PublicBindings(error) => ("public bindings", error),
            Self::NominalInterfaces(error) => ("nominal interfaces", error),
            Self::CallableInterfaces(error) => ("callable interfaces", error),
            Self::PropertyInterfaces(error) => ("property interfaces", error),
            Self::TypeAliases(error) => ("type aliases", error),
            Self::SourceInterfaces(error) => ("callable source interfaces", error),
            Self::DefaultTemplates(error) => ("default templates", error),
            Self::GenericCallableBodies(error) => ("generic callable bodies", error),
            Self::GenericInitializations(error) => ("generic initializations", error),
            Self::GenericDelegates(error) => ("generic delegates", error),
            Self::Constants(error) => ("constants", error),
            Self::DefinitionSources(error) => ("definition sources", error),
            Self::ExternalReferences(error) => ("external references", error),
        };
        write!(formatter, "invalid cross-Cone HIR {field}: {error}")
    }
}

impl<E: std::error::Error + 'static> std::error::Error for CrossConeHirInterfaceResolutionError<E> {}

#[cfg(test)]
mod tests;
