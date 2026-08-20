use scoop_wire::{Encoder, WireEncode};

use super::*;

mod read;
pub use read::{
    DecodedCanonicalExactCallableAbiExportsV1, DecodedExactCallableAbiExportV1,
    ExactCallableAbiWireError,
};

impl WireEncode for ExactCallableAbiExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.target().encode(encoder)?;
        encoder.field(2)?;
        self.canonical_signature().encode(encoder)?;
        encoder.field(3)?;
        self.calling_convention().encode(encoder)?;
        encoder.field(4)?;
        self.call_protocol().encode(encoder)?;
        encoder.field(6)?;
        self.definition().encode(encoder)
    }
}

impl WireEncode for CanonicalExactCallableAbiExportsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records().len() as u64)?;
        for record in self.records() {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireEncode for ExactCallableProtocolV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(1)?;
        encoder.field(0)?;
        encoder.unsigned(match self {
            Self::OrdinaryManaged => 1,
            Self::OrdinaryNoGc => 2,
        })
    }
}
