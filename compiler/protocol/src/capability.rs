use scoop_wire::{Decoder, Digest256, Encoder, WireDecode, WireEncode, sha256};

use crate::framing::{ProtocolReadError, ProtocolWriteError, decode_frame_payload, encode_frame};
use crate::{PROTOCOL_VERSION, ProtocolValidationError};

const CAPABILITY_MAGIC: &str = "scoopc-machine-capability";
const MACHINE_IDENTITY_MAGIC: &str = "scoopc-machine-identity";
const REQUEST_SCHEMA_V2: &[u8] =
    b"scoopc-request-envelope-v2:magic,version,request-id,build-request,ordered-file-dumps";
const RESPONSE_SCHEMA_V2: &[u8] =
    b"scoopc-response-envelope-v2:magic,version,request-id,result,diagnostics,ordered-file-dumps";

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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ScoopcMachineCapabilityV1 {
    protocol: ScoopcProtocolCapabilityV1,
    toolchain_distribution_id: Digest256,
    compiler_build_identity: Digest256,
    identity_abi: Digest256,
}

impl ScoopcMachineCapabilityV1 {
    pub const fn new(
        protocol: ScoopcProtocolCapabilityV1,
        toolchain_distribution_id: Digest256,
        compiler_build_identity: Digest256,
        identity_abi: Digest256,
    ) -> Self {
        Self {
            protocol,
            toolchain_distribution_id,
            compiler_build_identity,
            identity_abi,
        }
    }

    pub const fn protocol(self) -> ScoopcProtocolCapabilityV1 {
        self.protocol
    }

    pub const fn toolchain_distribution_id(self) -> Digest256 {
        self.toolchain_distribution_id
    }

    pub const fn compiler_build_identity(self) -> Digest256 {
        self.compiler_build_identity
    }

    pub const fn identity_abi(self) -> Digest256 {
        self.identity_abi
    }
}

impl WireEncode for ScoopcMachineCapabilityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        encoder.text(MACHINE_IDENTITY_MAGIC)?;
        encoder.field(2)?;
        self.protocol.encode(encoder)?;
        encoder.field(3)?;
        self.toolchain_distribution_id.encode(encoder)?;
        encoder.field(4)?;
        self.compiler_build_identity.encode(encoder)?;
        encoder.field(5)?;
        self.identity_abi.encode(encoder)
    }
}

impl ScoopcProtocolCapabilityV1 {
    pub fn current() -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            request_schema: sha256(REQUEST_SCHEMA_V2),
            response_schema: sha256(RESPONSE_SCHEMA_V2),
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

impl WireDecode for ScoopcProtocolCapabilityV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, scoop_wire::WireError> {
        DecodedScoopcProtocolCapabilityV1::decode(decoder)?
            .validate()
            .map_err(|error| validation_wire_error(decoder, error))
    }
}

impl WireDecode for ScoopcMachineCapabilityV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, scoop_wire::WireError> {
        decoder.expect_map(5)?;
        let magic = decoder.field(1, Decoder::owned_text)?;
        if magic != MACHINE_IDENTITY_MAGIC {
            return Err(validation_wire_error(
                decoder,
                ProtocolValidationError::InvalidCapabilityMagic,
            ));
        }
        let protocol = decoder.field(2, ScoopcProtocolCapabilityV1::decode)?;
        let toolchain_distribution_id = decoder.field(3, Digest256::decode)?;
        let compiler_build_identity = decoder.field(4, Digest256::decode)?;
        let identity_abi = decoder.field(5, Digest256::decode)?;
        Ok(Self {
            protocol,
            toolchain_distribution_id,
            compiler_build_identity,
            identity_abi,
        })
    }
}

fn validation_wire_error(
    decoder: &Decoder<'_>,
    error: ProtocolValidationError,
) -> scoop_wire::WireError {
    let kind = match error {
        ProtocolValidationError::UnsupportedVersion(_) => {
            scoop_wire::WireErrorKind::IntegerOutOfRange
        }
        _ => scoop_wire::WireErrorKind::UnknownTag { tag: u64::MAX },
    };
    scoop_wire::WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
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
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, scoop_wire::WireError> {
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
    decoder: &mut Decoder<'_>,
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
    scoop_wire::decode_canonical::<DecodedScoopcProtocolCapabilityV1>(payload)
        .map_err(ProtocolReadError::Wire)?
        .validate()
        .map_err(ProtocolReadError::Validation)
}

pub fn encode_machine_capability_frame(
    capability: &ScoopcMachineCapabilityV1,
) -> Result<Vec<u8>, ProtocolWriteError> {
    encode_frame(capability)
}

pub fn decode_machine_capability_frame(
    frame: &[u8],
) -> Result<ScoopcMachineCapabilityV1, ProtocolReadError> {
    let payload = decode_frame_payload(frame).map_err(ProtocolReadError::Frame)?;
    scoop_wire::decode_canonical::<ScoopcMachineCapabilityV1>(payload)
        .map_err(ProtocolReadError::Wire)
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

    #[test]
    fn machine_identity_round_trips_all_pairing_dimensions() {
        let identity = ScoopcMachineCapabilityV1::new(
            ScoopcProtocolCapabilityV1::current(),
            sha256(b"distribution"),
            sha256(b"compiler"),
            sha256(b"identity ABI"),
        );
        let frame = encode_machine_capability_frame(&identity).unwrap();
        assert_eq!(decode_machine_capability_frame(&frame).unwrap(), identity);
        assert!(frame.len() < 512);
    }
}
