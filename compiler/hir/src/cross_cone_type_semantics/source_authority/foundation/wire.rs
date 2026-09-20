use scoop_identity::{DecodedPersistentId, PersistentEnumVariantId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode};

use super::*;

pub trait TypeFoundationSourceResolver<E>:
    NominalRepresentationResolver<E>
    + SourceNominalIdResolver<E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
    + PersistentIdResolver<PersistentEnumVariantId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
{
}

impl<R, E> TypeFoundationSourceResolver<E> for R where
    R: NominalRepresentationResolver<E>
        + SourceNominalIdResolver<E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
        + PersistentIdResolver<PersistentEnumVariantId, Error = E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
{
}

impl WireEncode for TypeFoundationSourceAuthorityV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let value = self.entries();
        e.map(13)?;
        e.field(1)?;
        value.provider.encode(e)?;
        e.field(2)?;
        value.exact_keys.encode(e)?;
        e.field(3)?;
        value.sources.encode(e)?;
        e.field(4)?;
        value.representations.encode(e)?;
        e.field(5)?;
        value.generated_nominals.encode(e)?;
        e.field(6)?;
        value.accessor_keys.encode(e)?;
        e.field(7)?;
        value.definition_sources.encode(e)?;
        e.field(8)?;
        value.source_roots.encode(e)?;
        e.field(9)?;
        value.local_exact_facts.encode(e)?;
        e.field(10)?;
        value.dependency_facts.encode(e)?;
        e.field(11)?;
        value.local_inheritance_edges.encode(e)?;
        e.field(12)?;
        value.fact_shapes.encode(e)?;
        e.field(13)?;
        value.representation_owners.encode(e)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTypeFoundationSourceAuthorityV1 {
    provider: DecodedPersistentId<ConeIdentity>,
    exact_keys: DecodedCanonicalPersistentIdsV1<PersistentExactTypeId>,
    sources: DecodedCanonicalTypeSourceNominalsV1,
    representations: DecodedCanonicalNominalRepresentationSupportV1,
    generated_nominals: DecodedCanonicalPersistentIdsV1<PersistentTypeId>,
    accessor_keys: DecodedCanonicalPersistentIdsV1<PersistentPropertyAccessorId>,
    definition_sources: DecodedCanonicalExportDefinitionSourcesV1,
    source_roots: DecodedCanonicalSourceNominalIdsV1,
    local_exact_facts: DecodedCanonicalPersistentIdsV1<PersistentExactTypeId>,
    dependency_facts: DecodedCanonicalTypeSectionDependencyFactsV1,
    local_inheritance_edges: DecodedCanonicalNominalInheritanceEdgesV1,
    fact_shapes: DecodedCanonicalExactTypeFactShapesV1,
    representation_owners: DecodedCanonicalPersistentIdsV1<PersistentTypeId>,
}

mod resolve;

impl WireDecode for DecodedTypeFoundationSourceAuthorityV1 {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        d.expect_map(13)?;
        Ok(Self {
            provider: d.field(1, DecodedPersistentId::decode)?,
            exact_keys: d.field(2, DecodedCanonicalPersistentIdsV1::decode)?,
            sources: d.field(3, DecodedCanonicalTypeSourceNominalsV1::decode)?,
            representations: d.field(4, DecodedCanonicalNominalRepresentationSupportV1::decode)?,
            generated_nominals: d.field(5, DecodedCanonicalPersistentIdsV1::decode)?,
            accessor_keys: d.field(6, DecodedCanonicalPersistentIdsV1::decode)?,
            definition_sources: d.field(7, DecodedCanonicalExportDefinitionSourcesV1::decode)?,
            source_roots: d.field(8, DecodedCanonicalSourceNominalIdsV1::decode)?,
            local_exact_facts: d.field(9, DecodedCanonicalPersistentIdsV1::decode)?,
            dependency_facts: d.field(10, DecodedCanonicalTypeSectionDependencyFactsV1::decode)?,
            local_inheritance_edges: d
                .field(11, DecodedCanonicalNominalInheritanceEdgesV1::decode)?,
            fact_shapes: d.field(12, DecodedCanonicalExactTypeFactShapesV1::decode)?,
            representation_owners: d.field(13, DecodedCanonicalPersistentIdsV1::decode)?,
        })
    }
}

impl WireEncode for DecodedTypeFoundationSourceAuthorityV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(13)?;
        e.field(1)?;
        self.provider.encode(e)?;
        e.field(2)?;
        self.exact_keys.encode(e)?;
        e.field(3)?;
        self.sources.encode(e)?;
        e.field(4)?;
        self.representations.encode(e)?;
        e.field(5)?;
        self.generated_nominals.encode(e)?;
        e.field(6)?;
        self.accessor_keys.encode(e)?;
        e.field(7)?;
        self.definition_sources.encode(e)?;
        e.field(8)?;
        self.source_roots.encode(e)?;
        e.field(9)?;
        self.local_exact_facts.encode(e)?;
        e.field(10)?;
        self.dependency_facts.encode(e)?;
        e.field(11)?;
        self.local_inheritance_edges.encode(e)?;
        e.field(12)?;
        self.fact_shapes.encode(e)?;
        e.field(13)?;
        self.representation_owners.encode(e)
    }
}
