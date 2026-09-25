use super::super::wire as codec;
use super::*;
use crate::DecodedInheritanceCallableDeclarationV1;
use scoop_identity::{
    DecodedPersistentId, PersistentFunctionId, PersistentIdResolver, PersistentPropertyAccessorId,
};
use scoop_wire::{Decoder, WireDecode, WireError, WireErrorKind};

impl WireEncode for InheritanceSourceSlotSelectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, declaration) = match self {
            Self::Abstract => return codec::tag(encoder, 1, 1),
            Self::Concrete(declaration) => (2, declaration),
            Self::InterfaceDefault(declaration) => (3, declaration),
        };
        codec::tag(encoder, 2, tag)?;
        encoder.field(1)?;
        declaration.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodedInheritanceSourceSlotSelectionV1 {
    Abstract,
    Concrete(DecodedInheritanceCallableDeclarationV1),
    InterfaceDefault(DecodedInheritanceCallableDeclarationV1),
}

impl WireEncode for DecodedInheritanceSourceSlotSelectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, declaration) = match self {
            Self::Abstract => return codec::tag(encoder, 1, 1),
            Self::Concrete(declaration) => (2, declaration),
            Self::InterfaceDefault(declaration) => (3, declaration),
        };
        codec::tag(encoder, 2, tag)?;
        encoder.field(1)?;
        declaration.encode(encoder)
    }
}

impl WireDecode for DecodedInheritanceSourceSlotSelectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => {
                codec::expect_fields(decoder, fields, 1)?;
                Ok(Self::Abstract)
            }
            tag @ (2 | 3) => {
                codec::expect_fields(decoder, fields, 2)?;
                let declaration =
                    decoder.field(1, DecodedInheritanceCallableDeclarationV1::decode)?;
                Ok(if tag == 2 {
                    Self::Concrete(declaration)
                } else {
                    Self::InterfaceDefault(declaration)
                })
            }
            tag => Err(codec::error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireEncode for InheritanceSourceSlotSelectionRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.slot.encode(encoder)?;
        encoder.field(3)?;
        self.selection.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedInheritanceSourceSlotSelectionRecordV1 {
    owner: DecodedPersistentId<PersistentExactTypeId>,
    slot: DecodedPersistentId<PersistentDispatchSlotId>,
    selection: DecodedInheritanceSourceSlotSelectionV1,
}

impl WireEncode for DecodedInheritanceSourceSlotSelectionRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.owner.encode(encoder)?;
        encoder.field(2)?;
        self.slot.encode(encoder)?;
        encoder.field(3)?;
        self.selection.encode(encoder)
    }
}

impl WireDecode for DecodedInheritanceSourceSlotSelectionRecordV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            owner: decoder.field(1, DecodedPersistentId::decode)?,
            slot: decoder.field(2, DecodedPersistentId::decode)?,
            selection: decoder.field(3, DecodedInheritanceSourceSlotSelectionV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalInheritanceSourceSlotSelectionsV1 {
    records: Vec<DecodedInheritanceSourceSlotSelectionRecordV1>,
}

impl DecodedCanonicalInheritanceSourceSlotSelectionsV1 {
    pub fn resolve<R, E: fmt::Display>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalInheritanceSourceSlotSelectionsV1, SourceInventoryError>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentDispatchSlotId, Error = E>
            + PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>,
    {
        let mut records = reserve(self.records.len())?;
        for record in self.records.into_iter() {
            let owner = resolver.resolve(record.owner).map_err(reference)?;
            let slot = resolver.resolve(record.slot).map_err(reference)?;
            let selection = match record.selection {
                DecodedInheritanceSourceSlotSelectionV1::Abstract => {
                    InheritanceSourceSlotSelectionV1::Abstract
                }
                DecodedInheritanceSourceSlotSelectionV1::Concrete(id) => {
                    InheritanceSourceSlotSelectionV1::Concrete(
                        id.resolve(resolver).map_err(reference)?,
                    )
                }
                DecodedInheritanceSourceSlotSelectionV1::InterfaceDefault(id) => {
                    InheritanceSourceSlotSelectionV1::InterfaceDefault(
                        id.resolve(resolver).map_err(reference)?,
                    )
                }
            };
            records.push(InheritanceSourceSlotSelectionRecordV1::new(
                owner, slot, selection,
            ));
        }
        CanonicalInheritanceSourceSlotSelectionsV1::from_ordered(records)
    }
}

impl WireEncode for DecodedCanonicalInheritanceSourceSlotSelectionsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        codec::sequence(encoder, &self.records)
    }
}

impl WireDecode for DecodedCanonicalInheritanceSourceSlotSelectionsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| {
                DecodedInheritanceSourceSlotSelectionRecordV1::decode(decoder)
            })
            .map(|records| Self { records })
    }
}
