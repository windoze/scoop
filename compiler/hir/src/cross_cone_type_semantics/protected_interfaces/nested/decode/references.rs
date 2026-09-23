use super::*;
use crate::{DecodedSourceNominalId, SourceNominalId};
use scoop_identity::{
    DecodedPersistentId, PersistentFunctionId, PersistentGenericFunctionId, PersistentPropertyId,
};
use scoop_wire::WireErrorKind;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedNestedSourceMemberRefV1 {
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
    Property(DecodedPersistentId<PersistentPropertyId>),
}
impl DecodedNestedSourceMemberRefV1 {
    pub fn resolve<R, E>(self, resolver: &mut R) -> Result<NestedSourceMemberRefV1, E>
    where
        R: PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentPropertyId, Error = E>,
    {
        Ok(match self {
            Self::Function(id) => NestedSourceMemberRefV1::Function(resolver.resolve(id)?),
            Self::GenericFunction(id) => {
                NestedSourceMemberRefV1::GenericFunction(resolver.resolve(id)?)
            }
            Self::Property(id) => NestedSourceMemberRefV1::Property(resolver.resolve(id)?),
        })
    }
}
impl WireEncode for DecodedNestedSourceMemberRefV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Function(id) => {
                wire::tag(encoder, 2, 1)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
            Self::GenericFunction(id) => {
                wire::tag(encoder, 2, 2)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
            Self::Property(id) => {
                wire::tag(encoder, 2, 3)?;
                encoder.field(1)?;
                id.encode(encoder)
            }
        }
    }
}
impl WireDecode for DecodedNestedSourceMemberRefV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Function),
            2 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::GenericFunction),
            3 => decoder
                .field(1, DecodedPersistentId::decode)
                .map(Self::Property),
            tag => Err(wire::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

pub(super) fn resolve_members<R: NestedSourceInterfaceResolver<E>, E>(
    values: Vec<DecodedNestedSourceMemberRefV1>,
    resolver: &mut R,
    meter: &mut BudgetMeter,
) -> Result<CanonicalNestedMemberRefsV1, NestedSourceResolutionError<E>> {
    let mut members = Vec::new();
    reserve(&mut members, values.len(), meter)?;
    for value in values {
        members.push(
            value
                .resolve(resolver)
                .map_err(NestedSourceResolutionError::Foundation)?,
        );
    }
    CanonicalNestedMemberRefsV1::from_ordered(members).map_err(NestedSourceResolutionError::Build)
}
pub(super) fn resolve_children<R: NestedSourceInterfaceResolver<E>, E>(
    values: Vec<DecodedSourceNominalId>,
    resolver: &mut R,
    meter: &mut BudgetMeter,
) -> Result<CanonicalNestedNominalRefsV1, NestedSourceResolutionError<E>> {
    let mut children: Vec<SourceNominalId> = Vec::new();
    reserve(&mut children, values.len(), meter)?;
    for value in values {
        children.push(
            value
                .resolve(resolver)
                .map_err(NestedSourceResolutionError::Foundation)?,
        );
    }
    CanonicalNestedNominalRefsV1::from_ordered(children).map_err(NestedSourceResolutionError::Build)
}
