use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoopcOutputRequestV1 {
    Artifact(HostPathCarrier),
    Documentation(HostPathCarrier),
    ArtifactWithDocumentation {
        artifact: HostPathCarrier,
        documentation: HostPathCarrier,
    },
}

impl From<HostPathCarrier> for ScoopcOutputRequestV1 {
    fn from(path: HostPathCarrier) -> Self {
        Self::Artifact(path)
    }
}

impl ScoopcOutputRequestV1 {
    pub fn artifact(&self) -> Option<&HostPathCarrier> {
        match self {
            Self::Artifact(path) | Self::ArtifactWithDocumentation { artifact: path, .. } => {
                Some(path)
            }
            Self::Documentation(_) => None,
        }
    }

    pub fn documentation(&self) -> Option<&HostPathCarrier> {
        match self {
            Self::Documentation(path)
            | Self::ArtifactWithDocumentation {
                documentation: path,
                ..
            } => Some(path),
            Self::Artifact(_) => None,
        }
    }
}

impl WireEncode for ScoopcOutputRequestV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, first, second) = match self {
            Self::Artifact(path) => (1, path, None),
            Self::Documentation(path) => (2, path, None),
            Self::ArtifactWithDocumentation {
                artifact,
                documentation,
            } => (3, artifact, Some(documentation)),
        };
        encode(encoder, tag, first, second)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DecodedOutputRequest {
    Artifact(DecodedHostPathCarrier),
    Documentation(DecodedHostPathCarrier),
    Both(DecodedHostPathCarrier, DecodedHostPathCarrier),
}

impl DecodedOutputRequest {
    pub(super) fn validate(self) -> Result<ScoopcOutputRequestV1, ProtocolValidationError> {
        let path = |value: DecodedHostPathCarrier| {
            value.validate().map_err(ProtocolValidationError::HostPath)
        };
        Ok(match self {
            Self::Artifact(value) => ScoopcOutputRequestV1::Artifact(path(value)?),
            Self::Documentation(value) => ScoopcOutputRequestV1::Documentation(path(value)?),
            Self::Both(artifact, documentation) => {
                ScoopcOutputRequestV1::ArtifactWithDocumentation {
                    artifact: path(artifact)?,
                    documentation: path(documentation)?,
                }
            }
        })
    }
}

impl WireEncode for DecodedOutputRequest {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, first, second) = match self {
            Self::Artifact(path) => (1, path, None),
            Self::Documentation(path) => (2, path, None),
            Self::Both(artifact, documentation) => (3, artifact, Some(documentation)),
        };
        encode(encoder, tag, first, second)
    }
}

fn encode(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: Option<&impl WireEncode>,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(if second.is_some() { 3 } else { 2 })?;
    encoder.field(0)?;
    encoder.unsigned(tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    if let Some(second) = second {
        encoder.field(2)?;
        second.encode(encoder)?;
    }
    Ok(())
}

impl WireDecode for DecodedOutputRequest {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        crate::framing::expect_sum_length(decoder, fields, if tag == 3 { 3 } else { 2 })?;
        let first = decoder.field(1, DecodedHostPathCarrier::decode)?;
        match tag {
            1 => Ok(Self::Artifact(first)),
            2 => Ok(Self::Documentation(first)),
            3 => Ok(Self::Both(
                first,
                decoder.field(2, DecodedHostPathCarrier::decode)?,
            )),
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        }
    }
}
