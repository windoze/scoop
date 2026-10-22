use scoop_identity::NormalizedSourcePath;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use crate::path::DecodedHostPathCarrier;
use crate::response::ProtocolConeIdentity;
use crate::{HostPathCarrier, ProtocolValidationError};

mod artifact_paths;
mod origin;
use origin::DecodedDiagnosticOriginV1;
pub use origin::DiagnosticOriginV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticSeverityV1 {
    Error,
    Warning,
}

impl DiagnosticSeverityV1 {
    pub const fn is_error(self) -> bool {
        matches!(self, Self::Error)
    }

    const fn tag(self) -> u64 {
        match self {
            Self::Error => 1,
            Self::Warning => 2,
        }
    }

    fn from_tag(tag: u64) -> Result<Self, ProtocolValidationError> {
        match tag {
            1 => Ok(Self::Error),
            2 => Ok(Self::Warning),
            _ => Err(ProtocolValidationError::UnknownEnumTag {
                kind: "diagnostic severity",
                tag,
            }),
        }
    }
}

impl WireEncode for DiagnosticSeverityV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(self.tag())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtocolByteSpan {
    start: u64,
    end: u64,
}

impl ProtocolByteSpan {
    pub fn new(start: u64, end: u64) -> Result<Self, ProtocolValidationError> {
        if start > end {
            return Err(ProtocolValidationError::InvalidDiagnosticSpan);
        }
        Ok(Self { start, end })
    }

    pub const fn start(self) -> u64 {
        self.start
    }

    pub const fn end(self) -> u64 {
        self.end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticNoteV1 {
    message: String,
    origin: DiagnosticOriginV1,
}

impl DiagnosticNoteV1 {
    pub fn new(
        message: String,
        origin: DiagnosticOriginV1,
    ) -> Result<Self, ProtocolValidationError> {
        if !valid_text(&message) {
            return Err(ProtocolValidationError::InvalidDiagnosticMessage);
        }
        Ok(Self { message, origin })
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn origin(&self) -> &DiagnosticOriginV1 {
        &self.origin
    }
}

impl WireEncode for DiagnosticNoteV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.text(&self.message)?;
        encoder.field(2)?;
        self.origin.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredDiagnosticV1 {
    severity: DiagnosticSeverityV1,
    code: String,
    message: String,
    origin: DiagnosticOriginV1,
    notes: Vec<DiagnosticNoteV1>,
}

impl StructuredDiagnosticV1 {
    pub fn new(
        severity: DiagnosticSeverityV1,
        code: String,
        message: String,
        origin: DiagnosticOriginV1,
        notes: Vec<DiagnosticNoteV1>,
    ) -> Result<Self, ProtocolValidationError> {
        if !valid_code(&code) {
            return Err(ProtocolValidationError::InvalidDiagnosticCode);
        }
        if !valid_text(&message) {
            return Err(ProtocolValidationError::InvalidDiagnosticMessage);
        }

        Ok(Self {
            severity,
            code,
            message,
            origin,
            notes,
        })
    }

    pub const fn severity(&self) -> DiagnosticSeverityV1 {
        self.severity
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub const fn origin(&self) -> &DiagnosticOriginV1 {
        &self.origin
    }

    pub fn notes(&self) -> &[DiagnosticNoteV1] {
        &self.notes
    }
}

impl WireEncode for StructuredDiagnosticV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        self.severity.encode(encoder)?;
        encoder.field(2)?;
        encoder.text(&self.code)?;
        encoder.field(3)?;
        encoder.text(&self.message)?;
        encoder.field(4)?;
        self.origin.encode(encoder)?;
        encoder.field(5)?;
        encoder.array(self.notes.len() as u64)?;
        for note in &self.notes {
            note.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for StructuredDiagnosticV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        DecodedStructuredDiagnosticV1::decode(decoder)?
            .validate()
            .map_err(|error| diagnostic_validation_error(decoder, error))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedDiagnosticNoteV1 {
    message: String,
    origin: DecodedDiagnosticOriginV1,
}

impl DecodedDiagnosticNoteV1 {
    fn validate(self) -> Result<DiagnosticNoteV1, ProtocolValidationError> {
        DiagnosticNoteV1::new(self.message, self.origin.validate()?)
    }
}

impl WireEncode for DecodedDiagnosticNoteV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.text(&self.message)?;
        encoder.field(2)?;
        self.origin.encode(encoder)
    }
}

impl WireDecode for DecodedDiagnosticNoteV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(2)?;
        let message = decoder.field(1, Decoder::owned_text)?;
        let origin = decoder.field(2, DecodedDiagnosticOriginV1::decode)?;
        Ok(Self { message, origin })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DecodedStructuredDiagnosticV1 {
    severity: u64,
    code: String,
    message: String,
    origin: DecodedDiagnosticOriginV1,
    notes: Vec<DecodedDiagnosticNoteV1>,
}

impl DecodedStructuredDiagnosticV1 {
    pub(crate) fn validate(self) -> Result<StructuredDiagnosticV1, ProtocolValidationError> {
        let notes = self
            .notes
            .into_iter()
            .map(DecodedDiagnosticNoteV1::validate)
            .collect::<Result<Vec<_>, _>>()?;
        StructuredDiagnosticV1::new(
            DiagnosticSeverityV1::from_tag(self.severity)?,
            self.code,
            self.message,
            self.origin.validate()?,
            notes,
        )
    }
}

impl WireEncode for DecodedStructuredDiagnosticV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        encoder.unsigned(self.severity)?;
        encoder.field(2)?;
        encoder.text(&self.code)?;
        encoder.field(3)?;
        encoder.text(&self.message)?;
        encoder.field(4)?;
        self.origin.encode(encoder)?;
        encoder.field(5)?;
        encoder.array(self.notes.len() as u64)?;
        for note in &self.notes {
            note.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedStructuredDiagnosticV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(5)?;
        let severity = decoder.field(1, Decoder::unsigned)?;
        let code = decoder.field(2, Decoder::owned_text)?;
        let message = decoder.field(3, Decoder::owned_text)?;
        let origin = decoder.field(4, DecodedDiagnosticOriginV1::decode)?;
        let notes = decoder.field(5, |decoder| {
            decoder.decode_array(|decoder, _| DecodedDiagnosticNoteV1::decode(decoder))
        })?;
        Ok(Self {
            severity,
            code,
            message,
            origin,
            notes,
        })
    }
}

fn valid_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 128
        && bytes[0].is_ascii_uppercase()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || *byte == b'_')
}

