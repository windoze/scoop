use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::diagnostic::DecodedStructuredDiagnosticV1;
use crate::path::DecodedHostPathCarrier;
use crate::{
    HostPathCarrier, MAX_DIAGNOSTICS, MAX_EMITTED_DUMPS, PROTOCOL_VERSION, ProtocolValidationError,
    StageDumpKindV1, StructuredDiagnosticV1,
};

const RESPONSE_MAGIC: &[u8; 8] = b"SCOOPRES";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RequestCorrelationId([u8; 16]);

impl RequestCorrelationId {
    pub const fn from_array(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_array(&self) -> &[u8; 16] {
        &self.0
    }

    pub(crate) fn from_vec(bytes: Vec<u8>) -> Result<Self, ProtocolValidationError> {
        let actual = bytes.len();
        let bytes = bytes
            .try_into()
            .map_err(|_| ProtocolValidationError::InvalidRequestIdLength(actual))?;
        Ok(Self(bytes))
    }
}

impl WireEncode for RequestCorrelationId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.bytes(&self.0)
    }
}

macro_rules! protocol_digest {
    ($name:ident, $field:literal) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_array(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn as_array(&self) -> &[u8; 32] {
                &self.0
            }

            pub(crate) fn from_vec(bytes: Vec<u8>) -> Result<Self, ProtocolValidationError> {
                let actual = bytes.len();
                let bytes =
                    bytes
                        .try_into()
                        .map_err(|_| ProtocolValidationError::InvalidDigestLength {
                            field: $field,
                            actual,
                        })?;
                Ok(Self(bytes))
            }
        }

        impl WireEncode for $name {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.bytes(&self.0)
            }
        }
    };
}

protocol_digest!(ProtocolArtifactFingerprint, "artifact fingerprint");
protocol_digest!(ProtocolConeIdentity, "Cone identity");
protocol_digest!(ProtocolHirFingerprint, "HIR fingerprint");
protocol_digest!(ProtocolMirFingerprint, "MIR fingerprint");
protocol_digest!(ProtocolLirFingerprint, "LIR fingerprint");
protocol_digest!(ProtocolCodeFingerprint, "Code fingerprint");
protocol_digest!(ProtocolDumpContentDigest, "dump content digest");
protocol_digest!(ProtocolRuntimeImageFingerprint, "runtime image fingerprint");

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmittedDumpDestinationV1 {
    Stdout,
    File(HostPathCarrier),
}

impl WireEncode for EmittedDumpDestinationV1 {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmittedDumpDescriptorV1 {
    stage: StageDumpKindV1,
    destination: EmittedDumpDestinationV1,
    content_digest: ProtocolDumpContentDigest,
}

impl EmittedDumpDescriptorV1 {
    pub const fn new(
        stage: StageDumpKindV1,
        destination: EmittedDumpDestinationV1,
        content_digest: ProtocolDumpContentDigest,
    ) -> Self {
        Self {
            stage,
            destination,
            content_digest,
        }
    }

    pub const fn stage(&self) -> StageDumpKindV1 {
        self.stage
    }

    pub const fn destination(&self) -> &EmittedDumpDestinationV1 {
        &self.destination
    }

