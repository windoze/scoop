//! Canonical child request decoding, separate from the public request model.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DecodedCurrentConeRequestV1 {
    ExpandedCone(Vec<u8>),
    SingleFile(DecodedHostPathCarrier),
}

impl DecodedCurrentConeRequestV1 {
    fn validate(self) -> Result<CurrentConeRequestV1, ProtocolValidationError> {
        match self {
            Self::ExpandedCone(bytes) => Ok(CurrentConeRequestV1::ExpandedCone {
                config: serde_json::from_slice(&bytes).map_err(|error| {
                    ProtocolValidationError::InvalidExpandedCone(error.to_string())
                })?,
            }),
            Self::SingleFile(source) => Ok(CurrentConeRequestV1::SingleFile {
                source: source
                    .validate()
                    .map_err(ProtocolValidationError::HostPath)?,
            }),
        }
    }
}

impl WireEncode for DecodedCurrentConeRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ExpandedCone(bytes) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                encoder.bytes(bytes)
            }
            Self::SingleFile(source) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                source.encode(encoder)
            }
        }
    }
}

impl WireDecode for DecodedCurrentConeRequestV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            3 => {
                crate::framing::expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, Decoder::owned_bytes)
                    .map(Self::ExpandedCone)
            }
            2 => {
                crate::framing::expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedHostPathCarrier::decode)
                    .map(Self::SingleFile)
            }
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DecodedTrustedCoreRequestV1 {
    ArtifactSlot(DecodedHostPathCarrier),
    Bootstrap,
}

impl DecodedTrustedCoreRequestV1 {
    fn validate(self) -> Result<TrustedCoreRequestV1, ProtocolValidationError> {
        match self {
            Self::ArtifactSlot(artifact) => Ok(TrustedCoreRequestV1::ArtifactSlot {
                artifact: artifact
                    .validate()
                    .map_err(ProtocolValidationError::HostPath)?,
            }),
            Self::Bootstrap => Ok(TrustedCoreRequestV1::Bootstrap),
        }
    }
}

impl WireEncode for DecodedTrustedCoreRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ArtifactSlot(artifact) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                artifact.encode(encoder)
            }
            Self::Bootstrap => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(2)
            }
        }
    }
}

impl WireDecode for DecodedTrustedCoreRequestV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                crate::framing::expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedHostPathCarrier::decode)
                    .map(Self::ArtifactSlot)
            }
            2 => {
                crate::framing::expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Bootstrap)
            }
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DecodedScoopcBuildRequestV1 {
    current: DecodedCurrentConeRequestV1,
    direct_slibs: Vec<DecodedHostPathCarrier>,
    support_slibs: Vec<DecodedHostPathCarrier>,
    trusted_core: DecodedTrustedCoreRequestV1,
    target: DecodedTargetSelectionRequestV1,
    output: DecodedOutputRequest,
    diagnostics: u64,
    emit: DecodedStageDumpPolicyV1,
    optimization: OptimizationMode,
    debug_info: DebugInfoLevel,
    purpose: crate::CompilationPurpose,
    native_inputs: Vec<DecodedHostPathCarrier>,
    generators: Vec<crate::GeneratorRegistrationV1>,
}

impl DecodedScoopcBuildRequestV1 {
    fn validate(self) -> Result<ScoopcBuildRequestV1, ProtocolValidationError> {
        let current = self.current.validate()?;
        let direct_slibs = validate_paths(self.direct_slibs)?;
        let support_slibs = validate_paths(self.support_slibs)?;
        let trusted_core = self.trusted_core.validate()?;
        let target = self.target.validate()?;
        let output = self.output.validate()?;
        let diagnostics = DiagnosticOutputPolicyV1::from_tag(self.diagnostics)?;
        let emit = self.emit.validate()?;
        let native_inputs = validate_paths(self.native_inputs)?;
        ScoopcBuildRequestV1::new(
            current,
            direct_slibs,
            support_slibs,
            trusted_core,
            target,
            output,
            diagnostics,
            emit,
        )
        .and_then(|request| {
            request
                .with_optimization(self.optimization)
                .with_debug_info(self.debug_info)
                .with_native_inputs(native_inputs)
                .with_purpose(self.purpose)
        })
        .and_then(|request| request.with_generators(self.generators))
    }
}

