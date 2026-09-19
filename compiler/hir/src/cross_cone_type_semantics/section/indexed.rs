use super::*;
use scoop_wire::{BudgetMeter, Encoder, WireEncode};

/// Publisher-side local indices. This view carries no source or use authority.
pub struct IndexedCrossConeTypeSemanticsSectionV1<'a> {
    section: &'a CrossConeTypeSemanticsSectionV1,
    sources: IndexedCanonicalProtectedCallableSourceInterfacesV1<'a>,
    defaults: IndexedCanonicalProtectedDefaultTemplatesV1<'a>,
}
impl CrossConeTypeSemanticsSectionV1 {
    pub fn index_for_wire(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<IndexedCrossConeTypeSemanticsSectionV1<'_>, TypeSemanticsSectionIndexError> {
        let sources = self
            .protected_source_interfaces
            .index_templates(self.protected_defaults.keys(), meter)
            .map_err(TypeSemanticsSectionIndexError::Source)?;
        let defaults = self
            .protected_defaults
            .index_locals()
            .map_err(TypeSemanticsSectionIndexError::Default)?;
        Ok(IndexedCrossConeTypeSemanticsSectionV1 {
            section: self,
            sources,
            defaults,
        })
    }
}
impl WireEncode for IndexedCrossConeTypeSemanticsSectionV1<'_> {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(8)?;
        e.field(1)?;
        self.section.exact_facts.encode(e)?;
        e.field(2)?;
        self.section.representation_support.encode(e)?;
        e.field(3)?;
        self.section.inheritance.encode(e)?;
        e.field(4)?;
        self.section.protected_declarations.encode(e)?;
        e.field(5)?;
        self.sources.encode(e)?;
        e.field(6)?;
        self.defaults.encode(e)?;
        e.field(7)?;
        self.section.definition_sources.encode(e)?;
        e.field(8)?;
        self.section.selected.encode(e)
    }
}

#[derive(Debug)]
pub enum TypeSemanticsSectionIndexError {
    Source(ProtectedSourceIndexError),
    Default(ProtectedDefaultTemplateTableIndexError),
}
impl std::fmt::Display for TypeSemanticsSectionIndexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => write!(f, "type section field 5: {error}"),
            Self::Default(error) => write!(f, "type section field 6: {error}"),
        }
    }
}
impl std::error::Error for TypeSemanticsSectionIndexError {}
