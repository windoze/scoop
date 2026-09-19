use super::*;
use scoop_identity::PersistentIdResolver;

use super::implementation_wire::DecodedImplementation;

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedEntry {
    slot: DecodedPersistentId<PersistentDispatchSlotId>,
    position: MirDispatchPositionV1,
    signature: DecodedMirBridgeCallableSignatureV1,
    implementation: DecodedImplementation,
}
impl DecodedEntry {
    fn resolve(
        self,
        graph: &mut ValidatedIdentityGraph,
        meter: &mut BudgetMeter,
    ) -> Result<MirDispatchEntryV1, MirDispatchSchemaError> {
        Ok(MirDispatchEntryV1::new(
            graph.resolve(self.slot)?,
            self.position,
            self.signature
                .resolve(graph, meter)
                .map_err(|error| MirDispatchSchemaError::Signature(Box::new(error)))?,
            self.implementation.resolve(graph)?,
        ))
    }
}
macro_rules! encode_entry {
    ($ty:ty) => {
        impl WireEncode for $ty {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(4)?;
                encoder.field(1)?;
                self.slot.encode(encoder)?;
                encoder.field(2)?;
                encoder.unsigned(u64::from(self.position.get()))?;
                encoder.field(3)?;
                self.signature.encode(encoder)?;
                encoder.field(4)?;
                self.implementation.encode(encoder)
            }
        }
    };
}
encode_entry!(MirDispatchEntryV1);
encode_entry!(DecodedEntry);
impl WireDecode for DecodedEntry {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            slot: decoder.field(1, DecodedPersistentId::decode)?,
            position: MirDispatchPositionV1::new(decoder.field(2, Decoder::u32)?),
            signature: decoder.field(3, DecodedMirBridgeCallableSignatureV1::decode)?,
            implementation: decoder.field(4, DecodedImplementation::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedVtable {
    NoClassVtable,
    ClassVtable(Vec<DecodedEntry>),
}
macro_rules! encode_vtable {
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
                }
            }
        }
    };
}
encode_vtable!(MirClassVtableSchemaV1);
encode_vtable!(DecodedVtable);
impl WireDecode for DecodedVtable {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
    vtable: DecodedVtable,
    itables: Vec<DecodedItable>,
}
impl DecodedParamFreeMirDispatchSchemaV1 {
    pub fn validate(
        self,
        graph: &mut ValidatedIdentityGraph,
        types: &dyn MirTypeBridgeTypeLookupV1,
        callables: &dyn MirTypeBridgeCallableLookupV1,
        meter: &mut BudgetMeter,
    ) -> Result<ParamFreeMirDispatchSchemaV1, MirDispatchSchemaError> {
        let owner = graph.resolve(self.owner)?;
        let vtable = match self.vtable {
            DecodedVtable::NoClassVtable => MirClassVtableSchemaV1::NoClassVtable,
            DecodedVtable::ClassVtable(entries) => {
                MirClassVtableSchemaV1::ClassVtable(resolve_entries(entries, graph, meter)?)
            }
        };
        let mut itables = reserve(self.itables.len(), meter)?;
        for itable in self.itables {
            itables.push(MirInterfaceDispatchTableV1::new(
                graph.resolve(itable.interface)?,
                resolve_entries(itable.entries, graph, meter)?,
            ));
        }
        ParamFreeMirDispatchSchemaV1::try_new(
            MirDispatchSchemaAuthority {
                identities: graph,
                types,
                callables,
            },
            owner,
            vtable,
            itables,
            meter,
        )
    }
}
fn resolve_entries(
    entries: Vec<DecodedEntry>,
    graph: &mut ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<Vec<MirDispatchEntryV1>, MirDispatchSchemaError> {
    let mut result = reserve(entries.len(), meter)?;
    for entry in entries {
        meter.charge_nodes(1, &WirePath::root())?;
        result.push(entry.resolve(graph, meter)?);
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
                self.vtable.encode(encoder)?;
                encoder.field(3)?;
                sequence(encoder, &self.itables)
            }
        }
    };
}
encode_schema!(ParamFreeMirDispatchSchemaV1);
encode_schema!(DecodedParamFreeMirDispatchSchemaV1);
impl WireDecode for DecodedParamFreeMirDispatchSchemaV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            owner: decoder.field(1, DecodedPersistentId::decode)?,
            vtable: decoder.field(2, DecodedVtable::decode)?,
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
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalMirDispatchSchemasV1, MirDispatchSchemaError> {
        self.validate_with_dependencies(graph, types, callables, &[], meter)
    }
    pub fn validate_with_dependencies(
        self,
        graph: &mut ValidatedIdentityGraph,
        types: &dyn MirTypeBridgeTypeLookupV1,
        callables: &dyn MirTypeBridgeCallableLookupV1,
        dependencies: &[&CanonicalMirDispatchSchemasV1],
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalMirDispatchSchemasV1, MirDispatchSchemaError> {
        let mut records: Vec<ParamFreeMirDispatchSchemaV1> = reserve(self.records.len(), meter)?;
        for (index, record) in self.records.into_iter().enumerate() {
            let record = record.validate(graph, types, callables, meter)?;
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
        .validate_with_dependencies(&table, dependencies, meter)?;
        Ok(table)
    }
}
impl WireDecode for DecodedCanonicalMirDispatchSchemasV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