impl WireEncode for DecodedScoopcBuildRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(13)?;
        encoder.field(1)?;
        self.current.encode(encoder)?;
        encoder.field(2)?;
        encode_decoded_paths(encoder, &self.direct_slibs)?;
        encoder.field(3)?;
        encode_decoded_paths(encoder, &self.support_slibs)?;
        encoder.field(4)?;
        self.trusted_core.encode(encoder)?;
        encoder.field(5)?;
        self.target.encode(encoder)?;
        encoder.field(6)?;
        self.output.encode(encoder)?;
        encoder.field(7)?;
        encoder.unsigned(self.diagnostics)?;
        encoder.field(8)?;
        self.emit.encode(encoder)?;
        encoder.field(9)?;
        self.optimization.encode(encoder)?;
        encoder.field(10)?;
        encode_decoded_paths(encoder, &self.native_inputs)?;
        encoder.field(11)?;
        self.purpose.encode(encoder)?;
        encoder.field(12)?;
        encoder.array(self.generators.len() as u64)?;
        for generator in &self.generators {
            generator.encode(encoder)?;
        }
        encoder.field(13)?;
        self.debug_info.encode(encoder)
    }
}

impl WireDecode for DecodedScoopcBuildRequestV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(13)?;
        let current = decoder.field(1, DecodedCurrentConeRequestV1::decode)?;
        let direct_slibs = decoder.field(2, |decoder| {
            decoder.decode_array(|decoder, _| DecodedHostPathCarrier::decode(decoder))
        })?;
        let support_slibs = decoder.field(3, |decoder| {
            decoder.decode_array(|decoder, _| DecodedHostPathCarrier::decode(decoder))
        })?;
        let trusted_core = decoder.field(4, DecodedTrustedCoreRequestV1::decode)?;
        let target = decoder.field(5, DecodedTargetSelectionRequestV1::decode)?;
        let output = decoder.field(6, DecodedOutputRequest::decode)?;
        let diagnostics = decoder.field(7, Decoder::unsigned)?;
        let emit = decoder.field(8, DecodedStageDumpPolicyV1::decode)?;
        let optimization = decoder.field(9, OptimizationMode::decode)?;
        let native_inputs = decoder.field(10, |decoder| {
            decoder.decode_array(|decoder, _| DecodedHostPathCarrier::decode(decoder))
        })?;
        Ok(Self {
            current,
            direct_slibs,
            support_slibs,
            trusted_core,
            target,
            output,
            diagnostics,
            emit,
            optimization,
            native_inputs,
            purpose: decoder.field(11, crate::CompilationPurpose::decode)?,
            generators: decoder.field(12, |d| {
                d.decode_array(|d, _| crate::GeneratorRegistrationV1::decode(d))
            })?,
            debug_info: decoder.field(13, DebugInfoLevel::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodedScoopcRequestEnvelopeV1 {
    magic: Vec<u8>,
    version: u32,
    request_id: Vec<u8>,
    build: DecodedScoopcBuildRequestV1,
    observation_only: bool,
}

impl DecodedScoopcRequestEnvelopeV1 {
    pub(crate) fn validate(self) -> Result<ScoopcRequestEnvelopeV1, ProtocolValidationError> {
        if self.magic.as_slice() != REQUEST_MAGIC {
            return Err(ProtocolValidationError::InvalidRequestMagic);
        }
        if self.version != PROTOCOL_VERSION {
            return Err(ProtocolValidationError::UnsupportedVersion(self.version));
        }
        let request = ScoopcRequestEnvelopeV1::new(
            RequestCorrelationId::from_vec(self.request_id)?,
            self.build.validate()?,
        );
        if self.observation_only {
            let StageDumpPolicyV1::Files { stages, .. } = request.build().emit() else {
                return Err(ProtocolValidationError::InvalidDumpStages);
            };
            let stages = *stages;
            request.observe(stages)
        } else {
            Ok(request)
        }
    }
}

impl WireEncode for DecodedScoopcRequestEnvelopeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        encoder.bytes(&self.magic)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.version))?;
        encoder.field(3)?;
        encoder.bytes(&self.request_id)?;
        encoder.field(4)?;
        self.build.encode(encoder)?;
        encoder.field(5)?;
        encoder.unsigned(u64::from(self.observation_only))
    }
}

impl WireDecode for DecodedScoopcRequestEnvelopeV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        let magic = decoder.field(1, Decoder::owned_bytes)?;
        let version = decoder.field(2, Decoder::u32)?;
        let request_id = decoder.field(3, Decoder::owned_bytes)?;
        let build = decoder.field(4, DecodedScoopcBuildRequestV1::decode)?;
        let observation_only = decoder.field(5, |decoder| match decoder.unsigned()? {
            0 => Ok(false),
            1 => Ok(true),
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        })?;
        Ok(Self {
            magic,
            version,
            request_id,
            build,
            observation_only,
        })
    }
}
