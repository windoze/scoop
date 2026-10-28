use scoop_wire::{Encoder, WireEncode};

mod decode;
mod product;
pub(crate) use decode::DecodedScoopcResponseEnvelopeV1;
pub use product::{ScoopcDocumentationOutputV1, ScoopcProductV1};

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

protocol_digest!(ProtocolConeIdentity, "Cone identity");

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
}

impl EmittedDumpDescriptorV1 {
    pub const fn new(stage: StageDumpKindV1, destination: EmittedDumpDestinationV1) -> Self {
        Self { stage, destination }
    }

    pub const fn stage(&self) -> StageDumpKindV1 {
        self.stage
    }

    pub const fn destination(&self) -> &EmittedDumpDestinationV1 {
        &self.destination
    }
}

impl WireEncode for EmittedDumpDescriptorV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.stage.encode(encoder)?;
        encoder.field(2)?;
        self.destination.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoopcSuccessV1 {
    product: ScoopcProductV1,
    warnings: Vec<StructuredDiagnosticV1>,
    emitted_dump_descriptors: Vec<EmittedDumpDescriptorV1>,
    used_generators: Vec<crate::GeneratorToolInputV1>,
    compiler_inputs: Vec<crate::CompilerInputV1>,
}

impl ScoopcSuccessV1 {
    pub fn new(
        product: impl Into<ScoopcProductV1>,
        warnings: Vec<StructuredDiagnosticV1>,
        emitted_dump_descriptors: Vec<EmittedDumpDescriptorV1>,
    ) -> Result<Self, ProtocolValidationError> {
        let product = product.into();
        if matches!(product, ScoopcProductV1::Check) && !emitted_dump_descriptors.is_empty() {
            return Err(ProtocolValidationError::InvalidDumpDescriptors);
        }
        validate_success_fields(&warnings, &emitted_dump_descriptors)?;
        Ok(Self {
            product,
            warnings,
            emitted_dump_descriptors,
            used_generators: Vec::new(),
            compiler_inputs: Vec::new(),
        })
    }

    pub fn with_used_generators(mut self, inputs: Vec<crate::GeneratorToolInputV1>) -> Self {
        self.used_generators = inputs;
        self
    }

    pub fn with_compiler_inputs(mut self, inputs: Vec<crate::CompilerInputV1>) -> Self {
        self.compiler_inputs = inputs;
        self
    }

    pub fn compiler_inputs(&self) -> &[crate::CompilerInputV1] {
        &self.compiler_inputs
    }

    pub fn used_generators(&self) -> &[crate::GeneratorToolInputV1] {
        &self.used_generators
    }

    pub fn summary(&self) -> Option<&scoop_slib::ArtifactManifestSummaryV1> {
        self.product.summary()
    }

    pub fn documentation(&self) -> Option<&ScoopcDocumentationOutputV1> {
        self.product.documentation()
    }

    pub fn product(&self) -> &ScoopcProductV1 {
        &self.product
    }

    pub fn warnings(&self) -> &[StructuredDiagnosticV1] {
        &self.warnings
    }

    pub fn emitted_dump_descriptors(&self) -> &[EmittedDumpDescriptorV1] {
        &self.emitted_dump_descriptors
    }
}

fn validate_success_fields(
    warnings: &[StructuredDiagnosticV1],
    emitted_dump_descriptors: &[EmittedDumpDescriptorV1],
) -> Result<(), ProtocolValidationError> {
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
    Ok(())
}

mod observation;
pub use observation::ScoopcObservationV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoopcResponseEnvelopeV1 {
    Observed {
        request_id: RequestCorrelationId,
        result: ScoopcObservationV1,
    },
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
            Self::Success { request_id, .. }
            | Self::Observed { request_id, .. }
            | Self::Failure { request_id, .. } => *request_id,
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
            Self::Observed { request_id, result } => {
                observation::encode_observation(encoder, *request_id, result)
            }
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
    encoder.map(7)?;
    encoder.field(0)?;
    encoder.unsigned(1)?;
    encoder.field(1)?;
    request_id.encode(encoder)?;
    encoder.field(2)?;
    result.product.encode(encoder)?;
    encoder.field(3)?;
    encode_diagnostics(encoder, &result.warnings)?;
    encoder.field(4)?;
    encoder.array(result.emitted_dump_descriptors.len() as u64)?;
    for dump in &result.emitted_dump_descriptors {
        dump.encode(encoder)?;
    }
    encoder.field(5)?;
    encoder.array(result.used_generators.len() as u64)?;
    for input in &result.used_generators {
        input.encode(encoder)?;
    }
    encoder.field(6)?;
    encoder.array(result.compiler_inputs.len() as u64)?;
    for input in &result.compiler_inputs {
        input.encode(encoder)?;
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