    pub const fn content_digest(&self) -> ProtocolDumpContentDigest {
        self.content_digest
    }
}

impl WireEncode for EmittedDumpDescriptorV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.stage.encode(encoder)?;
        encoder.field(2)?;
        self.destination.encode(encoder)?;
        encoder.field(3)?;
        self.content_digest.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoopcSuccessV1 {
    artifact_fingerprint: ProtocolArtifactFingerprint,
    cone_identity: ProtocolConeIdentity,
    hir_fingerprint: ProtocolHirFingerprint,
    mir_fingerprint: ProtocolMirFingerprint,
    lir_fingerprint: ProtocolLirFingerprint,
    code_fingerprint: ProtocolCodeFingerprint,
    runtime_image_fingerprint: ProtocolRuntimeImageFingerprint,
    warnings: Vec<StructuredDiagnosticV1>,
    emitted_dump_descriptors: Vec<EmittedDumpDescriptorV1>,
}

impl ScoopcSuccessV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        artifact_fingerprint: ProtocolArtifactFingerprint,
        cone_identity: ProtocolConeIdentity,
        hir_fingerprint: ProtocolHirFingerprint,
        mir_fingerprint: ProtocolMirFingerprint,
        lir_fingerprint: ProtocolLirFingerprint,
        code_fingerprint: ProtocolCodeFingerprint,
        runtime_image_fingerprint: ProtocolRuntimeImageFingerprint,
        warnings: Vec<StructuredDiagnosticV1>,
        emitted_dump_descriptors: Vec<EmittedDumpDescriptorV1>,
    ) -> Result<Self, ProtocolValidationError> {
        if warnings.len() > MAX_DIAGNOSTICS {
            return Err(ProtocolValidationError::TooManyDiagnostics(warnings.len()));
        }
        if warnings.iter().any(|warning| warning.severity().is_error()) {
            return Err(ProtocolValidationError::SuccessContainsErrorDiagnostic);
        }
        if emitted_dump_descriptors.len() > MAX_EMITTED_DUMPS {
            return Err(ProtocolValidationError::TooManyEmittedDumps(
                emitted_dump_descriptors.len(),
            ));
        }
        Ok(Self {
            artifact_fingerprint,
            cone_identity,
            hir_fingerprint,
            mir_fingerprint,
            lir_fingerprint,
            code_fingerprint,
            runtime_image_fingerprint,
            warnings,
            emitted_dump_descriptors,
        })
    }

    pub const fn artifact_fingerprint(&self) -> ProtocolArtifactFingerprint {
        self.artifact_fingerprint
    }

    pub const fn cone_identity(&self) -> ProtocolConeIdentity {
        self.cone_identity
    }

    pub const fn hir_fingerprint(&self) -> ProtocolHirFingerprint {
        self.hir_fingerprint
    }

    pub const fn mir_fingerprint(&self) -> ProtocolMirFingerprint {
        self.mir_fingerprint
    }

    pub const fn lir_fingerprint(&self) -> ProtocolLirFingerprint {
        self.lir_fingerprint
    }

    pub const fn code_fingerprint(&self) -> ProtocolCodeFingerprint {
        self.code_fingerprint
    }

    pub const fn runtime_image_fingerprint(&self) -> ProtocolRuntimeImageFingerprint {
        self.runtime_image_fingerprint
    }

    pub fn warnings(&self) -> &[StructuredDiagnosticV1] {
        &self.warnings
    }

    pub fn emitted_dump_descriptors(&self) -> &[EmittedDumpDescriptorV1] {
        &self.emitted_dump_descriptors
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoopcResponseEnvelopeV1 {
    Success {
        request_id: RequestCorrelationId,
        result: Box<ScoopcSuccessV1>,
    },
    Failure {
        request_id: RequestCorrelationId,
        diagnostics: Vec<StructuredDiagnosticV1>,
    },
}

impl ScoopcResponseEnvelopeV1 {
    pub fn success(request_id: RequestCorrelationId, result: ScoopcSuccessV1) -> Self {
        Self::Success {
            request_id,
            result: Box::new(result),
        }
    }

    pub fn failure(
        request_id: RequestCorrelationId,
        diagnostics: Vec<StructuredDiagnosticV1>,
    ) -> Result<Self, ProtocolValidationError> {
        validate_failure_diagnostics(&diagnostics)?;
        Ok(Self::Failure {
            request_id,
            diagnostics,
        })
    }

    pub const fn request_id(&self) -> RequestCorrelationId {
        match self {
            Self::Success { request_id, .. } | Self::Failure { request_id, .. } => *request_id,
        }
    }
}

impl WireEncode for ScoopcResponseEnvelopeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.bytes(RESPONSE_MAGIC)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(PROTOCOL_VERSION))?;
        encoder.field(3)?;
        match self {
            Self::Success { request_id, result } => encode_success(encoder, *request_id, result),
            Self::Failure {
                request_id,
                diagnostics,
            } => encode_failure(encoder, *request_id, diagnostics),
        }
    }
}

fn encode_success(
    encoder: &mut Encoder,
    request_id: RequestCorrelationId,
    result: &ScoopcSuccessV1,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(11)?;
    encoder.field(0)?;
    encoder.unsigned(1)?;
    encoder.field(1)?;
    request_id.encode(encoder)?;
    encoder.field(2)?;
    result.artifact_fingerprint.encode(encoder)?;
    encoder.field(3)?;
    result.cone_identity.encode(encoder)?;
    encoder.field(4)?;
    result.hir_fingerprint.encode(encoder)?;
    encoder.field(5)?;
    result.mir_fingerprint.encode(encoder)?;
    encoder.field(6)?;
    result.lir_fingerprint.encode(encoder)?;
    encoder.field(7)?;
    result.code_fingerprint.encode(encoder)?;
    encoder.field(8)?;
    result.runtime_image_fingerprint.encode(encoder)?;
    encoder.field(9)?;
    encode_diagnostics(encoder, &result.warnings)?;
    encoder.field(10)?;
    encoder.array(result.emitted_dump_descriptors.len() as u64)?;
    for dump in &result.emitted_dump_descriptors {
        dump.encode(encoder)?;
    }
    Ok(())
}

