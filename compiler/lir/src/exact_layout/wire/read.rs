//! Raw complete layout records. Refinement compares every constituent with
//! a record replayed from canonical source identities and checked inputs.

use scoop_identity::{
    DecodedCapabilityId, DecodedPersistentId, PersistentExactTypeId, PersistentLayoutId,
    PersistentScanId, RepresentationRole,
};
use scoop_wire::{Decoder, WireDecode, WireError, WireErrorKind};

use super::*;
use crate::{DecodedTypeInstanceShapeV1, DecodedValueStorageLayoutV1};

mod fields;
mod instance;
#[cfg(test)]
mod tests;
mod validation;
mod value;

use fields::*;
use instance::*;
pub use validation::ExactLayoutWireError;
use value::*;

#[derive(Debug)]
pub struct DecodedExactLayoutExportV1 {
    semantic: DecodedExactLayoutSemanticProjectionV1,
    definition: crate::production::DecodedStrongShapeDefinitionV1<PersistentLayoutId>,
}

#[derive(Debug)]
pub struct DecodedExactLayoutSemanticProjectionV1 {
    layout: DecodedPersistentId<PersistentLayoutId>,
    exact: DecodedPersistentId<PersistentExactTypeId>,
    target: DecodedCapabilityId,
    role: RepresentationRole,
    body: RawBody,
    scan: DecodedPersistentId<PersistentScanId>,
}

#[derive(Debug)]
enum RawBody {
    Value {
        storage: DecodedValueStorageLayoutV1,
        representation: RawValue,
    },
    Instance {
        shape: DecodedTypeInstanceShapeV1,
        representation: RawInstance,
    },
}

impl WireDecode for DecodedExactLayoutExportV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(7)?;
        Ok(Self {
            semantic: DecodedExactLayoutSemanticProjectionV1::decode_fields(decoder)?,
            definition: decoder
                .field(7, crate::production::DecodedStrongShapeDefinitionV1::decode)?,
        })
    }
}

impl WireEncode for DecodedExactLayoutExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(7)?;
        self.semantic.encode_fields(encoder)?;
        field(encoder, 7, &self.definition)
    }
}

impl WireDecode for RawBody {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        match decoder.field(0, Decoder::unsigned)? {
            1 => Ok(Self::Value {
                storage: decoder.field(1, DecodedValueStorageLayoutV1::decode)?,
                representation: decoder.field(2, RawValue::decode)?,
            }),
            2 => Ok(Self::Instance {
                shape: decoder.field(1, DecodedTypeInstanceShapeV1::decode)?,
                representation: decoder.field(2, RawInstance::decode)?,
            }),
            tag => Err(unknown(decoder, tag)),
        }
    }
}

impl WireEncode for RawBody {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        match self {
            Self::Value {
                storage,
                representation,
            } => {
                sum(encoder, 1, 2)?;
                field(encoder, 1, storage)?;
                field(encoder, 2, representation)
            }
            Self::Instance {
                shape,
                representation,
            } => {
                sum(encoder, 2, 2)?;
                field(encoder, 1, shape)?;
                field(encoder, 2, representation)
            }
        }
    }
}

fn unknown(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

fn bool_value(decoder: &mut Decoder<'_>) -> Result<bool, WireError> {
    match decoder.unsigned()? {
        0 => Ok(false),
        1 => Ok(true),
        tag => Err(unknown(decoder, tag)),
    }
}

fn pointer_kind(decoder: &mut Decoder<'_>) -> Result<crate::NichePointerKind, WireError> {
    decoder.expect_map(1)?;
    match decoder.field(0, Decoder::unsigned)? {
        1 => Ok(crate::NichePointerKind::Managed),
        2 => Ok(crate::NichePointerKind::Raw),
        3 => Ok(crate::NichePointerKind::Code),
        tag => Err(unknown(decoder, tag)),
    }
}

fn table<T: WireDecode>(decoder: &mut Decoder<'_>, index: u32) -> Result<Vec<T>, WireError> {
    decoder.field(index, |decoder| {
        decoder.decode_array(|decoder, _| T::decode(decoder))
    })
}

impl DecodedExactLayoutSemanticProjectionV1 {
    fn decode_fields(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        Ok(Self {
            layout: decoder.field(1, DecodedPersistentId::decode)?,
            exact: decoder.field(2, DecodedPersistentId::decode)?,
            target: decoder.field(3, DecodedCapabilityId::decode)?,
            role: decoder.field(4, RepresentationRole::decode)?,
            body: decoder.field(5, RawBody::decode)?,
            scan: decoder.field(6, DecodedPersistentId::decode)?,
        })
    }

    fn encode_fields(&self, encoder: &mut Encoder) -> EncodeResult {
        field(encoder, 1, &self.layout)?;
        field(encoder, 2, &self.exact)?;
        field(encoder, 3, &self.target)?;
        field(encoder, 4, &self.role)?;
        field(encoder, 5, &self.body)?;
        field(encoder, 6, &self.scan)
    }
}
impl WireDecode for DecodedExactLayoutSemanticProjectionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Self::decode_fields(decoder)
    }
}
impl WireEncode for DecodedExactLayoutSemanticProjectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(6)?;
        self.encode_fields(encoder)
    }
}
