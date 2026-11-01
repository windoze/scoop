//! One target record for all physical members that reference the same symbol.

use std::collections::BTreeMap;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    CanonicalDefinedLinkSymbolOwnerSetV1, SlibMemberId, StrongRelocationResolutionV1,
    VerifiedCurrentConeStrongRelocationClosureV1, encode_array,
};
use crate::link_object::DecodedFixedBytesV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SymbolUseTarget {
    Defined(u64),
    External(Vec<u8>),
}

impl SymbolUseTarget {
    pub(super) fn symbol<'a>(
        &'a self,
        defined: &'a CanonicalDefinedLinkSymbolOwnerSetV1,
    ) -> Result<&'a [u8], String> {
        match self {
            Self::Defined(index) => usize::try_from(*index)
                .ok()
                .and_then(|index| defined.owners().get(index))
                .map(|entry| entry.symbol())
                .ok_or_else(|| format!("symbol use definition index {index} is out of bounds")),
            Self::External(symbol) => Ok(symbol),
        }
    }
}

impl WireEncode for SymbolUseTarget {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::Defined(_) => 1,
            Self::External(_) => 2,
        })?;
        encoder.field(1)?;
        match self {
            Self::Defined(index) => encoder.unsigned(*index),
            Self::External(symbol) => encoder.bytes(symbol),
        }
    }
}

impl WireDecode for SymbolUseTarget {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => decoder.field(1, Decoder::unsigned).map(Self::Defined),
            2 => decoder.field(1, Decoder::owned_bytes).map(Self::External),
            tag => Err(WireError::new(
                WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SymbolUseGroup {
    pub(super) target: SymbolUseTarget,
    pub(super) members: Vec<SlibMemberId>,
}

impl WireEncode for SymbolUseGroup {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.target.encode(encoder)?;
        encoder.field(2)?;
        encode_array(encoder, &self.members)
    }
}

impl WireDecode for SymbolUseGroup {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            target: decoder.field(1, SymbolUseTarget::decode)?,
            members: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| {
                    DecodedFixedBytesV1::<SlibMemberId>::decode(decoder)
                        .map(|id| SlibMemberId::from_array(*id.bytes()))
                })
            })?,
        })
    }
}

pub(super) fn symbol_uses(
    relocations: &VerifiedCurrentConeStrongRelocationClosureV1,
    defined: &CanonicalDefinedLinkSymbolOwnerSetV1,
) -> Vec<SymbolUseGroup> {
    let mut groups = BTreeMap::<&[u8], Vec<SlibMemberId>>::new();
    for binding in relocations.bindings() {
        if !matches!(
            binding.resolution(),
            StrongRelocationResolutionV1::ObjectLocalStrong { .. }
        ) {
            groups
                .entry(binding.symbol())
                .or_default()
                .push(binding.source_member());
        }
    }
    groups
        .into_iter()
        .map(|(symbol, mut members)| {
            members.sort_unstable();
            members.dedup();
            let target = match defined
                .owners()
                .binary_search_by(|entry| entry.symbol().cmp(symbol))
            {
                Ok(index) => SymbolUseTarget::Defined(index as u64),
                Err(_) => SymbolUseTarget::External(symbol.to_vec()),
            };
            SymbolUseGroup { target, members }
        })
        .collect()
}