fn encode_failure(
    encoder: &mut Encoder,
    request_id: RequestCorrelationId,
    diagnostics: &[StructuredDiagnosticV1],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(0)?;
    encoder.unsigned(2)?;
    encoder.field(1)?;
    request_id.encode(encoder)?;
    encoder.field(2)?;
    encode_diagnostics(encoder, diagnostics)
}

fn encode_diagnostics(
    encoder: &mut Encoder,
    diagnostics: &[StructuredDiagnosticV1],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(diagnostics.len() as u64)?;
    for diagnostic in diagnostics {
        diagnostic.encode(encoder)?;
    }
    Ok(())
}

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
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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
    decoder: &mut Decoder<'_, '_>,
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
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
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

fn validate_failure_diagnostics(
    diagnostics: &[StructuredDiagnosticV1],
) -> Result<(), ProtocolValidationError> {
    if diagnostics.is_empty() {
        return Err(ProtocolValidationError::FailureRequiresDiagnostic);
    }
    if diagnostics.len() > MAX_DIAGNOSTICS {
        return Err(ProtocolValidationError::TooManyDiagnostics(
            diagnostics.len(),
        ));
    }
    if !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity().is_error())
    {
        return Err(ProtocolValidationError::FailureRequiresErrorDiagnostic);
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DiagnosticOriginV1, DiagnosticSeverityV1, ProtocolWriteError};

    fn diagnostic(severity: DiagnosticSeverityV1, message: String) -> StructuredDiagnosticV1 {
        StructuredDiagnosticV1::new(
            severity,
            "SCOOPC_TEST_DIAGNOSTIC".to_owned(),
            message,
            DiagnosticOriginV1::None,
            Vec::new(),
        )
        .unwrap()
    }

    fn success(
        warnings: Vec<StructuredDiagnosticV1>,
        dumps: Vec<EmittedDumpDescriptorV1>,
    ) -> Result<ScoopcSuccessV1, ProtocolValidationError> {
        ScoopcSuccessV1::new(
            ProtocolArtifactFingerprint::from_array([1; 32]),
            ProtocolConeIdentity::from_array([2; 32]),
            ProtocolHirFingerprint::from_array([3; 32]),
            ProtocolMirFingerprint::from_array([4; 32]),
            ProtocolLirFingerprint::from_array([5; 32]),
            ProtocolCodeFingerprint::from_array([6; 32]),
            ProtocolRuntimeImageFingerprint::from_array([7; 32]),
            warnings,
            dumps,
        )
    }

    fn dump() -> EmittedDumpDescriptorV1 {
        EmittedDumpDescriptorV1::new(
            StageDumpKindV1::Hir,
            EmittedDumpDestinationV1::Stdout,
            ProtocolDumpContentDigest::from_array([8; 32]),
        )
    }

    #[test]
    fn response_constructors_enforce_diagnostic_and_dump_shape() {
        let error = diagnostic(DiagnosticSeverityV1::Error, "error".to_owned());
        assert_eq!(
            success(vec![error], Vec::new()).unwrap_err(),
            ProtocolValidationError::SuccessContainsErrorDiagnostic
        );

        assert_eq!(
            success(Vec::new(), vec![dump(), dump()]).unwrap_err(),
            ProtocolValidationError::TooManyEmittedDumps(2)
        );

        let warning = diagnostic(DiagnosticSeverityV1::Warning, "warning".to_owned());
        assert_eq!(
            ScoopcResponseEnvelopeV1::failure(
                RequestCorrelationId::from_array([0; 16]),
                vec![warning],
            )
            .unwrap_err(),
            ProtocolValidationError::FailureRequiresErrorDiagnostic
        );
    }

    #[test]
    fn frame_writer_rejects_oversize_response_before_payload_encoding() {
        let message = "x".repeat(crate::MAX_DIAGNOSTIC_TEXT_BYTES);
        let warning = diagnostic(DiagnosticSeverityV1::Warning, message);
        let result = success(vec![warning; 17], Vec::new()).unwrap();
        let response =
            ScoopcResponseEnvelopeV1::success(RequestCorrelationId::from_array([0; 16]), result);
        assert!(matches!(
            crate::encode_response_frame(&response),
            Err(ProtocolWriteError::Frame(
                crate::ProtocolFrameError::PayloadTooLarge { .. }
            ))
        ));
    }
}
