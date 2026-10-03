use super::*;
use crate::diagnostic::DecodedStructuredDiagnosticV1;
use crate::path::DecodedHostPathCarrier;
use scoop_wire::{Decoder, WireDecode, WireError};

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedDumpDestinationV1 {
    Stdout,
    File(DecodedHostPathCarrier),
}

impl DecodedDumpDestinationV1 {
    fn validate(self) -> Result<EmittedDumpDestinationV1, ProtocolValidationError> {
        match self {
            Self::Stdout => Ok(EmittedDumpDestinationV1::Stdout),
            Self::File(path) => Ok(EmittedDumpDestinationV1::File(
                path.validate().map_err(ProtocolValidationError::HostPath)?,
            )),
        }
    }
}

impl WireEncode for DecodedDumpDestinationV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Stdout => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::File(path) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                path.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedDumpDestinationV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                crate::framing::expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Stdout)
            }
            2 => {
                crate::framing::expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedHostPathCarrier::decode)
                    .map(Self::File)
            }
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedDumpDescriptorV1 {
    stage: u64,
    destination: DecodedDumpDestinationV1,
    digest: Vec<u8>,
}

impl DecodedDumpDescriptorV1 {
    fn validate(self) -> Result<EmittedDumpDescriptorV1, ProtocolValidationError> {
        Ok(EmittedDumpDescriptorV1::new(
            decode_stage(self.stage)?,
            self.destination.validate()?,
            ProtocolDumpContentDigest::from_vec(self.digest)?,
        ))
    }
}

impl WireEncode for DecodedDumpDescriptorV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.unsigned(self.stage)?;
        encoder.field(2)?;
        self.destination.encode(encoder)?;
        encoder.field(3)?;
        encoder.bytes(&self.digest)
    }
}

impl WireDecode for DecodedDumpDescriptorV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        let stage = decoder.field(1, Decoder::unsigned)?;
        let destination = decoder.field(2, DecodedDumpDestinationV1::decode)?;
        let digest = decoder.field(3, Decoder::owned_bytes)?;
        Ok(Self {
            stage,
            destination,
            digest,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedResponseV1 {
    Success {
        request_id: Vec<u8>,
        artifact_fingerprint: Vec<u8>,
        cone_identity: Vec<u8>,
        hir_fingerprint: Vec<u8>,
        mir_fingerprint: Vec<u8>,
        lir_fingerprint: Vec<u8>,
        code_fingerprint: Vec<u8>,
        runtime_image_fingerprint: Vec<u8>,
        warnings: Vec<DecodedStructuredDiagnosticV1>,
        dumps: Vec<DecodedDumpDescriptorV1>,
    },
    Failure {
        request_id: Vec<u8>,
        diagnostics: Vec<DecodedStructuredDiagnosticV1>,
    },
}

impl DecodedResponseV1 {
    fn validate(self) -> Result<ScoopcResponseEnvelopeV1, ProtocolValidationError> {
        match self {
            Self::Success {
                request_id,
                artifact_fingerprint,
                cone_identity,
                hir_fingerprint,
                mir_fingerprint,
                lir_fingerprint,
                code_fingerprint,
                runtime_image_fingerprint,
                warnings,
                dumps,
            } => {
                let warnings = warnings
                    .into_iter()
                    .map(DecodedStructuredDiagnosticV1::validate)
                    .collect::<Result<Vec<_>, _>>()?;
                let dumps = dumps
                    .into_iter()
                    .map(DecodedDumpDescriptorV1::validate)
                    .collect::<Result<Vec<_>, _>>()?;
                let result = ScoopcSuccessV1::new(
                    ProtocolArtifactFingerprint::from_vec(artifact_fingerprint)?,
                    ProtocolConeIdentity::from_vec(cone_identity)?,
                    ProtocolHirFingerprint::from_vec(hir_fingerprint)?,
                    ProtocolMirFingerprint::from_vec(mir_fingerprint)?,
                    ProtocolLirFingerprint::from_vec(lir_fingerprint)?,
                    ProtocolCodeFingerprint::from_vec(code_fingerprint)?,
                    ProtocolRuntimeImageFingerprint::from_vec(runtime_image_fingerprint)?,
                    warnings,
                    dumps,
                )?;
                Ok(ScoopcResponseEnvelopeV1::success(
                    RequestCorrelationId::from_vec(request_id)?,
                    result,
                ))
            }
            Self::Failure {
                request_id,
                diagnostics,
            } => {
                let diagnostics = diagnostics
                    .into_iter()
                    .map(DecodedStructuredDiagnosticV1::validate)
                    .collect::<Result<Vec<_>, _>>()?;
                ScoopcResponseEnvelopeV1::failure(
                    RequestCorrelationId::from_vec(request_id)?,
                    diagnostics,
                )
            }
        }
    }
}

impl WireEncode for DecodedResponseV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Success {
                request_id,
                artifact_fingerprint,
                cone_identity,
                hir_fingerprint,
                mir_fingerprint,
                lir_fingerprint,
                code_fingerprint,
                runtime_image_fingerprint,
                warnings,
                dumps,
            } => {
                encoder.map(11)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                for (field, bytes) in [
                    (1, request_id),
                    (2, artifact_fingerprint),
                    (3, cone_identity),
                    (4, hir_fingerprint),
                    (5, mir_fingerprint),
                    (6, lir_fingerprint),
                    (7, code_fingerprint),
                    (8, runtime_image_fingerprint),
                ] {
                    encoder.field(field)?;
                    encoder.bytes(bytes)?;
                }
                encoder.field(9)?;
                encode_decoded_diagnostics(encoder, warnings)?;
                encoder.field(10)?;
                encoder.array(dumps.len() as u64)?;
                for dump in dumps {
                    dump.encode(encoder)?;
                }
                Ok(())
            }
            Self::Failure {
                request_id,
                diagnostics,
            } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                encoder.bytes(request_id)?;
                encoder.field(2)?;
                encode_decoded_diagnostics(encoder, diagnostics)
            }
        }
    }
}