fn valid_text(text: &str) -> bool {
    !text.is_empty()
}

fn diagnostic_validation_error(
    decoder: &Decoder<'_>,
    _error: ProtocolValidationError,
) -> WireError {
    let kind = scoop_wire::WireErrorKind::UnknownTag { tag: u64::MAX };
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_diagnostic_wire_decode_validates_and_round_trips() {
        let diagnostic = StructuredDiagnosticV1::new(
            DiagnosticSeverityV1::Warning,
            "SCOOPC_TEST_WARNING".to_owned(),
            "bounded warning".to_owned(),
            DiagnosticOriginV1::None,
            Vec::new(),
        )
        .unwrap();
        let encoded = scoop_wire::encode(&diagnostic).unwrap();

        assert_eq!(
            scoop_wire::decode_canonical::<StructuredDiagnosticV1>(&encoded,).unwrap(),
            diagnostic
        );
    }

    #[test]
    fn diagnostic_constructor_rejects_invalid_code_and_span() {
        assert_eq!(
            StructuredDiagnosticV1::new(
                DiagnosticSeverityV1::Error,
                "bad-code".to_owned(),
                "message".to_owned(),
                DiagnosticOriginV1::None,
                Vec::new(),
            )
            .unwrap_err(),
            ProtocolValidationError::InvalidDiagnosticCode
        );
        assert_eq!(
            ProtocolByteSpan::new(2, 1).unwrap_err(),
            ProtocolValidationError::InvalidDiagnosticSpan
        );
    }
}
