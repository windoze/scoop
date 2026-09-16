use scoop_wire::{DecodeLimits, Decoder, Digest256, Encoder, WireDecode, WireEncode, sha256};

use crate::framing::{ProtocolReadError, ProtocolWriteError, decode_frame_payload, encode_frame};
use crate::{PROTOCOL_VERSION, ProtocolValidationError};

const CAPABILITY_MAGIC: &str = "scoopc-machine-capability";
const REQUEST_SCHEMA_V1: &[u8] =
    b"scoopc-request-envelope-v1:magic,version,request-id,build-request";
const RESPONSE_SCHEMA_V1: &[u8] =
    b"scoopc-response-envelope-v1:magic,version,request-id,result,diagnostics,dumps";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MachineTransportCapabilityV1 {
    LengthPrefixedCanonicalCborStdio,
}

impl WireEncode for MachineTransportCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::LengthPrefixedCanonicalCborStdio => 1,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ScoopcProtocolCapabilityV1 {
    protocol_version: u32,
    request_schema: Digest256,
    response_schema: Digest256,
    machine_transport: MachineTransportCapabilityV1,
}

impl ScoopcProtocolCapabilityV1 {
    pub fn current() -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_schema: sha256(REQUEST_SCHEMA_V1),
            response_schema: sha256(RESPONSE_SCHEMA_V1),
            machine_transport: MachineTransportCapabilityV1::LengthPrefixedCanonicalCborStdio,
        }
    }

    pub const fn protocol_version(self) -> u32 {
        self.protocol_version
    }

    pub const fn request_schema(self) -> Digest256 {
        self.request_schema
    }

    pub const fn response_schema(self) -> Digest256 {
        self.response_schema
    }

    pub const fn machine_transport(self) -> MachineTransportCapabilityV1 {
        self.machine_transport
    }
}

impl WireEncode for ScoopcProtocolCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        encoder.text(CAPABILITY_MAGIC)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.protocol_version))?;
        encoder.field(3)?;
        self.request_schema.encode(encoder)?;
        encoder.field(4)?;
        self.response_schema.encode(encoder)?;
        encoder.field(5)?;
        self.machine_transport.encode(encoder)
    }
}

struct DecodedScoopcProtocolCapabilityV1 {
    magic: String,
    protocol_version: u32,
    request_schema: Digest256,
    response_schema: Digest256,
    machine_transport: MachineTransportCapabilityV1,
}

impl WireEncode for DecodedScoopcProtocolCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        encoder.text(&self.magic)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.protocol_version))?;
        encoder.field(3)?;
        self.request_schema.encode(encoder)?;
        encoder.field(4)?;
        self.response_schema.encode(encoder)?;
        encoder.field(5)?;
        self.machine_transport.encode(encoder)
    }
}

impl DecodedScoopcProtocolCapabilityV1 {
    fn validate(self) -> Result<ScoopcProtocolCapabilityV1, ProtocolValidationError> {
        if self.magic != CAPABILITY_MAGIC {
            return Err(ProtocolValidationError::InvalidCapabilityMagic);
        }
        if self.protocol_version != PROTOCOL_VERSION {
            return Err(ProtocolValidationError::UnsupportedVersion(
                self.protocol_version,
            ));
        }
        Ok(ScoopcProtocolCapabilityV1 {
            protocol_version: self.protocol_version,
            request_schema: self.request_schema,
            response_schema: self.response_schema,
            machine_transport: self.machine_transport,
        })
    }
}

impl WireDecode for DecodedScoopcProtocolCapabilityV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, scoop_wire::WireError> {
        decoder.expect_map(5)?;
        let magic = decoder.field(1, Decoder::owned_text)?;
        let version = decoder.field(2, Decoder::unsigned)?;
        let protocol_version = u32::try_from(version).map_err(|_| {
            scoop_wire::WireError::new(
                scoop_wire::WireErrorKind::IntegerOutOfRange,
                decoder.path().clone(),
                Some(decoder.position()),
            )
        })?;
        let request_schema = decoder.field(3, Digest256::decode)?;
        let response_schema = decoder.field(4, Digest256::decode)?;
        let machine_transport = decoder.field(5, decode_machine_transport)?;
        Ok(Self {
            magic,
            protocol_version,
            request_schema,
            response_schema,
            machine_transport,
        })
    }
}

fn decode_machine_transport(
    decoder: &mut Decoder<'_, '_>,
) -> Result<MachineTransportCapabilityV1, scoop_wire::WireError> {
    let tag = decoder.unsigned()?;
    match tag {
        1 => Ok(MachineTransportCapabilityV1::LengthPrefixedCanonicalCborStdio),
        _ => Err(scoop_wire::WireError::new(
            scoop_wire::WireErrorKind::UnknownTag { tag },
            decoder.path().clone(),
            Some(decoder.position()),
        )),
    }
}

pub fn encode_capability_frame(
    capability: &ScoopcProtocolCapabilityV1,
) -> Result<Vec<u8>, ProtocolWriteError> {
    encode_frame(capability)
}

pub fn decode_capability_frame(
    frame: &[u8],
) -> Result<ScoopcProtocolCapabilityV1, ProtocolReadError> {
    let payload = decode_frame_payload(frame).map_err(ProtocolReadError::Frame)?;
    scoop_wire::decode_canonical::<DecodedScoopcProtocolCapabilityV1>(
        payload,
        capability_decode_limits(),
    )
    .map_err(ProtocolReadError::Wire)?
    .validate()
    .map_err(ProtocolReadError::Validation)
}

fn capability_decode_limits() -> DecodeLimits {
    DecodeLimits {
        cbor_nesting: 8,
        semantic_table_entries: 16,
        semantic_leaf_bytes: 1_024,
        semantic_recursion: 8,
        logical_heap_bytes: 4_096,
        decoded_nodes: 64,
        decoded_edges: 0,
        owned_bytes: 1_024,
        validation_work_units: 256,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProtocolFrameError;

    #[test]
    fn current_capability_has_a_canonical_bounded_frame() {
        let current = ScoopcProtocolCapabilityV1::current();
        let frame = encode_capability_frame(&current).unwrap();
        assert_eq!(decode_capability_frame(&frame).unwrap(), current);
        assert!(frame.len() < 256);
    }

    #[test]
    fn capability_frame_rejects_trailing_bytes() {
        let mut frame = encode_capability_frame(&ScoopcProtocolCapabilityV1::current()).unwrap();
        frame.push(0);
        assert!(matches!(
            decode_capability_frame(&frame),
            Err(ProtocolReadError::Frame(
                ProtocolFrameError::LengthMismatch { .. }
            ))
        ));
    }
}