impl WireDecode for DecodedResponseV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                crate::framing::expect_sum_length(decoder, fields, 11)?;
                let request_id = decoder.field(1, Decoder::owned_bytes)?;
                let artifact_fingerprint = decoder.field(2, Decoder::owned_bytes)?;
                let cone_identity = decoder.field(3, Decoder::owned_bytes)?;
                let hir_fingerprint = decoder.field(4, Decoder::owned_bytes)?;
                let mir_fingerprint = decoder.field(5, Decoder::owned_bytes)?;
                let lir_fingerprint = decoder.field(6, Decoder::owned_bytes)?;
                let code_fingerprint = decoder.field(7, Decoder::owned_bytes)?;
                let runtime_image_fingerprint = decoder.field(8, Decoder::owned_bytes)?;
                let warnings = decoder.field(9, decode_decoded_diagnostics)?;
                let dumps = decoder.field(10, |decoder| {
                    decoder.decode_array(|decoder, _| DecodedDumpDescriptorV1::decode(decoder))
                })?;
                Ok(Self::Success {
                    request_id,
                    artifact_fingerprint,
                    cone_identity,
                    hir_fingerprint,
                    mir_fingerprint,
                    lir_fingerprint,
                    code_fingerprint,
                    runtime_image_fingerprint,
                    warnings,
                    dumps,
                })
            }
            2 => {
                crate::framing::expect_sum_length(decoder, fields, 3)?;
                let request_id = decoder.field(1, Decoder::owned_bytes)?;
                let diagnostics = decoder.field(2, decode_decoded_diagnostics)?;
                Ok(Self::Failure {
                    request_id,
                    diagnostics,
                })
            }
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        }
    }
}

fn encode_decoded_diagnostics(
    encoder: &mut Encoder,
    diagnostics: &[DecodedStructuredDiagnosticV1],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(diagnostics.len() as u64)?;
    for diagnostic in diagnostics {
        diagnostic.encode(encoder)?;
    }
    Ok(())
}

fn decode_decoded_diagnostics(
    decoder: &mut Decoder<'_>,
) -> Result<Vec<DecodedStructuredDiagnosticV1>, WireError> {
    decoder.decode_array(|decoder, _| DecodedStructuredDiagnosticV1::decode(decoder))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodedScoopcResponseEnvelopeV1 {
    magic: Vec<u8>,
    version: u32,
    response: DecodedResponseV1,
}

impl DecodedScoopcResponseEnvelopeV1 {
    pub(crate) fn validate(self) -> Result<ScoopcResponseEnvelopeV1, ProtocolValidationError> {
        if self.magic.as_slice() != RESPONSE_MAGIC {
            return Err(ProtocolValidationError::InvalidResponseMagic);
        }
        if self.version != PROTOCOL_VERSION {
            return Err(ProtocolValidationError::UnsupportedVersion(self.version));
        }
        self.response.validate()
    }
}

impl WireEncode for DecodedScoopcResponseEnvelopeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.bytes(&self.magic)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.version))?;
        encoder.field(3)?;
        self.response.encode(encoder)
    }
}

impl WireDecode for DecodedScoopcResponseEnvelopeV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        let magic = decoder.field(1, Decoder::owned_bytes)?;
        let version = decoder.field(2, Decoder::u32)?;
        let response = decoder.field(3, DecodedResponseV1::decode)?;
        Ok(Self {
            magic,
            version,
            response,
        })
    }
}

fn decode_stage(tag: u64) -> Result<StageDumpKindV1, ProtocolValidationError> {
    match tag {
        1 => Ok(StageDumpKindV1::Ast),
        2 => Ok(StageDumpKindV1::Hir),
        3 => Ok(StageDumpKindV1::Mir),
        4 => Ok(StageDumpKindV1::Lir),
        _ => Err(ProtocolValidationError::UnknownEnumTag {
            kind: "stage dump",
            tag,
        }),
    }
}
