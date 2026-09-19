use scoop_wire::{Encoder, WireEncode};

use super::*;

mod read;
pub use read::{DecodedExactCallableAbiExportV1, ExactCallableAbiWireError};

impl WireEncode for ExactCallableAbiExportV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.target().encode(encoder)?;
        encoder.field(2)?;
        self.canonical_signature().encode(encoder)?;
        encoder.field(3)?;
        self.calling_convention().encode(encoder)?;
        encoder.field(4)?;
        self.call_protocol().encode(encoder)?;
        encoder.field(5)?;
        self.layout_dependencies().encode(encoder)?;
        encoder.field(6)?;
        self.definition().encode(encoder)
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

impl WireEncode for CallableAbiLayoutDependenciesV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        match self.receiver() {
            CallableAbiReceiverLayoutV1::NoReceiver => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
            }
            CallableAbiReceiverLayoutV1::Receiver(value) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                value.identity().layout().encode(encoder)?;
            }
        }
        encoder.field(2)?;
        encoder.array(self.parameters().len() as u64)?;
        for parameter in self.parameters() {
            parameter.identity().layout().encode(encoder)?;
        }
        encoder.field(3)?;
        self.result().identity().layout().encode(encoder)
    }
}
