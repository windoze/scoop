//! Untrusted wire carriers for complete strong registration production.

use scoop_identity::{
    DecodedPersistentId, DecodedPersistentSymbolRequest, DigestNodeId, DigestPatchIntentId,
    ObjectDefinitionAtomId, ObjectDefinitionPlanId, PersistentCallableBodyId,
    PersistentDispatchTableId, PersistentExactTypeId, PersistentImmortalObjectId,
    PersistentInitializationUnitId, PersistentLayoutId, PersistentSafepointSiteId,
    PersistentScanId, PersistentStaticStorageId,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::DecodedStrongRegistrationIdentitySurfaceV1;

mod projection;
pub use projection::StrongSemanticProjectionError;
use projection::compare_semantics;

mod surface;
pub use surface::*;

mod type_plans;
pub use type_plans::*;
mod callables;
pub use callables::*;
mod type_shape;
pub(super) use type_shape::*;
mod type_references;
pub use type_references::*;
mod type_scan;
pub(super) use type_scan::*;
mod immortal;
pub use immortal::*;
mod scan;
pub(super) use scan::*;
mod static_storage;
pub use static_storage::*;
mod initialization;
pub use initialization::*;

fn encode_field(
    encoder: &mut Encoder,
    field: u32,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    value.encode(encoder)
}

fn encode_unsigned_field(
    encoder: &mut Encoder,
    field: u32,
    value: u64,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.unsigned(value)
}

fn encode_array_field<T: WireEncode>(
    encoder: &mut Encoder,
    field: u32,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(field)?;
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn decode_array_field<T>(
    decoder: &mut Decoder<'_, '_>,
    field: u32,
    decode: impl Fn(&mut Decoder<'_, '_>) -> Result<T, WireError>,
) -> Result<Vec<T>, WireError> {
    decoder.field(field, |decoder| {
        decoder.decode_array(|decoder, _| decode(decoder))
    })
}

fn encode_empty_sum(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_unsigned_field(encoder, 0, tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_unsigned_field(encoder, 0, tag)?;
    encode_field(encoder, 1, value)
}

fn unknown_tag(decoder: &Decoder<'_, '_>, tag: u64) -> WireError {
    WireError::new(
        scoop_wire::WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn require_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            scoop_wire::WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_the_obsolete_seven_field_registration_surface() {
        let error = scoop_wire::decode_canonical::<DecodedStrongRegistrationProductionSurfaceV1>(
            &[0xa7],
            scoop_wire::DecodeLimits::default(),
        )
        .unwrap_err();
        assert!(matches!(
            error.kind(),
            scoop_wire::WireErrorKind::InvalidLength {
                expected: 8,
                actual: 7
            }
        ));
    }
}
