use super::{
    ProtectedCallableInterfaceBuildError, ProtectedCallableInterfaceResolutionError, wire,
};
use scoop_identity::{DecodedPersistentId, PersistentDispatchSlotId, PersistentIdResolver};
use scoop_wire::{BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalProtectedSlotRefsV1 {
    slots: Vec<PersistentDispatchSlotId>,
}
impl CanonicalProtectedSlotRefsV1 {
    pub fn try_new(
        mut slots: Vec<PersistentDispatchSlotId>,
    ) -> Result<Self, ProtectedCallableInterfaceBuildError> {
        slots.sort_unstable();
        Self::from_ordered(slots)
    }
    fn from_ordered(
        slots: Vec<PersistentDispatchSlotId>,
    ) -> Result<Self, ProtectedCallableInterfaceBuildError> {
        if let Some(index) = slots.windows(2).position(|pair| pair[0] >= pair[1]) {
            return Err(ProtectedCallableInterfaceBuildError::SlotOrder { index: index + 1 });
        }
        Ok(Self { slots })
    }
    pub fn slots(&self) -> &[PersistentDispatchSlotId] {
        &self.slots
    }
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}
impl WireEncode for CanonicalProtectedSlotRefsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.slots)
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalProtectedSlotRefsV1 {
    slots: Vec<DecodedPersistentId<PersistentDispatchSlotId>>,
}
impl DecodedCanonicalProtectedSlotRefsV1 {
    pub fn resolve<R: PersistentIdResolver<PersistentDispatchSlotId, Error = E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalProtectedSlotRefsV1, ProtectedCallableInterfaceResolutionError<E>> {
        let mut slots = Vec::new();
        meter
            .try_reserve_collection_slots(&mut slots, self.slots.len(), &WirePath::root())
            .map_err(ProtectedCallableInterfaceResolutionError::Resource)?;
        for slot in self.slots {
            slots.push(
                resolver
                    .resolve(slot)
                    .map_err(ProtectedCallableInterfaceResolutionError::Identity)?,
            );
        }
        CanonicalProtectedSlotRefsV1::from_ordered(slots)
            .map_err(ProtectedCallableInterfaceResolutionError::Interface)
    }
}
impl WireEncode for DecodedCanonicalProtectedSlotRefsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        wire::sequence(encoder, &self.slots)
    }
}
impl WireDecode for DecodedCanonicalProtectedSlotRefsV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
            .map(|slots| Self { slots })
    }
}
