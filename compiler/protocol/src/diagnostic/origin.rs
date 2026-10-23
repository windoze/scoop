use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_primary_and_note_preserve_raw_text_through_transport() {
        let origin = DiagnosticOriginV1::SourceTextSpan {
            cone: ProtocolConeIdentity::from_array([3; 32]),
            logical_path: NormalizedSourcePath::new("generated/body.scoop").unwrap(),
            span: ProtocolByteSpan::new(15, 20).unwrap(),
            text: "{\n    val 中文 = unknown\n}".into(),
        };
        let diagnostic = StructuredDiagnosticV1::new(
            DiagnosticSeverityV1::Error,
            "SCOOPC_HIR_ERROR".into(),
            "unknown binding".into(),
            origin.clone(),
            vec![DiagnosticNoteV1::new("in the same fragment".into(), origin).unwrap()],
        )
        .unwrap();
        assert_eq!(
            scoop_wire::decode_canonical::<StructuredDiagnosticV1>(
                &scoop_wire::encode(&diagnostic).unwrap()
            )
            .unwrap(),
            diagnostic
        );
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagnosticOriginV1 {
    None,
    HostPath {
        path: HostPathCarrier,
    },
    HostPathSpan {
        path: HostPathCarrier,
        span: ProtocolByteSpan,
    },
    SemanticSourceSpan {
        cone: ProtocolConeIdentity,
        logical_path: NormalizedSourcePath,
        span: ProtocolByteSpan,
    },
    SourceTextSpan {
        cone: ProtocolConeIdentity,
        logical_path: NormalizedSourcePath,
        span: ProtocolByteSpan,
        text: String,
    },
    ArtifactPath {
        path: HostPathCarrier,
        semantic_path: String,
    },
}

impl DiagnosticOriginV1 {
    pub fn artifact_path(
        path: HostPathCarrier,
        semantic_path: String,
    ) -> Result<Self, ProtocolValidationError> {
        if !valid_text(&semantic_path) {
            return Err(ProtocolValidationError::InvalidArtifactSemanticPath);
        }
        Ok(Self::ArtifactPath {
            path,
            semantic_path,
        })
    }
}

impl WireEncode for DiagnosticOriginV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::None => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::HostPath { path } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(5)?;
                encoder.field(1)?;
                path.encode(encoder)
            }
            Self::HostPathSpan { path, span } => {
                encoder.map(4)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                path.encode(encoder)?;
                encoder.field(2)?;
                encoder.unsigned(span.start)?;
                encoder.field(3)?;
                encoder.unsigned(span.end)
            }
            Self::SemanticSourceSpan {
                cone,
                logical_path,
                span,
            } => {
                encoder.map(5)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                cone.encode(encoder)?;
                encoder.field(2)?;
                logical_path.encode(encoder)?;
                encoder.field(3)?;
                encoder.unsigned(span.start)?;
                encoder.field(4)?;
                encoder.unsigned(span.end)
            }
            Self::ArtifactPath {
                path,
                semantic_path,
            } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(4)?;
                encoder.field(1)?;
                path.encode(encoder)?;
                encoder.field(2)?;
                encoder.text(semantic_path)
            }
            Self::SourceTextSpan {
                cone,
                logical_path,
                span,
                text,
            } => {
                encoder.map(6)?;
                encoder.field(0)?;
                encoder.unsigned(6)?;
                encoder.field(1)?;
                cone.encode(encoder)?;
                encoder.field(2)?;
                logical_path.encode(encoder)?;
                encoder.field(3)?;
                encoder.unsigned(span.start)?;
                encoder.field(4)?;
                encoder.unsigned(span.end)?;
                encoder.field(5)?;
                encoder.text(text)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DecodedDiagnosticOriginV1 {
    None,
    HostPath {
        path: DecodedHostPathCarrier,
    },
    HostPathSpan {
        path: DecodedHostPathCarrier,
        start: u64,
        end: u64,
    },
    SemanticSourceSpan {
        cone: Vec<u8>,
        logical_path: String,
        start: u64,
        end: u64,
    },
    SourceTextSpan {
        cone: Vec<u8>,
        logical_path: String,
        start: u64,
        end: u64,
        text: String,
    },
    ArtifactPath {
        path: DecodedHostPathCarrier,
        semantic_path: String,
    },
}

impl DecodedDiagnosticOriginV1 {
    pub(super) fn validate(self) -> Result<DiagnosticOriginV1, ProtocolValidationError> {
        match self {
            Self::None => Ok(DiagnosticOriginV1::None),
            Self::HostPath { path } => Ok(DiagnosticOriginV1::HostPath {
                path: path.validate().map_err(ProtocolValidationError::HostPath)?,
            }),
            Self::HostPathSpan { path, start, end } => Ok(DiagnosticOriginV1::HostPathSpan {
                path: path.validate().map_err(ProtocolValidationError::HostPath)?,
                span: ProtocolByteSpan::new(start, end)?,
            }),
            Self::SemanticSourceSpan {
                cone,
                logical_path,
                start,
                end,
            } => Ok(DiagnosticOriginV1::SemanticSourceSpan {
                cone: ProtocolConeIdentity::from_vec(cone)?,
                logical_path: NormalizedSourcePath::new(&logical_path)
                    .map_err(|_| ProtocolValidationError::InvalidSemanticSourcePath)?,
                span: ProtocolByteSpan::new(start, end)?,
            }),
            Self::ArtifactPath {
                path,
                semantic_path,
            } => DiagnosticOriginV1::artifact_path(
                path.validate().map_err(ProtocolValidationError::HostPath)?,
                semantic_path,
            ),
            Self::SourceTextSpan {
                cone,
                logical_path,
                start,
                end,
                text,
            } => Ok(DiagnosticOriginV1::SourceTextSpan {
                cone: ProtocolConeIdentity::from_vec(cone)?,
                logical_path: NormalizedSourcePath::new(&logical_path)
                    .map_err(|_| ProtocolValidationError::InvalidSemanticSourcePath)?,
                span: ProtocolByteSpan::new(start, end)?,
                text,
            }),
        }
    }
}

impl WireEncode for DecodedDiagnosticOriginV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::SourceTextSpan {
                cone,
                logical_path,
                start,
                end,
                text,
            } => {
                encoder.map(6)?;
                encoder.field(0)?;
                encoder.unsigned(6)?;
                encoder.field(1)?;
                encoder.bytes(cone)?;
                encoder.field(2)?;
                encoder.text(logical_path)?;
                encoder.field(3)?;
                encoder.unsigned(*start)?;
                encoder.field(4)?;
                encoder.unsigned(*end)?;
                encoder.field(5)?;
                encoder.text(text)
            }
            Self::None => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::HostPath { path } => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(5)?;
                encoder.field(1)?;
                path.encode(encoder)
            }
            Self::HostPathSpan { path, start, end } => {
                encoder.map(4)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                path.encode(encoder)?;
                encoder.field(2)?;
                encoder.unsigned(*start)?;
                encoder.field(3)?;
                encoder.unsigned(*end)
            }
            Self::SemanticSourceSpan {
                cone,
                logical_path,
                start,
                end,
            } => {
                encoder.map(5)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                encoder.bytes(cone)?;
                encoder.field(2)?;
                encoder.text(logical_path)?;
                encoder.field(3)?;
                encoder.unsigned(*start)?;
                encoder.field(4)?;
                encoder.unsigned(*end)
            }
            Self::ArtifactPath {
                path,
                semantic_path,
            } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(4)?;
                encoder.field(1)?;
                path.encode(encoder)?;
                encoder.field(2)?;
                encoder.text(semantic_path)
            }
        }
    }
}

