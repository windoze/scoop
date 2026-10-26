use super::*;
use crate::DecodedHostPathCarrier;
use crate::diagnostic::DecodedStructuredDiagnosticV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::response) enum DecodedProduct {
    Check,
    Artifact(Box<scoop_slib::ArtifactManifestSummaryV1>),
    Documentation(DecodedDocumentation),
    Both(
        Box<scoop_slib::ArtifactManifestSummaryV1>,
        DecodedDocumentation,
    ),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::response) struct DecodedDocumentation {
    input: DecodedHostPathCarrier,
    warnings: Vec<DecodedStructuredDiagnosticV1>,
}

impl DecodedDocumentation {
    fn validate(self) -> Result<ScoopcDocumentationOutputV1, ProtocolValidationError> {
        ScoopcDocumentationOutputV1::new(
            self.input
                .validate()
                .map_err(ProtocolValidationError::HostPath)?,
            self.warnings
                .into_iter()
                .map(DecodedStructuredDiagnosticV1::validate)
                .collect::<Result<_, _>>()?,
        )
    }
}

impl WireEncode for DecodedDocumentation {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.input.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.warnings.len() as u64)?;
        for warning in &self.warnings {
            warning.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedDocumentation {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        Ok(Self {
            input: decoder.field(1, DecodedHostPathCarrier::decode)?,
            warnings: decoder.field(2, |decoder| {
                decoder.decode_array(|decoder, _| DecodedStructuredDiagnosticV1::decode(decoder))
            })?,
        })
    }
}

impl DecodedProduct {
    pub(in crate::response) fn validate(self) -> Result<ScoopcProductV1, ProtocolValidationError> {
        Ok(match self {
            Self::Check => ScoopcProductV1::Check,
            Self::Artifact(summary) => ScoopcProductV1::Artifact(summary),
            Self::Documentation(output) => ScoopcProductV1::Documentation(output.validate()?),
            Self::Both(artifact, documentation) => ScoopcProductV1::ArtifactWithDocumentation {
                artifact,
                documentation: documentation.validate()?,
            },
        })
    }
}

impl WireDecode for DecodedProduct {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        crate::framing::expect_sum_length(
            decoder,
            fields,
            match tag {
                3 => 3,
                4 => 1,
                _ => 2,
            },
        )?;
        match tag {
            4 => Ok(Self::Check),
            1 => Ok(Self::Artifact(Box::new(
                decoder.field(1, scoop_slib::ArtifactManifestSummaryV1::decode)?,
            ))),
            2 => Ok(Self::Documentation(
                decoder.field(1, DecodedDocumentation::decode)?,
            )),
            3 => Ok(Self::Both(
                Box::new(decoder.field(1, scoop_slib::ArtifactManifestSummaryV1::decode)?),
                decoder.field(2, DecodedDocumentation::decode)?,
            )),
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        }
    }
}

impl WireEncode for DecodedProduct {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let tag = match self {
            Self::Check => 4,
            Self::Artifact(_) => 1,
            Self::Documentation(_) => 2,
            Self::Both(..) => 3,
        };
        encoder.map(match tag {
            3 => 3,
            4 => 1,
            _ => 2,
        })?;
        encoder.field(0)?;
        encoder.unsigned(tag)?;
        if tag == 4 {
            return Ok(());
        }
        encoder.field(1)?;
        match self {
            Self::Check => unreachable!("check has no product payload"),
            Self::Artifact(summary) => summary.encode(encoder)?,
            Self::Documentation(output) => output.encode(encoder)?,
            Self::Both(artifact, documentation) => {
                artifact.encode(encoder)?;
                encoder.field(2)?;
                documentation.encode(encoder)?;
            }
        }
        Ok(())
    }
}
