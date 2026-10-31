//! Decode the member assignments and symbol tables saved by the producer.

use crate::SlibMemberId;
use crate::link_object::{
    CanonicalDefinedLinkSymbolOwnerSetV1, DecodedCanonicalDefinedLinkSymbolOwnerSetV1,
    DecodedFixedBytesV1,
};
use scoop_identity::{DecodedPersistentId, ObjectDefinitionPlanId, PersistentId};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};
mod read;
use super::SymbolUseGroup;

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedLinkObjectMaterializationV1 {
    ScoopLir {
        member: DecodedFixedBytesV1<SlibMemberId>,
        units: Vec<DecodedPersistentId<ObjectDefinitionPlanId>>,
    },
    GeneratedCBridge {
        member: DecodedFixedBytesV1<SlibMemberId>,
    },
}

impl WireEncode for DecodedLinkObjectMaterializationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(match self {
            Self::ScoopLir { .. } => 3,
            Self::GeneratedCBridge { .. } => 2,
        })?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::ScoopLir { .. } => 1,
            Self::GeneratedCBridge { .. } => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::ScoopLir { member, .. } | Self::GeneratedCBridge { member, .. } => {
                member.encode(encoder)?;
            }
        }
        if let Self::ScoopLir { units, .. } = self {
            encoder.field(2)?;
            encode_array(encoder, units)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedLinkObjectMaterializationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        let expected = match tag {
            1 => 3,
            2 => 2,
            tag => return Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        };
        if fields != expected {
            return Err(wire_error(
                decoder,
                WireErrorKind::InvalidLength {
                    expected,
                    actual: fields,
                },
            ));
        }
        let member = decoder.field(1, DecodedFixedBytesV1::decode)?;
        match tag {
            1 => Ok(Self::ScoopLir {
                member,
                units: decoder.field(2, decode_persistent_id_array)?,
            }),
            2 => Ok(Self::GeneratedCBridge { member }),
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedLinkIdentityClosureSectionV1 {
    materializations: Vec<DecodedLinkObjectMaterializationV1>,
    defined_symbols: DecodedCanonicalDefinedLinkSymbolOwnerSetV1,
    symbol_uses: Vec<SymbolUseGroup>,
}

impl DecodedLinkIdentityClosureSectionV1 {
    pub(crate) fn read_defined_symbols(
        &self,
        foundation: &scoop_lir::ConeLirFoundation,
        identities: &mut scoop_identity::ValidatedIdentityGraph,
        normalization: scoop_lir::NativeSymbolNormalization,
    ) -> Result<CanonicalDefinedLinkSymbolOwnerSetV1, String> {
        self.defined_symbols
            .read(foundation, identities, normalization)
    }
}

impl WireEncode for DecodedLinkIdentityClosureSectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encode_array(encoder, &self.materializations)?;
        encoder.field(4)?;
        self.defined_symbols.encode(encoder)?;
        encoder.field(5)?;
        encode_array(encoder, &self.symbol_uses)
    }
}

impl WireDecode for DecodedLinkIdentityClosureSectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            materializations: decoder.field(1, |decoder| {
                decoder
                    .decode_array(|decoder, _| DecodedLinkObjectMaterializationV1::decode(decoder))
            })?,
            defined_symbols: decoder
                .field(4, DecodedCanonicalDefinedLinkSymbolOwnerSetV1::decode)?,
            symbol_uses: decoder.field(5, |decoder| {
                decoder.decode_array(|decoder, _| SymbolUseGroup::decode(decoder))
            })?,
        })
    }
}

fn decode_persistent_id_array<I: PersistentId>(
    decoder: &mut Decoder<'_>,
) -> Result<Vec<DecodedPersistentId<I>>, WireError> {
    decoder.decode_array(|decoder, _| DecodedPersistentId::decode(decoder))
}

fn encode_array(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
pub(crate) mod tests;
