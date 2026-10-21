use super::*;
use scoop_wire::{Decoder, WireDecode, WireError};

mod decode;
pub(super) use decode::DecodedProduct;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoopcDocumentationOutputV1 {
    input: HostPathCarrier,
    warnings: Vec<StructuredDiagnosticV1>,
}

impl ScoopcDocumentationOutputV1 {
    pub fn new(
        input: HostPathCarrier,
        warnings: Vec<StructuredDiagnosticV1>,
    ) -> Result<Self, ProtocolValidationError> {
        validate_success_fields(&warnings, &[])?;
        Ok(Self { input, warnings })
    }

    pub fn input(&self) -> &HostPathCarrier {
        &self.input
    }

    pub fn warnings(&self) -> &[StructuredDiagnosticV1] {
        &self.warnings
    }
}

impl WireEncode for ScoopcDocumentationOutputV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.input.encode(encoder)?;
        encoder.field(2)?;
        encode_diagnostics(encoder, &self.warnings)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoopcProductV1 {
    Artifact(Box<scoop_slib::ArtifactManifestSummaryV1>),
    Documentation(ScoopcDocumentationOutputV1),
    ArtifactWithDocumentation {
        artifact: Box<scoop_slib::ArtifactManifestSummaryV1>,
        documentation: ScoopcDocumentationOutputV1,
    },
}

impl From<scoop_slib::ArtifactManifestSummaryV1> for ScoopcProductV1 {
    fn from(summary: scoop_slib::ArtifactManifestSummaryV1) -> Self {
        Self::Artifact(Box::new(summary))
    }
}

impl ScoopcProductV1 {
    pub fn summary(&self) -> Option<&scoop_slib::ArtifactManifestSummaryV1> {
        match self {
            Self::Artifact(summary)
            | Self::ArtifactWithDocumentation {
                artifact: summary, ..
            } => Some(summary),
            Self::Documentation(_) => None,
        }
    }

    pub fn documentation(&self) -> Option<&ScoopcDocumentationOutputV1> {
        match self {
            Self::Documentation(output)
            | Self::ArtifactWithDocumentation {
                documentation: output,
                ..
            } => Some(output),
            Self::Artifact(_) => None,
        }
    }
}

impl WireEncode for ScoopcProductV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let tag = match self {
            Self::Artifact(_) => 1,
            Self::Documentation(_) => 2,
            Self::ArtifactWithDocumentation { .. } => 3,
        };
        encoder.map(if tag == 3 { 3 } else { 2 })?;
        encoder.field(0)?;
        encoder.unsigned(tag)?;
        encoder.field(1)?;
        match self {
            Self::Artifact(summary) => summary.encode(encoder)?,
            Self::Documentation(output) => output.encode(encoder)?,
            Self::ArtifactWithDocumentation {
                artifact,
                documentation,
            } => {
                artifact.encode(encoder)?;
                encoder.field(2)?;
                documentation.encode(encoder)?;
            }
        }
        Ok(())
    }
}