impl WireDecode for DecodedDiagnosticOriginV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            6 => {
                crate::framing::expect_sum_length(decoder, fields, 6)?;
                Ok(Self::SourceTextSpan {
                    cone: decoder.field(1, Decoder::owned_bytes)?,
                    logical_path: decoder.field(2, Decoder::owned_text)?,
                    start: decoder.field(3, Decoder::unsigned)?,
                    end: decoder.field(4, Decoder::unsigned)?,
                    text: decoder.field(5, Decoder::owned_text)?,
                })
            }
            1 => {
                crate::framing::expect_sum_length(decoder, fields, 1)?;
                Ok(Self::None)
            }
            2 => {
                crate::framing::expect_sum_length(decoder, fields, 4)?;
                let path = decoder.field(1, DecodedHostPathCarrier::decode)?;
                let start = decoder.field(2, Decoder::unsigned)?;
                let end = decoder.field(3, Decoder::unsigned)?;
                Ok(Self::HostPathSpan { path, start, end })
            }
            3 => {
                crate::framing::expect_sum_length(decoder, fields, 5)?;
                let cone = decoder.field(1, Decoder::owned_bytes)?;
                let logical_path = decoder.field(2, Decoder::owned_text)?;
                let start = decoder.field(3, Decoder::unsigned)?;
                let end = decoder.field(4, Decoder::unsigned)?;
                Ok(Self::SemanticSourceSpan {
                    cone,
                    logical_path,
                    start,
                    end,
                })
            }
            4 => {
                crate::framing::expect_sum_length(decoder, fields, 3)?;
                let path = decoder.field(1, DecodedHostPathCarrier::decode)?;
                let semantic_path = decoder.field(2, Decoder::owned_text)?;
                Ok(Self::ArtifactPath {
                    path,
                    semantic_path,
                })
            }
            5 => {
                crate::framing::expect_sum_length(decoder, fields, 2)?;
                let path = decoder.field(1, DecodedHostPathCarrier::decode)?;
                Ok(Self::HostPath { path })
            }
            tag => Err(crate::framing::unknown_tag(decoder, tag)),
        }
    }
}
