use scoop_wire::{Encoder, WireEncode};

mod decode;
pub(crate) use decode::DecodedScoopcResponseEnvelopeV1;

use crate::{
    HostPathCarrier, MAX_EMITTED_DUMPS, PROTOCOL_VERSION, ProtocolValidationError, StageDumpKindV1,
    StructuredDiagnosticV1,
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
        if warnings.iter().any(|warning| warning.severity().is_error()) {
            return Err(ProtocolValidationError::SuccessContainsErrorDiagnostic);
        }
        if emitted_dump_descriptors.len() > MAX_EMITTED_DUMPS {
            return Err(ProtocolValidationError::TooManyEmittedDumps(
                emitted_dump_descriptors.len(),
            ));
        }
        if !emitted_dump_descriptors
            .windows(2)
            .all(|pair| pair[0].stage() < pair[1].stage())
            || emitted_dump_descriptors
                .iter()
                .any(|dump| matches!(dump.destination(), EmittedDumpDestinationV1::Stdout))
        {
            return Err(ProtocolValidationError::InvalidDumpDescriptors);
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

fn validate_failure_diagnostics(
    diagnostics: &[StructuredDiagnosticV1],
) -> Result<(), ProtocolValidationError> {
    if diagnostics.is_empty() {
        return Err(ProtocolValidationError::FailureRequiresDiagnostic);
    }

    if !diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity().is_error())
    {
        return Err(ProtocolValidationError::FailureRequiresErrorDiagnostic);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
