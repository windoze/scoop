//! Representation-neutral, identity-checked MIR type exports.
//!
//! This constituent table is consumed by the complete type bridge section;
//! it does not grant import selection or production artifact eligibility.

use scoop_identity::{
    DecodedPersistentId, ExactTypeKey, GeneratedNominalKey, IdentityReferenceError,
    PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentExactTypeId,
    PersistentFieldId, PersistentTypeId, ValidatedIdentityGraph,
};
use scoop_wire::{
    BudgetMeter, Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind, WirePath,
};

mod callables;
mod dispatch;
mod facts;
mod lookup;
mod objects;
mod origin;
mod record;
mod representation;
mod representation_wire;
mod section;
mod semantic_refs;
mod shape_support;
mod source_join;
mod table;
#[cfg(test)]
mod tests;
mod validation;
mod wire;

pub use callables::*;
pub use dispatch::*;
pub use facts::*;
pub use lookup::*;
pub use objects::*;
pub use origin::*;
pub use record::*;
pub use representation::*;
pub use representation_wire::DecodedMirTypeRepresentationV1;
pub use section::*;
pub use semantic_refs::*;
pub use shape_support::*;
pub use source_join::*;
pub use table::*;
pub use validation::MirTypeBridgeError;
pub use wire::DecodedParamFreeMirTypeExportV1;

fn tag(encoder: &mut Encoder, fields: u64, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(fields)?;
    encoder.field(0)?;
    encoder.unsigned(tag)
}
fn sequence<T: WireEncode>(
    encoder: &mut Encoder,
    values: &[T],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}
fn error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}
fn fields(decoder: &Decoder<'_, '_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}
