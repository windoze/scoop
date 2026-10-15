use super::*;
use scoop_identity::{
    DecodedPersistentId, IdentityReferenceError, PersistentIdResolver, ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

#[derive(Debug)]
pub(crate) struct DecodedRuntimeImageRecords(Records<DecodedPersistentId<ObjectDefinitionPlanId>>);

impl DecodedRuntimeImageRecords {
    pub(crate) fn read(
        &self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<RuntimeImageRecords, IdentityReferenceError> {
        let mut result = RuntimeImageRecords::new(self.0.prefix.clone());
        for (index, table) in self.0.tables.iter().enumerate() {
            for record in table {
                result.push(
                    index,
                    PersistentIdResolver::resolve(identities, record.definition)?,
                    record.bytes.clone(),
                );
            }
        }
        Ok(result)
    }
}

impl<I: WireEncode> WireEncode for Records<I> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.bytes(&self.prefix)?;
        encoder.field(2)?;
        encoder.array(6)?;
        for table in &self.tables {
            encoder.array(table.len() as u64)?;
            for record in table {
                encoder.array(2)?;
                record.definition.encode(encoder)?;
                encoder.bytes(&record.bytes)?;
            }
        }
        Ok(())
    }
}

impl WireEncode for RuntimeImageRecords {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

impl WireEncode for DecodedRuntimeImageRecords {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

impl WireDecode for DecodedRuntimeImageRecords {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let prefix = decoder.field(1, Decoder::owned_bytes)?;
        let tables = decoder.field(2, |decoder| {
            expect_array(decoder, 6)?;
            let mut tables = std::array::from_fn(|_| Vec::new());
            for table in &mut tables {
                *table = decoder.decode_array(|decoder, _| {
                    expect_array(decoder, 2)?;
                    Ok(Record {
                        definition: DecodedPersistentId::decode(decoder)?,
                        bytes: decoder.owned_bytes()?,
                    })
                })?;
            }
            Ok(tables)
        })?;
        Ok(Self(Records { prefix, tables }))
    }
}

fn expect_array(decoder: &mut Decoder<'_>, expected: u64) -> Result<(), WireError> {
    let actual = decoder.array()?;
    if actual != expected {
        return Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ));
    }
    Ok(())
}
