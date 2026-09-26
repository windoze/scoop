use scoop_wire::{Encoder, WireEncode};

use super::*;
use crate::shape_link::wire::{EncodeResult, field};

impl WireEncode for ShapeLinkContractV1 {
    fn encode(&self, encoder: &mut Encoder) -> EncodeResult {
        encoder.map(
            if matches!(self, Self::CallableAbi { .. } | Self::Scan { .. }) {
                4
            } else {
                2
            },
        )?;
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
            Self::Layout { record } => field(encoder, 1, &record.semantic_projection()),
            Self::Scan {
                layout,
                role,
                canonical_scan,
            } => {
                field(encoder, 1, layout)?;
                field(encoder, 2, role)?;
                encoder.field(3)?;
                crate::scan::encode_canonical_scan(canonical_scan, encoder)
            }
            Self::Type {
                descriptor_projection,
            } => field(encoder, 1, &descriptor_projection.semantic_projection()),
            Self::Dispatch { table_projection } => {
                field(encoder, 1, &table_projection.semantic_projection())
            }
            Self::StaticStorage { storage_projection } => {
                field(encoder, 1, &storage_projection.semantic_projection())
            }
            Self::Initialization { unit_projection } => {
                field(encoder, 1, &unit_projection.semantic_projection())
            }
        }
    }
}
