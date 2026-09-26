//! Complete representation, inheritance, and selected type-use metadata.
//! Source protocols and defaults are stored in the shared HIR interface.
use crate::*;

mod decode;
mod source;
#[cfg(test)]
mod tests;
pub use decode::*;
pub use source::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeTypeSemanticsSectionV1 {
    exact_facts: CanonicalExactTypeFactsV1,
    representation_support: CanonicalNominalRepresentationSupportV1,
    inheritance: CanonicalNominalInheritanceInterfacesV1,
    protected_declarations: CanonicalProtectedDeclarationInterfacesV1,
    selected: CanonicalSelectedExternalTypeUsesV1,
}
impl CrossConeTypeSemanticsSectionV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        exact_facts: CanonicalExactTypeFactsV1,
        representation_support: CanonicalNominalRepresentationSupportV1,
        inheritance: CanonicalNominalInheritanceInterfacesV1,
        protected_declarations: CanonicalProtectedDeclarationInterfacesV1,
        selected: CanonicalSelectedExternalTypeUsesV1,
    ) -> Self {
        Self {
            exact_facts,
            representation_support,
            inheritance,
            protected_declarations,
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
    pub const fn protected_declarations(&self) -> &CanonicalProtectedDeclarationInterfacesV1 {
        &self.protected_declarations
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
        encoder.map(5)?;
        encoder.field(1)?;
        self.exact_facts.encode(encoder)?;
        encoder.field(2)?;
        self.representation_support.encode(encoder)?;
        encoder.field(3)?;
        self.inheritance.encode(encoder)?;
        encoder.field(4)?;
        self.protected_declarations.encode(encoder)?;
        encoder.field(8)?;
        self.selected.encode(encoder)
    }
}
