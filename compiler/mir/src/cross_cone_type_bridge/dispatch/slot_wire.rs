use super::*;
use scoop_identity::PersistentIdResolver;

use super::implementation_wire::DecodedImplementation;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedSlot {
    slot: DecodedPersistentId<PersistentDispatchSlotId>,
    position: MirDispatchPositionV1,
    signature: DecodedMirBridgeCallableSignatureV1,
}
impl DecodedSlot {
    pub(super) fn resolve(
        self,
        graph: &mut ValidatedIdentityGraph,
    ) -> Result<MirDispatchSlotV1, MirDispatchSchemaError> {
        Ok(MirDispatchSlotV1::new(
            graph.resolve(self.slot)?,
            self.position,
            self.signature
                .resolve(graph)
                .map_err(|error| MirDispatchSchemaError::Signature(Box::new(error)))?,
        ))
    }
}

macro_rules! encode_slot {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(3)?;
                encoder.field(1)?;
                self.slot.encode(encoder)?;
                encoder.field(2)?;
                encoder.unsigned(u64::from(self.position.get()))?;
                encoder.field(3)?;
                self.signature.encode(encoder)
            }
        }
    };
}
encode_slot!(MirDispatchSlotV1);
encode_slot!(DecodedSlot);

impl WireDecode for DecodedSlot {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            slot: decoder.field(1, DecodedPersistentId::decode)?,
            position: MirDispatchPositionV1::new(decoder.field(2, Decoder::u32)?),
            signature: decoder.field(3, DecodedMirBridgeCallableSignatureV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedEntry {
    contract: DecodedSlot,
    implementation: DecodedImplementation,
}
impl DecodedEntry {
    pub(super) fn resolve(
        self,
        graph: &mut ValidatedIdentityGraph,
    ) -> Result<MirDispatchEntryV1, MirDispatchSchemaError> {
        Ok(MirDispatchEntryV1 {
            contract: self.contract.resolve(graph)?,
            implementation: self.implementation.resolve(graph)?,
        })
    }
}

macro_rules! encode_entry {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(4)?;
                encoder.field(1)?;
                self.contract.slot.encode(encoder)?;
                encoder.field(2)?;
                encoder.unsigned(u64::from(self.contract.position.get()))?;
                encoder.field(3)?;
                self.contract.signature.encode(encoder)?;
                encoder.field(4)?;
                self.implementation.encode(encoder)
            }
        }
    };
}
encode_entry!(MirDispatchEntryV1);
encode_entry!(DecodedEntry);

impl WireDecode for DecodedEntry {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            contract: DecodedSlot {
                slot: decoder.field(1, DecodedPersistentId::decode)?,
                position: MirDispatchPositionV1::new(decoder.field(2, Decoder::u32)?),
                signature: decoder.field(3, DecodedMirBridgeCallableSignatureV1::decode)?,
            },
            implementation: decoder.field(4, DecodedImplementation::decode)?,
        })
    }
}
