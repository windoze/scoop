use super::*;
use scoop_identity::PersistentIdResolver;

use super::slot_wire::{DecodedEntry, DecodedSlot};

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedSlots {
    NoClassVtable,
    ClassVtable(Vec<DecodedEntry>),
    InterfaceSlots(Vec<DecodedSlot>),
}
macro_rules! encode_slots {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                match self {
                    Self::NoClassVtable => tag(encoder, 1, 1),
                    Self::ClassVtable(entries) => {
                        tag(encoder, 2, 2)?;
                        encoder.field(1)?;
                        sequence(encoder, entries)
                    }
                    Self::InterfaceSlots(slots) => {
                        tag(encoder, 2, 3)?;
                        encoder.field(1)?;
                        sequence(encoder, slots)
                    }
                }
            }
        }
    };
}
encode_slots!(MirDispatchSlotsV1);
encode_slots!(DecodedSlots);
impl WireDecode for DecodedSlots {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let count = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                fields(decoder, count, 1)?;
                Ok(Self::NoClassVtable)
            }
            2 => {
                fields(decoder, count, 2)?;
                Ok(Self::ClassVtable(decoder.field(1, |decoder| {
                    decoder.decode_array(|decoder, _| DecodedEntry::decode(decoder))
                })?))
            }
            3 => {
                fields(decoder, count, 2)?;
                Ok(Self::InterfaceSlots(decoder.field(1, |decoder| {
                    decoder.decode_array(|decoder, _| DecodedSlot::decode(decoder))
                })?))
            }
            tag => Err(error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedItable {
    interface: DecodedPersistentId<PersistentExactTypeId>,
    entries: Vec<DecodedEntry>,
}
macro_rules! encode_itable {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(2)?;
                encoder.field(1)?;
                self.interface.encode(encoder)?;
                encoder.field(2)?;
                sequence(encoder, &self.entries)
            }
        }
    };
}
encode_itable!(MirInterfaceDispatchTableV1);
encode_itable!(DecodedItable);
impl WireDecode for DecodedItable {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            interface: decoder.field(1, DecodedPersistentId::decode)?,
            entries: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedEntry::decode(decoder))
            })?,
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedParamFreeMirDispatchSchemaV1 {
    owner: DecodedPersistentId<PersistentExactTypeId>,
    slots: DecodedSlots,
    itables: Vec<DecodedItable>,
}
impl DecodedParamFreeMirDispatchSchemaV1 {
    pub fn validate(
        self,
        graph: &mut ValidatedIdentityGraph,
        types: &dyn MirTypeBridgeTypeLookupV1,
        callables: &dyn MirTypeBridgeCallableLookupV1,
    ) -> Result<ParamFreeMirDispatchSchemaV1, MirDispatchSchemaError> {
        let owner = graph.resolve(self.owner)?;
        let slots = match self.slots {
            DecodedSlots::NoClassVtable => MirDispatchSlotsV1::NoClassVtable,
            DecodedSlots::ClassVtable(entries) => {
                MirDispatchSlotsV1::ClassVtable(resolve_entries(entries, graph)?)
            }
            DecodedSlots::InterfaceSlots(slots) => {
                let mut resolved = reserve(slots.len())?;
                for slot in slots {
                    resolved.push(slot.resolve(graph)?);
                }
                MirDispatchSlotsV1::InterfaceSlots(resolved)
            }
        };
        let mut itables = reserve(self.itables.len())?;
        for itable in self.itables {
            itables.push(MirInterfaceDispatchTableV1::new(
                graph.resolve(itable.interface)?,
                resolve_entries(itable.entries, graph)?,
            ));
        }
        ParamFreeMirDispatchSchemaV1::try_new(
            MirDispatchSchemaAuthority {
                identities: graph,
                types,
                callables,
            },
            owner,
            slots,
            itables,
        )
    }
}
fn resolve_entries(
    entries: Vec<DecodedEntry>,
    graph: &mut ValidatedIdentityGraph,
) -> Result<Vec<MirDispatchEntryV1>, MirDispatchSchemaError> {
    let mut result = reserve(entries.len())?;
    for entry in entries {
        result.push(entry.resolve(graph)?);
    }
    Ok(result)
}
macro_rules! encode_schema {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(3)?;
                encoder.field(1)?;
                self.owner.encode(encoder)?;
                encoder.field(2)?;
                self.slots.encode(encoder)?;
                encoder.field(3)?;
                sequence(encoder, &self.itables)
            }
        }
    };
}
encode_schema!(ParamFreeMirDispatchSchemaV1);
encode_schema!(DecodedParamFreeMirDispatchSchemaV1);
impl WireDecode for DecodedParamFreeMirDispatchSchemaV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            owner: decoder.field(1, DecodedPersistentId::decode)?,
            slots: decoder.field(2, DecodedSlots::decode)?,
            itables: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| DecodedItable::decode(decoder))
            })?,
        })
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalMirDispatchSchemasV1 {
    records: Vec<DecodedParamFreeMirDispatchSchemaV1>,
}
impl DecodedCanonicalMirDispatchSchemasV1 {
    pub fn validate(
        self,
        graph: &mut ValidatedIdentityGraph,
        types: &dyn MirTypeBridgeTypeLookupV1,
        callables: &dyn MirTypeBridgeCallableLookupV1,
    ) -> Result<CanonicalMirDispatchSchemasV1, MirDispatchSchemaError> {
        self.validate_with_dependencies(graph, types, callables, &[])
    }
    pub fn validate_with_dependencies(
        self,
        graph: &mut ValidatedIdentityGraph,
        types: &dyn MirTypeBridgeTypeLookupV1,
        callables: &dyn MirTypeBridgeCallableLookupV1,
        dependencies: &[&CanonicalMirDispatchSchemasV1],
    ) -> Result<CanonicalMirDispatchSchemasV1, MirDispatchSchemaError> {
        let mut records: Vec<ParamFreeMirDispatchSchemaV1> = reserve(self.records.len())?;
        for (index, record) in self.records.into_iter().enumerate() {
            let record = record.validate(graph, types, callables)?;
            if records
                .last()
                .is_some_and(|previous| previous.owner() >= record.owner())
            {
                return Err(MirDispatchSchemaError::NonCanonicalOwnerOrder { index });
            }
            records.push(record);
        }
        let table = CanonicalMirDispatchSchemasV1 { records };
        MirDispatchSchemaAuthority {
            identities: graph,
            types,
            callables,
        }
        .validate_with_dependencies(&table, dependencies)?;
        Ok(table)
    }
}
impl WireDecode for DecodedCanonicalMirDispatchSchemasV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedParamFreeMirDispatchSchemaV1::decode(decoder))
            .map(|records| Self { records })
    }
}
impl WireEncode for DecodedCanonicalMirDispatchSchemasV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, &self.records)
    }
}
impl WireEncode for CanonicalMirDispatchSchemasV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        sequence(encoder, &self.records)
    }
}
