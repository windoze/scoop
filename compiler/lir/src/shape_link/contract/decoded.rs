use scoop_identity::{DecodedCanonicalScoopAbiFunctionSignature, DecodedPersistentId};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::*;
use crate::shape_link::wire::{EncodeResult, field, length, unknown};
use crate::{
    DecodedExactDescriptorSemanticProjectionV1, DecodedExactDispatchSemanticProjectionV1,
    DecodedExactLayoutSemanticProjectionV1, DecodedRefScanV1,
    DecodedStrongInitializationUnitSemanticProjectionV1,
    DecodedStrongStaticStorageSemanticProjectionV1,
};

#[derive(Debug)]
pub enum DecodedShapeLinkContractV1 {
    CallableAbi {
        canonical_signature: DecodedCanonicalScoopAbiFunctionSignature,
        calling_convention: CallingConvention,
        protocol: ExactCallableProtocolV1,
    },
    Layout(DecodedExactLayoutSemanticProjectionV1),
    Scan {
        layout: DecodedPersistentId<PersistentLayoutId>,
        role: ScanRole,
        canonical_scan: DecodedRefScanV1,
    },
    Type(DecodedExactDescriptorSemanticProjectionV1),
    Dispatch(DecodedExactDispatchSemanticProjectionV1),
    StaticStorage(DecodedStrongStaticStorageSemanticProjectionV1),
    Initialization(DecodedStrongInitializationUnitSemanticProjectionV1),
}

impl DecodedShapeLinkContractV1 {
    pub const fn tag(&self) -> u32 {
        match self {
            Self::CallableAbi { .. } => 1,
            Self::Layout(_) => 2,
            Self::Scan { .. } => 3,
            Self::Type(_) => 4,
            Self::Dispatch(_) => 5,
            Self::StaticStorage(_) => 6,
            Self::Initialization(_) => 7,
        }
    }
}

impl WireDecode for DecodedShapeLinkContractV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        if !(1..=7).contains(&tag) {
            return Err(unknown(decoder, tag));
        }
        length(decoder, fields, if tag == 1 || tag == 3 { 4 } else { 2 })?;
        Ok(match tag {
            1 => Self::CallableAbi {
                canonical_signature: decoder
                    .field(1, DecodedCanonicalScoopAbiFunctionSignature::decode)?,
                calling_convention: decoder.field(2, CallingConvention::decode)?,
                protocol: decoder.field(3, ExactCallableProtocolV1::decode)?,
            },
            2 => Self::Layout(decoder.field(1, DecodedExactLayoutSemanticProjectionV1::decode)?),
            3 => Self::Scan {
                layout: decoder.field(1, DecodedPersistentId::decode)?,
                role: decoder.field(2, ScanRole::decode)?,
                canonical_scan: decoder.field(3, DecodedRefScanV1::decode)?,
            },
            4 => Self::Type(decoder.field(1, DecodedExactDescriptorSemanticProjectionV1::decode)?),
            5 => {
                Self::Dispatch(decoder.field(1, DecodedExactDispatchSemanticProjectionV1::decode)?)
            }
            6 => Self::StaticStorage(
                decoder.field(1, DecodedStrongStaticStorageSemanticProjectionV1::decode)?,
            ),
            7 => Self::Initialization(decoder.field(
                1,
                DecodedStrongInitializationUnitSemanticProjectionV1::decode,
            )?),
            _ => return Err(unknown(decoder, tag)),
        })
    }
}

impl WireEncode for DecodedShapeLinkContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(if self.tag() == 1 || self.tag() == 3 {
            4
        } else {
            2
        })?;
        encoder.field(0)?;
        encoder.unsigned(u64::from(self.tag()))?;
        match self {
            Self::CallableAbi {
                canonical_signature,
                calling_convention,
                protocol,
            } => {
                field(encoder, 1, canonical_signature)?;
                field(encoder, 2, calling_convention)?;
                field(encoder, 3, protocol)
            }
            Self::Layout(record) => field(encoder, 1, record),
            Self::Scan {
                layout,
                role,
                canonical_scan,
            } => {
                field(encoder, 1, layout)?;
                field(encoder, 2, role)?;
                field(encoder, 3, canonical_scan)
            }
            Self::Type(record) => field(encoder, 1, record),
            Self::Dispatch(record) => field(encoder, 1, record),
            Self::StaticStorage(record) => field(encoder, 1, record),
            Self::Initialization(record) => field(encoder, 1, record),
        }
    }
}
