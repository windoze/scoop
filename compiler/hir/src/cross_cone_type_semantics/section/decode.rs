use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

mod errors;
pub use errors::*;

pub trait TypeSemanticsSectionResolver<E>:
    NominalRepresentationResolver<E>
    + NominalInheritanceInterfaceResolver<E>
    + SelectedTypeUseResolver<E>
{
}
impl<R, E> TypeSemanticsSectionResolver<E> for R where
    R: NominalRepresentationResolver<E>
        + NominalInheritanceInterfaceResolver<E>
        + SelectedTypeUseResolver<E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCrossConeTypeSemanticsSectionV1 {
    exact_facts: DecodedCanonicalExactTypeFactsV1,
    representation_support: DecodedCanonicalNominalRepresentationSupportV1,
    inheritance: DecodedCanonicalNominalInheritanceInterfacesV1,
    selected: DecodedCanonicalSelectedExternalTypeUsesV1,
}
impl DecodedCrossConeTypeSemanticsSectionV1 {
    /// Resolves typed references at the artifact decoding boundary.
    pub fn resolve<R: TypeSemanticsSectionResolver<E>, E>(
        self,
        resolver: &mut R,

        path: &WirePath,
    ) -> Result<CrossConeTypeSemanticsSectionV1, TypeSemanticsSectionResolutionError<E>> {
        use TypeSemanticsSectionResolutionError as Error;

        let exact_facts = self.exact_facts.resolve(resolver).map_err(Error::Facts)?;
        let representation_support = self
            .representation_support
            .resolve(resolver)
            .map_err(|e| Error::Representation(Box::new(e)))?;
        let inheritance = self
            .inheritance
            .resolve(resolver)
            .map_err(|e| Error::Inheritance(Box::new(e)))?;
        let selected = self
            .selected
            .resolve(resolver, &path.clone().field(8))
            .map_err(Error::Selected)?;
        Ok(CrossConeTypeSemanticsSectionV1::new(
            exact_facts,
            representation_support,
            inheritance,
            selected,
        ))
    }
}
impl WireDecode for DecodedCrossConeTypeSemanticsSectionV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(4)?;
        Ok(Self {
            exact_facts: d.field(1, DecodedCanonicalExactTypeFactsV1::decode)?,
            representation_support: d
                .field(2, DecodedCanonicalNominalRepresentationSupportV1::decode)?,
            inheritance: d.field(3, DecodedCanonicalNominalInheritanceInterfacesV1::decode)?,
            selected: d.field(8, DecodedCanonicalSelectedExternalTypeUsesV1::decode)?,
        })
    }
}
impl WireEncode for DecodedCrossConeTypeSemanticsSectionV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(4)?;
        e.field(1)?;
        self.exact_facts.encode(e)?;
        e.field(2)?;
        self.representation_support.encode(e)?;
        e.field(3)?;
        self.inheritance.encode(e)?;
        e.field(8)?;
        self.selected.encode(e)
    }
}
