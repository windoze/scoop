use super::*;
use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

mod errors;
pub use errors::*;

pub trait TypeSemanticsSectionResolver<E>:
    NominalRepresentationResolver<E>
    + NominalInheritanceInterfaceResolver<E>
    + DefaultStatementReferenceResolver<E>
    + ProtectedDefaultReferenceResolver<E>
    + SelectedTypeUseResolver<E>
{
}
impl<R, E> TypeSemanticsSectionResolver<E> for R where
    R: NominalRepresentationResolver<E>
        + NominalInheritanceInterfaceResolver<E>
        + DefaultStatementReferenceResolver<E>
        + ProtectedDefaultReferenceResolver<E>
        + SelectedTypeUseResolver<E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCrossConeTypeSemanticsSectionV1 {
    exact_facts: DecodedCanonicalExactTypeFactsV1,
    representation_support: DecodedCanonicalNominalRepresentationSupportV1,
    inheritance: DecodedCanonicalNominalInheritanceInterfacesV1,
    protected_declarations: DecodedCanonicalProtectedDeclarationInterfacesV1,
    protected_source_interfaces: DecodedCanonicalProtectedCallableSourceInterfacesV1,
    protected_defaults: DecodedCanonicalProtectedDefaultTemplatesV1,
    definition_sources: DecodedCanonicalExportDefinitionSourcesV1,
    selected: DecodedCanonicalSelectedExternalTypeUsesV1,
}
impl DecodedCrossConeTypeSemanticsSectionV1 {
    /// Restore typed records with one shared budget. This produces transport,
    /// which must pass complete source and committed-use checks before use.
    pub fn resolve<R: TypeSemanticsSectionResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<CrossConeTypeSemanticsSectionV1, TypeSemanticsSectionResolutionError<E>> {
        use TypeSemanticsSectionResolutionError as Error;
        meter
            .check_semantic_depth(1, path)
            .map_err(Error::Resource)?;
        meter.charge_nodes(1, path).map_err(Error::Resource)?;
        let exact_facts = self
            .exact_facts
            .resolve_metered(resolver, meter, &path.clone().field(1))
            .map_err(Error::Facts)?;
        let representation_support = self
            .representation_support
            .resolve_metered(resolver, meter, &path.clone().field(2))
            .map_err(|e| Error::Representation(Box::new(e)))?;
        let inheritance = self
            .inheritance
            .resolve(resolver, meter)
            .map_err(|e| Error::Inheritance(Box::new(e)))?;
        let protected_declarations = self
            .protected_declarations
            .resolve(resolver, meter)
            .map_err(|e| Error::Declarations(Box::new(e)))?;
        // Field 5 refers forward to the exact key projection of field 6.
        // The source protocol resolver checks both directions of this relation.
        let protected_defaults = self
            .protected_defaults
            .resolve(resolver, meter)
            .map_err(|e| Error::Defaults(Box::new(e)))?;
        let protected_source_interfaces = self
            .protected_source_interfaces
            .resolve(resolver, protected_defaults.keys(), meter)
            .map_err(|e| Error::Sources(Box::new(e)))?;
        let definition_sources = self
            .definition_sources
            .resolve_metered(resolver, meter, &path.clone().field(7))
            .map_err(|e| Error::Origins(Box::new(e)))?;
        let selected = self
            .selected
            .resolve(resolver, meter, &path.clone().field(8))
            .map_err(Error::Selected)?;
        Ok(CrossConeTypeSemanticsSectionV1::new(
            exact_facts,
            representation_support,
            inheritance,
            protected_declarations,
            protected_source_interfaces,
            protected_defaults,
            definition_sources,
            selected,
        ))
    }
}
impl WireDecode for DecodedCrossConeTypeSemanticsSectionV1 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        d.expect_map(8)?;
        Ok(Self {
            exact_facts: d.field(1, DecodedCanonicalExactTypeFactsV1::decode)?,
            representation_support: d
                .field(2, DecodedCanonicalNominalRepresentationSupportV1::decode)?,
            inheritance: d.field(3, DecodedCanonicalNominalInheritanceInterfacesV1::decode)?,
            protected_declarations: d
                .field(4, DecodedCanonicalProtectedDeclarationInterfacesV1::decode)?,
            protected_source_interfaces: d.field(
                5,
                DecodedCanonicalProtectedCallableSourceInterfacesV1::decode,
            )?,
            protected_defaults: d.field(6, DecodedCanonicalProtectedDefaultTemplatesV1::decode)?,
            definition_sources: d.field(7, DecodedCanonicalExportDefinitionSourcesV1::decode)?,
            selected: d.field(8, DecodedCanonicalSelectedExternalTypeUsesV1::decode)?,
        })
    }
}
impl WireEncode for DecodedCrossConeTypeSemanticsSectionV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(8)?;
        e.field(1)?;
        self.exact_facts.encode(e)?;
        e.field(2)?;
        self.representation_support.encode(e)?;
        e.field(3)?;
        self.inheritance.encode(e)?;
        e.field(4)?;
        self.protected_declarations.encode(e)?;
        e.field(5)?;
        self.protected_source_interfaces.encode(e)?;
        e.field(6)?;
        self.protected_defaults.encode(e)?;
        e.field(7)?;
        self.definition_sources.encode(e)?;
        e.field(8)?;
        self.selected.encode(e)
    }
}
