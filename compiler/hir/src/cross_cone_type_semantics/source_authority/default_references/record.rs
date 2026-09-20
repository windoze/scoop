use crate::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

/// One source occurrence; its sequence position is not an expression index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefaultSourceReferenceV1<T> {
    target: T,
    definition_origin: ExportDefinitionSourceV1,
    witness: DefaultSourceAccessWitnessV1,
}
impl<T> DefaultSourceReferenceV1<T> {
    pub const fn new(
        target: T,
        definition_origin: ExportDefinitionSourceV1,
        witness: DefaultSourceAccessWitnessV1,
    ) -> Self {
        Self {
            target,
            definition_origin,
            witness,
        }
    }
    pub const fn target(&self) -> &T {
        &self.target
    }
    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
    pub const fn witness(&self) -> &DefaultSourceAccessWitnessV1 {
        &self.witness
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedDefaultSourceReferenceV1<T> {
    pub(super) target: T,
    pub(super) definition_origin: DecodedExportDefinitionSourceV1,
    pub(super) witness: DecodedDefaultSourceAccessWitnessV1,
}
impl<T: WireDecode> WireDecode for DecodedDefaultSourceReferenceV1<T> {
    fn decode(d: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        d.expect_map(3)?;
        Ok(Self {
            target: d.field(1, T::decode)?,
            definition_origin: d.field(2, DecodedExportDefinitionSourceV1::decode)?,
            witness: d.field(3, DecodedDefaultSourceAccessWitnessV1::decode)?,
        })
    }
}
macro_rules! encode_record {
    ($name:ident) => {
        impl<T: WireEncode> WireEncode for $name<T> {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                e.map(3)?;
                e.field(1)?;
                self.target.encode(e)?;
                e.field(2)?;
                self.definition_origin.encode(e)?;
                e.field(3)?;
                self.witness.encode(e)
            }
        }
    };
}
encode_record!(DefaultSourceReferenceV1);
encode_record!(DecodedDefaultSourceReferenceV1);
