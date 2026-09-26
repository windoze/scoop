//! Complete representation, inheritance, and selected type-use metadata.
//! Source protocols and defaults are stored in the shared HIR interface.
use crate::*;

mod decode;
#[cfg(test)]
mod tests;
pub use decode::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeTypeSemanticsSectionV1 {
    exact_facts: CanonicalExactTypeFactsV1,
    representation_support: CanonicalNominalRepresentationSupportV1,
    inheritance: CanonicalNominalInheritanceInterfacesV1,
    selected: CanonicalSelectedExternalTypeUsesV1,
}
impl CrossConeTypeSemanticsSectionV1 {
    pub const fn new(
        exact_facts: CanonicalExactTypeFactsV1,
        representation_support: CanonicalNominalRepresentationSupportV1,
        inheritance: CanonicalNominalInheritanceInterfacesV1,
        selected: CanonicalSelectedExternalTypeUsesV1,
    ) -> Self {
        Self {
            exact_facts,
            representation_support,
            inheritance,
            selected,
        }
    }
    pub const fn exact_facts(&self) -> &CanonicalExactTypeFactsV1 {
        &self.exact_facts
    }
    pub const fn representation_support(&self) -> &CanonicalNominalRepresentationSupportV1 {
        &self.representation_support
    }
    pub const fn inheritance(&self) -> &CanonicalNominalInheritanceInterfacesV1 {
        &self.inheritance
    }
    pub const fn selected(&self) -> &CanonicalSelectedExternalTypeUsesV1 {
        &self.selected
    }
}

impl scoop_wire::WireEncode for CrossConeTypeSemanticsSectionV1 {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.exact_facts.encode(encoder)?;
        encoder.field(2)?;
        self.representation_support.encode(encoder)?;
        encoder.field(3)?;
        self.inheritance.encode(encoder)?;
        // Fields 4 through 7 are retired; declarations live in the shared interface.
        encoder.field(8)?;
        self.selected.encode(encoder)
    }
}
