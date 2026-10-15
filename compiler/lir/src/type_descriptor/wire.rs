//! Saved instance geometry and GC scan data.

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::{DecodedRefScanV1, TypeInstanceShapeV1};

#[derive(Debug)]
pub struct DecodedTypeInstanceShapeV1 {
    scalars: [u64; 8],
    object_scan: DecodedRefScanV1,
    inline_scan: DecodedRefScanV1,
}

impl DecodedTypeInstanceShapeV1 {
    pub fn read(
        self,
        target: crate::LirTargetProfile,
    ) -> Result<TypeInstanceShapeV1, crate::LinkDataError> {
        use super::{InlineStorageKindV1 as Inline, TypeInstanceKindV1 as Kind};
        use crate::link_data::link_error;
        let [
            kind,
            inline,
            minimum_size,
            instance_alignment,
            inline_offset,
            inline_size,
            inline_stride,
            inline_alignment,
        ] = self.scalars;
        let instance_kind = match kind {
            1 => Kind::FixedObject,
            2 => Kind::BoxedValue,
            3 => Kind::InlineBytes,
            4 => Kind::InlineArray,
            5 => Kind::AbstractRef,
            _ => return Err(link_error("unknown instance kind")),
        };
        let inline_storage_kind = match inline {
            0 => Inline::None,
            1 => Inline::Inline,
            2 => Inline::ZeroSized,
            _ => return Err(link_error("unknown inline storage kind")),
        };
        let object_scan = self.object_scan.validate().map_err(link_error)?;
        let inline_scan = self.inline_scan.validate().map_err(link_error)?;
        if instance_kind != Kind::AbstractRef {
            crate::StorageGeometryV1::new(target, minimum_size, instance_alignment)
                .map_err(link_error)?;
            super::require_managed_object_size(target, minimum_size).map_err(link_error)?;
            object_scan
                .validate_extent(minimum_size, instance_alignment)
                .map_err(link_error)?;
        }
        if inline_storage_kind != Inline::None {
            crate::NonZeroPow2::new(inline_alignment).map_err(link_error)?;
            inline_scan
                .validate_extent(inline_size, inline_alignment)
                .map_err(link_error)?;
        }
        Ok(TypeInstanceShapeV1 {
            instance_kind,
            inline_storage_kind,
            minimum_size,
            instance_alignment,
            inline_offset,
            inline_size,
            inline_stride,
            inline_alignment,
            object_scan: object_scan.into_ref_scan(),
            inline_scan: inline_scan.into_ref_scan(),
        })
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

#[cfg(test)]
mod tests;
