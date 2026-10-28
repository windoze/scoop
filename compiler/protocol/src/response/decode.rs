use super::product::DecodedProduct;
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
}

impl DecodedDumpDescriptorV1 {
    fn validate(self) -> Result<EmittedDumpDescriptorV1, ProtocolValidationError> {
        Ok(EmittedDumpDescriptorV1::new(
            StageDumpKindV1::from_tag(self.stage)?,
            self.destination.validate()?,
        ))
    }
}

impl WireEncode for DecodedDumpDescriptorV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.unsigned(self.stage)?;
        encoder.field(2)?;
        self.destination.encode(encoder)
    }
}

impl WireDecode for DecodedDumpDescriptorV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let stage = decoder.field(1, Decoder::unsigned)?;
        let destination = decoder.field(2, DecodedDumpDestinationV1::decode)?;
        Ok(Self { stage, destination })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedResponseV1 {
    Observed {
        request_id: Vec<u8>,
        warnings: Vec<DecodedStructuredDiagnosticV1>,
        dumps: Vec<DecodedDumpDescriptorV1>,
    },
    Success {
        request_id: Vec<u8>,
        product: DecodedProduct,
        warnings: Vec<DecodedStructuredDiagnosticV1>,
        dumps: Vec<DecodedDumpDescriptorV1>,
        used_generators: Vec<crate::GeneratorToolInputV1>,
        environment_inputs: Vec<crate::EnvironmentInputV1>,
    },
    Failure {
        request_id: Vec<u8>,
        diagnostics: Vec<DecodedStructuredDiagnosticV1>,
    },
}

impl DecodedResponseV1 {
    fn validate(self) -> Result<ScoopcResponseEnvelopeV1, ProtocolValidationError> {
        match self {
            Self::Observed {
                request_id,
                warnings,
                dumps,
            } => Ok(ScoopcResponseEnvelopeV1::Observed {
                request_id: RequestCorrelationId::from_vec(request_id)?,
                result: ScoopcObservationV1::new(
                    warnings
                        .into_iter()
                        .map(DecodedStructuredDiagnosticV1::validate)
                        .collect::<Result<_, _>>()?,
                    dumps
                        .into_iter()
                        .map(DecodedDumpDescriptorV1::validate)
                        .collect::<Result<_, _>>()?,
                )?,
            }),
            Self::Success {
                request_id,
                product,
                warnings,
                dumps,
                used_generators,
                environment_inputs,
            } => {
                let warnings = warnings
                    .into_iter()
                    .map(DecodedStructuredDiagnosticV1::validate)
                    .collect::<Result<Vec<_>, _>>()?;
                let dumps = dumps
                    .into_iter()
                    .map(DecodedDumpDescriptorV1::validate)
                    .collect::<Result<Vec<_>, _>>()?;
                let result = ScoopcSuccessV1::new(product.validate()?, warnings, dumps)?
                    .with_used_generators(used_generators)
                    .with_environment_inputs(environment_inputs);
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
            Self::Observed {
                request_id,
                warnings,
                dumps,
            } => {
                encoder.map(4)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                encoder.bytes(request_id)?;
                encoder.field(2)?;
                encode_decoded_diagnostics(encoder, warnings)?;
                encoder.field(3)?;
                encoder.array(dumps.len() as u64)?;
                for dump in dumps {
                    dump.encode(encoder)?;
                }
                Ok(())
            }
            Self::Success {
                request_id,
                product,
                warnings,
                dumps,
                used_generators,
                environment_inputs,
            } => {
                encoder.map(7)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                encoder.bytes(request_id)?;
                encoder.field(2)?;
                product.encode(encoder)?;
                encoder.field(3)?;
                encode_decoded_diagnostics(encoder, warnings)?;
                encoder.field(4)?;
                encoder.array(dumps.len() as u64)?;
                for dump in dumps {
                    dump.encode(encoder)?;
                }
                encoder.field(5)?;
                encoder.array(used_generators.len() as u64)?;
                for input in used_generators {
                    input.encode(encoder)?;
                }
                encoder.field(6)?;
                encoder.array(environment_inputs.len() as u64)?;
                for input in environment_inputs {
                    input.encode(encoder)?;
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
            3 => {
                crate::framing::expect_sum_length(decoder, fields, 4)?;
                Ok(Self::Observed {
                    request_id: decoder.field(1, Decoder::owned_bytes)?,
                    warnings: decoder.field(2, decode_decoded_diagnostics)?,
                    dumps: decoder.field(3, |decoder| {
                        decoder.decode_array(|decoder, _| DecodedDumpDescriptorV1::decode(decoder))
                    })?,
                })
            }
            1 => {
                crate::framing::expect_sum_length(decoder, fields, 7)?;
                let request_id = decoder.field(1, Decoder::owned_bytes)?;
                let product = decoder.field(2, DecodedProduct::decode)?;
                let warnings = decoder.field(3, decode_decoded_diagnostics)?;
                let dumps = decoder.field(4, |decoder| {
                    decoder.decode_array(|decoder, _| DecodedDumpDescriptorV1::decode(decoder))
                })?;
                Ok(Self::Success {
                    request_id,
                    product,
                    warnings,
                    dumps,
                    used_generators: decoder.field(5, |d| {
                        d.decode_array(|d, _| crate::GeneratorToolInputV1::decode(d))
                    })?,
                    environment_inputs: decoder.field(6, |d| {
                        d.decode_array(|d, _| crate::EnvironmentInputV1::decode(d))
                    })?,
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
