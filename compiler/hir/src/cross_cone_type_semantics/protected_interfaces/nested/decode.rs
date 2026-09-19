use super::*;
use crate::{
    DecodedCanonicalBinderListV1, DecodedCanonicalSignatureTypesV1, DecodedNominalSourceShapeV1,
    DecodedSourceNominalId, NominalSourceShapeResolver, ProtectedPropertyInterfaceResolver,
};
use scoop_identity::{DecodedPersistentId, PersistentExactTypeId, PersistentIdResolver};
use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

mod errors;
mod payload;
mod references;
mod support;
pub use errors::*;
pub use payload::*;
pub use references::DecodedNestedSourceMemberRefV1;
pub use support::DecodedNestedSourceSupportV1;

pub trait NestedSourceInterfaceResolver<E>:
    ProtectedPropertyInterfaceResolver<E>
    + crate::ProtectedCallableInterfaceResolver<E>
    + NominalSourceShapeResolver<E>
    + PersistentIdResolver<PersistentExactTypeId, Error = E>
{
}
impl<R, E> NestedSourceInterfaceResolver<E> for R where
    R: ProtectedPropertyInterfaceResolver<E>
        + crate::ProtectedCallableInterfaceResolver<E>
        + NominalSourceShapeResolver<E>
        + PersistentIdResolver<PersistentExactTypeId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedNestedSourceInterfaceV1 {
    kind: PublicNominalKindV1,
    modality: NominalInheritanceModalityV1,
    type_parameters: DecodedCanonicalBinderListV1,
    supertypes: DecodedCanonicalSignatureTypesV1,
    constructors: Vec<DecodedPersistentId<PersistentConstructorId>>,
    members: Vec<DecodedNestedSourceMemberRefV1>,
    children: Vec<DecodedSourceNominalId>,
    source_shape: DecodedNominalSourceShapeV1,
    source_support: Vec<DecodedNestedSourceSupportV1>,
}
impl DecodedProtectedNestedSourceInterfaceV1 {
    pub fn resolve<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<ProtectedNestedSourceInterfaceV1, NestedSourceResolutionError<E>> {
        self.resolve_at(resolver, meter, 1)
    }
    fn resolve_at<R: NestedSourceInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        depth: u64,
    ) -> Result<ProtectedNestedSourceInterfaceV1, NestedSourceResolutionError<E>> {
        use NestedSourceResolutionError as Error;
        meter
            .check_semantic_depth(depth, &WirePath::root())
            .map_err(Error::Resource)?;
        let binders = self
            .type_parameters
            .resolve_metered(resolver, meter)
            .map_err(Error::Binders)?;
        let supertypes = self
            .supertypes
            .resolve_metered(resolver, meter)
            .map_err(Error::Supertypes)?;
        let mut constructors = Vec::new();
        reserve(&mut constructors, self.constructors.len(), meter)?;
        for value in self.constructors {
            let value = resolver.resolve(value).map_err(Error::Foundation)?;
            if constructors
                .last()
                .is_some_and(|previous| *previous >= value)
            {
                return Err(Error::Build(NestedSourceBuildError::NonCanonicalOrder));
            }
            constructors.push(value);
        }
        let constructors = CanonicalPersistentIdsV1::try_new(constructors)
            .map_err(|_| Error::Build(NestedSourceBuildError::Duplicate))?;
        let members = references::resolve_members(self.members, resolver, meter)?;
        let children = references::resolve_children(self.children, resolver, meter)?;
        let source_shape = self
            .source_shape
            .resolve_metered(resolver, meter)
            .map_err(Error::Shape)?;
        let mut records = Vec::new();
        reserve(&mut records, self.source_support.len(), meter)?;
        for record in self.source_support {
            records.push(record.resolve_at(resolver, meter, depth + 1)?);
        }
        let source_support =
            CanonicalNestedSourceSupportV1::from_ordered(records).map_err(Error::Build)?;
        ProtectedNestedSourceInterfaceV1::try_new(
            self.kind,
            self.modality,
            binders,
            supertypes,
            constructors,
            members,
            children,
            source_shape,
            source_support,
        )
        .map_err(Error::Build)
    }
}
impl WireDecode for DecodedProtectedNestedSourceInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(9)?;
        Ok(Self {
            kind: decoder.field(1, PublicNominalKindV1::decode)?,
            modality: decoder.field(2, NominalInheritanceModalityV1::decode)?,
            type_parameters: decoder.field(3, DecodedCanonicalBinderListV1::decode)?,
            supertypes: decoder.field(4, DecodedCanonicalSignatureTypesV1::decode)?,
            constructors: decoder
                .field(5, |d| d.decode_array(|d, _| DecodedPersistentId::decode(d)))?,
            members: decoder.field(6, |d| {
                d.decode_array(|d, _| DecodedNestedSourceMemberRefV1::decode(d))
            })?,
            children: decoder.field(7, |d| {
                d.decode_array(|d, _| DecodedSourceNominalId::decode(d))
            })?,
            source_shape: decoder.field(8, DecodedNominalSourceShapeV1::decode)?,
            source_support: decoder.field(9, |d| {
                d.decode_array(|d, _| DecodedNestedSourceSupportV1::decode(d))
            })?,
        })
    }
}
impl WireEncode for DecodedProtectedNestedSourceInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(9)?;
        encoder.field(1)?;
        self.kind.encode(encoder)?;
        encoder.field(2)?;
        self.modality.encode(encoder)?;
        encoder.field(3)?;
        self.type_parameters.encode(encoder)?;
        encoder.field(4)?;
        self.supertypes.encode(encoder)?;
        encoder.field(5)?;
        wire::sequence(encoder, &self.constructors)?;
        encoder.field(6)?;
        wire::sequence(encoder, &self.members)?;
        encoder.field(7)?;
        wire::sequence(encoder, &self.children)?;
        encoder.field(8)?;
        self.source_shape.encode(encoder)?;
        encoder.field(9)?;
        wire::sequence(encoder, &self.source_support)
    }
}
fn reserve<T, E>(
    values: &mut Vec<T>,
    count: usize,
    meter: &mut BudgetMeter,
) -> Result<(), NestedSourceResolutionError<E>> {
    let path = WirePath::root();
    meter
        .charge_nodes(count as u64, &path)
        .map_err(NestedSourceResolutionError::Resource)?;
    meter
        .charge_work((count as u64).saturating_mul(128), &path)
        .map_err(NestedSourceResolutionError::Resource)?;
    meter
        .try_reserve_collection_slots(values, count, &path)
        .map_err(NestedSourceResolutionError::Resource)
}
