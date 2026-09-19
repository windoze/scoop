use super::*;
use crate::{
    DecodedCanonicalProtectedSlotRefsV1, DecodedDeclarationAccessSourceV1, DecodedSourceNominalId,
    SignatureTypeReferenceResolver,
};
use scoop_identity::{
    ConeIdentity, DecodedPersistentId, DecodedSignatureTypeKey, PersistentIdResolver,
    PersistentKeyResolver, PersistentSourceContextId, SourceContextKey,
};
use scoop_wire::{BudgetMeter, Decoder, WireDecode, WireError, WirePath};

mod mutability;
pub use mutability::DecodedProtectedPropertyMutabilityV1;

pub trait ProtectedPropertyInterfaceResolver<E>:
    SignatureTypeReferenceResolver<E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
    + PersistentIdResolver<PersistentPropertyId, Error = E>
    + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
    + PersistentIdResolver<scoop_identity::PersistentDispatchSlotId, Error = E>
{
}
impl<R, E> ProtectedPropertyInterfaceResolver<E> for R where
    R: SignatureTypeReferenceResolver<E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
        + PersistentIdResolver<PersistentPropertyId, Error = E>
        + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
        + PersistentIdResolver<scoop_identity::PersistentDispatchSlotId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedPropertyPayloadV1 {
    owner: DecodedSourceNominalId,
    value_type: DecodedSignatureTypeKey,
    pub(super) getter: DecodedPersistentId<PersistentPropertyAccessorId>,
    pub(super) mutability: DecodedProtectedPropertyMutabilityV1,
    pub(super) representation: PropertyRepresentationV1,
    slot_relations: DecodedCanonicalProtectedSlotRefsV1,
}
impl DecodedProtectedPropertyPayloadV1 {
    pub fn resolve<R: ProtectedPropertyInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<ProtectedPropertyPayloadV1, ProtectedPropertyResolutionError<E>> {
        use ProtectedPropertyResolutionError as Error;
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(Error::Resource)?;
        let owner = self.owner.resolve(resolver).map_err(Error::Identity)?;
        self.value_type
            .charge_resolution(meter)
            .map_err(Error::Resource)?;
        let value_type = self.value_type.resolve(resolver).map_err(Error::Identity)?;
        let getter = resolver.resolve(self.getter).map_err(Error::Identity)?;
        let mutability = self.mutability.resolve(resolver, meter)?;
        let slots = self
            .slot_relations
            .resolve(resolver, meter)
            .map_err(Error::Slots)?;
        ProtectedPropertyPayloadV1::try_new(
            owner,
            value_type,
            getter,
            mutability,
            self.representation,
            slots,
        )
        .map_err(Error::Property)
    }
}
impl WireEncode for DecodedProtectedPropertyPayloadV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.getter.encode(encoder)?;
        encoder.field(4)?;
        self.mutability.encode(encoder)?;
        encoder.field(5)?;
        self.representation.encode(encoder)?;
        encoder.field(6)?;
        self.slot_relations.encode(encoder)
    }
}
impl WireDecode for DecodedProtectedPropertyPayloadV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            owner: decoder.field(1, DecodedSourceNominalId::decode)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            getter: decoder.field(3, DecodedPersistentId::decode)?,
            mutability: decoder.field(4, DecodedProtectedPropertyMutabilityV1::decode)?,
            representation: decoder.field(5, PropertyRepresentationV1::decode)?,
            slot_relations: decoder.field(6, DecodedCanonicalProtectedSlotRefsV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedProtectedPropertyInterfaceV1 {
    declaration: DecodedPersistentId<PersistentPropertyId>,
    declaration_access: DecodedDeclarationAccessSourceV1,
    pub(super) payload: DecodedProtectedPropertyPayloadV1,
}
impl DecodedProtectedPropertyInterfaceV1 {
    pub fn resolve<R: ProtectedPropertyInterfaceResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<ProtectedPropertyInterfaceV1, ProtectedPropertyResolutionError<E>> {
        use ProtectedPropertyResolutionError as Error;
        meter
            .charge_nodes(1, &WirePath::root())
            .map_err(Error::Resource)?;
        let declaration = resolver
            .resolve(self.declaration)
            .map_err(Error::Identity)?;
        let access = self
            .declaration_access
            .resolve_metered(resolver, meter)
            .map_err(Error::Access)?;
        let payload = self.payload.resolve(resolver, meter)?;
        ProtectedPropertyInterfaceV1::try_new(declaration, access, payload).map_err(Error::Property)
    }
}
impl WireEncode for DecodedProtectedPropertyInterfaceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.declaration.encode(encoder)?;
        encoder.field(2)?;
        self.declaration_access.encode(encoder)?;
        encoder.field(3)?;
        self.payload.encode(encoder)
    }
}
impl WireDecode for DecodedProtectedPropertyInterfaceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            declaration: decoder.field(1, DecodedPersistentId::decode)?,
            declaration_access: decoder.field(2, DecodedDeclarationAccessSourceV1::decode)?,
            payload: decoder.field(3, DecodedProtectedPropertyPayloadV1::decode)?,
        })
    }
}
