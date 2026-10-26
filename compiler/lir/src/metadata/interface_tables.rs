//! Physical interface-table cells and views retain their logical slot origins.

use scoop_identity::{
    DecodedPersistentId, PersistentDispatchTableId, PersistentIdResolver, ValidatedIdentityGraph,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatchSlotSource {
    pub table: PersistentDispatchTableId,
    pub method: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchTableLayout {
    storage: PersistentDispatchTableId,
    offset: u32,
    slots: Vec<DispatchSlotSource>,
}

impl DispatchTableLayout {
    pub fn flat(table: PersistentDispatchTableId, methods: usize) -> Self {
        Self {
            storage: table,
            offset: 0,
            slots: (0..methods)
                .map(|method| DispatchSlotSource {
                    table,
                    method: u32::try_from(method).expect("dispatch slots fit u32"),
                })
                .collect(),
        }
    }

    pub fn new(
        storage: PersistentDispatchTableId,
        offset: u32,
        slots: Vec<DispatchSlotSource>,
    ) -> Self {
        Self {
            storage,
            offset,
            slots,
        }
    }

    pub const fn storage(&self) -> PersistentDispatchTableId {
        self.storage
    }

    /// Offset in pointer-sized cells from the owning static array.
    pub const fn offset(&self) -> u32 {
        self.offset
    }

    pub fn slots(&self) -> &[DispatchSlotSource] {
        &self.slots
    }
}

impl WireEncode for DispatchTableLayout {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.storage.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.offset))?;
        encoder.field(3)?;
        encoder.array(self.slots.len() as u64)?;
        for slot in &self.slots {
            encoder.map(2)?;
            encoder.field(1)?;
            slot.table.encode(encoder)?;
            encoder.field(2)?;
            encoder.unsigned(u64::from(slot.method))?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DecodedDispatchTableLayout {
    storage: DecodedPersistentId<PersistentDispatchTableId>,
    offset: u32,
    slots: Vec<(DecodedPersistentId<PersistentDispatchTableId>, u32)>,
}

impl WireDecode for DecodedDispatchTableLayout {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            storage: decoder.field(1, DecodedPersistentId::decode)?,
            offset: decoder.field(2, Decoder::u32)?,
            slots: decoder.field(3, |decoder| {
                decoder.decode_array(|decoder, _| {
                    decoder.expect_map(2)?;
                    Ok((
                        decoder.field(1, DecodedPersistentId::decode)?,
                        decoder.field(2, Decoder::u32)?,
                    ))
                })
            })?,
        })
    }
}

impl WireEncode for DecodedDispatchTableLayout {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.storage.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.offset))?;
        encoder.field(3)?;
        encoder.array(self.slots.len() as u64)?;
        for (table, method) in &self.slots {
            encoder.map(2)?;
            encoder.field(1)?;
            table.encode(encoder)?;
            encoder.field(2)?;
            encoder.unsigned(u64::from(*method))?;
        }
        Ok(())
    }
}

impl DecodedDispatchTableLayout {
    pub(crate) fn read(
        self,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<DispatchTableLayout, crate::LinkDataError> {
        use crate::link_data::link_error;
        let storage = identities.resolve(self.storage).map_err(link_error)?;
        let slots = self
            .slots
            .into_iter()
            .map(|(table, method)| {
                Ok(DispatchSlotSource {
                    table: identities.resolve(table).map_err(link_error)?,
                    method,
                })
            })
            .collect::<Result<Vec<_>, crate::LinkDataError>>()?;
        Ok(DispatchTableLayout::new(storage, self.offset, slots))
    }
}
