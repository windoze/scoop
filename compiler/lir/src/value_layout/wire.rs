use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{DecodedRefScanV1, LirTargetProfile};

use super::*;

#[derive(Debug)]
pub struct DecodedValueStorageLayoutV1(RawStorage);

#[derive(Debug)]
pub struct DecodedArrayElementStorageV1(RawStorage);

#[derive(Debug)]
enum RawStorage {
    ZeroSized {
        alignment: u64,
    },
    NonZero {
        extent: u64,
        alignment: u64,
        scan: DecodedRefScanV1,
    },
}

impl RawStorage {
    fn validate(
        self,
        target: LirTargetProfile,
    ) -> Result<ValueStorageLayoutV1, TypeInstanceShapeError> {
        let value = match self {
            Self::ZeroSized { alignment } => ValueStorageLayoutV1::zero_sized(alignment)?,
            Self::NonZero {
                extent,
                alignment,
                scan,
            } => {
                let scan = scan.validate().map_err(TypeInstanceShapeError::Scan)?;
                ValueStorageLayoutV1::inline(extent, alignment, scan.into_ref_scan())?
            }
        };
        value.validate_target(target)?;
        Ok(value)
    }
}

impl DecodedValueStorageLayoutV1 {
    pub fn validate(
        self,
        target: LirTargetProfile,
    ) -> Result<ValueStorageLayoutV1, TypeInstanceShapeError> {
        self.0.validate(target)
    }
}

impl DecodedArrayElementStorageV1 {
    pub fn validate(
        self,
        target: LirTargetProfile,
    ) -> Result<ArrayElementStorageV1, TypeInstanceShapeError> {
        self.0
            .validate(target)
            .map(|value| ArrayElementStorageV1::from_value(&value))
    }
}

impl WireDecode for DecodedValueStorageLayoutV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decode_storage(decoder).map(Self)
    }
}

impl WireDecode for DecodedArrayElementStorageV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decode_storage(decoder).map(Self)
    }
}

impl WireEncode for ValueStorageLayoutV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        // The wire NonZero tag is explicit and independent of the Rust body.
        match self.kind() {
            ValueStorageKindV1::ZeroSized { alignment } => encode_zst(encoder, alignment.get()),
            ValueStorageKindV1::Inline {
                size,
                alignment,
                scan,
            } => encode_inline(encoder, size.get(), alignment.get(), scan),
        }
    }
}

impl WireEncode for ArrayElementStorageV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.kind() {
            ArrayElementStorageKindV1::ZeroSized { alignment } => {
                encode_zst(encoder, alignment.get())
            }
            ArrayElementStorageKindV1::Inline {
                stride,
                alignment,
                scan,
            } => encode_inline(encoder, stride.get(), alignment.get(), scan),
        }
    }
}

impl WireEncode for RawStorage {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ZeroSized { alignment } => encode_zst(encoder, *alignment),
            Self::NonZero {
                extent,
                alignment,
                scan,
            } => encode_inline(encoder, *extent, *alignment, scan),
        }
    }
}

impl WireEncode for DecodedValueStorageLayoutV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

impl WireEncode for DecodedArrayElementStorageV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

fn decode_storage(decoder: &mut Decoder<'_, '_>) -> Result<RawStorage, WireError> {
    let actual = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    let expected = match tag {
        1 => 2,
        2 => 4,
        _ => return Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
    };
    if actual != expected {
        return Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ));
    }
    match tag {
        1 => Ok(RawStorage::ZeroSized {
            alignment: decoder.field(1, Decoder::unsigned)?,
        }),
        _ => Ok(RawStorage::NonZero {
            extent: decoder.field(1, Decoder::unsigned)?,
            alignment: decoder.field(2, Decoder::unsigned)?,
            scan: decoder.field(3, DecodedRefScanV1::decode)?,
        }),
    }
}

fn encode_zst(encoder: &mut Encoder, alignment: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encoder.field(0)?;
    encoder.unsigned(1)?;
    encoder.field(1)?;
    encoder.unsigned(alignment)
}

fn encode_inline(
    encoder: &mut Encoder,
    extent: u64,
    alignment: u64,
    scan: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encoder.field(0)?;
    encoder.unsigned(2)?;
    encoder.field(1)?;
    encoder.unsigned(extent)?;
    encoder.field(2)?;
    encoder.unsigned(alignment)?;
    encoder.field(3)?;
    scan.encode(encoder)
}

#[cfg(test)]
mod tests;

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
