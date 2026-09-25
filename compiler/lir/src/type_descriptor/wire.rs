//! Frozen physical shape product. Its scalars are compared with a shape
//! rebuilt from the representation's typed dependencies before promotion.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{DecodedRefScanV1, RefScanValidationError, TypeInstanceShapeV1};

#[derive(Debug)]
pub struct DecodedTypeInstanceShapeV1 {
    scalars: [u64; 8],
    object_scan: DecodedRefScanV1,
    inline_scan: DecodedRefScanV1,
}

impl DecodedTypeInstanceShapeV1 {
    pub fn validate_against(
        self,
        expected: &TypeInstanceShapeV1,
    ) -> Result<(), TypeInstanceShapeWireError> {
        if self.scalars != shape_scalars(expected) {
            return Err(TypeInstanceShapeWireError::ShapeMismatch);
        }
        let object_scan = self.object_scan.validate()?;
        let inline_scan = self.inline_scan.validate()?;
        if object_scan.as_ref_scan() != expected.object_scan()
            || inline_scan.as_ref_scan() != expected.inline_scan()
        {
            return Err(TypeInstanceShapeWireError::ScanMismatch);
        }
        Ok(())
    }
}

impl WireEncode for TypeInstanceShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_scalars(encoder, shape_scalars(self))?;
        encoder.field(9)?;
        crate::scan::encode_canonical_scan(self.object_scan(), encoder)?;
        encoder.field(10)?;
        crate::scan::encode_canonical_scan(self.inline_scan(), encoder)
    }
}

impl WireEncode for DecodedTypeInstanceShapeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_scalars(encoder, self.scalars)?;
        encoder.field(9)?;
        self.object_scan.encode(encoder)?;
        encoder.field(10)?;
        self.inline_scan.encode(encoder)
    }
}

impl WireDecode for DecodedTypeInstanceShapeV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(10)?;
        let mut scalars = [0; 8];
        for (index, scalar) in scalars.iter_mut().enumerate() {
            *scalar = decoder.field(index as u32 + 1, Decoder::unsigned)?;
        }
        Ok(Self {
            scalars,
            object_scan: decoder.field(9, DecodedRefScanV1::decode)?,
            inline_scan: decoder.field(10, DecodedRefScanV1::decode)?,
        })
    }
}

fn shape_scalars(shape: &TypeInstanceShapeV1) -> [u64; 8] {
    [
        u64::from(shape.instance_kind().tag()),
        u64::from(shape.inline_storage_kind().tag()),
        shape.minimum_size(),
        shape.instance_alignment(),
        shape.inline_offset(),
        shape.inline_size(),
        shape.inline_stride(),
        shape.inline_alignment(),
    ]
}

fn encode_scalars(
    encoder: &mut Encoder,
    scalars: [u64; 8],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(10)?;
    for (index, scalar) in scalars.into_iter().enumerate() {
        encoder.field(index as u32 + 1)?;
        encoder.unsigned(scalar)?;
    }
    Ok(())
}

#[derive(Debug)]
pub enum TypeInstanceShapeWireError {
    ShapeMismatch,
    ScanMismatch,
    Scan(RefScanValidationError),
    Resource(WireError),
}
impl From<RefScanValidationError> for TypeInstanceShapeWireError {
    fn from(error: RefScanValidationError) -> Self {
        Self::Scan(error)
    }
}
impl From<WireError> for TypeInstanceShapeWireError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for TypeInstanceShapeWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "instance shape wire does not match replay: {self:?}")
    }
}
impl std::error::Error for TypeInstanceShapeWireError {}

#[cfg(test)]
mod tests;
