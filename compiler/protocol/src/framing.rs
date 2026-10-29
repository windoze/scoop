use std::fmt;

use scoop_wire::{Decoder, WireError, WireErrorKind, decode_canonical, encode};

use crate::request::DecodedScoopcRequestEnvelopeV1;
use crate::response::DecodedScoopcResponseEnvelopeV1;
use crate::{ScoopcRequestEnvelopeV1, ScoopcResponseEnvelopeV1};

const FRAME_LENGTH_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolFrameError {
    MissingLength,

    LengthOutOfRange(u64),
    LengthMismatch { declared: u64, actual: usize },
    Allocation,
}

impl fmt::Display for ProtocolFrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingLength => {
                formatter.write_str("protocol frame is shorter than its 8-byte length prefix")
            }

            Self::LengthOutOfRange(length) => {
                write!(
                    formatter,
                    "protocol payload length {length} does not fit this host"
                )
            }
            Self::LengthMismatch { declared, actual } => write!(
                formatter,
                "protocol frame declares {declared} payload bytes but contains {actual}"
            ),
            Self::Allocation => formatter.write_str("failed to allocate protocol frame"),
        }
    }
}

impl std::error::Error for ProtocolFrameError {}

#[derive(Debug)]
pub enum ProtocolWriteError {
    Wire(scoop_wire::cbor::EncodeError),
    Frame(ProtocolFrameError),
}

impl fmt::Display for ProtocolWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Wire(error) => error.fmt(formatter),
            Self::Frame(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProtocolWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Wire(error) => Some(error),
            Self::Frame(error) => Some(error),
        }
    }
}

#[derive(Debug)]
pub enum ProtocolReadError {
    Frame(ProtocolFrameError),
    Wire(WireError),
    Validation(crate::ProtocolValidationError),
}

impl fmt::Display for ProtocolReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frame(error) => error.fmt(formatter),
            Self::Wire(error) => error.fmt(formatter),
            Self::Validation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ProtocolReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Frame(error) => Some(error),
            Self::Wire(error) => Some(error),
            Self::Validation(error) => Some(error),
        }
    }
}

pub fn encode_request_frame(
    request: &ScoopcRequestEnvelopeV1,
) -> Result<Vec<u8>, ProtocolWriteError> {
    encode_frame(request)
}

pub fn encode_response_frame(
    response: &ScoopcResponseEnvelopeV1,
) -> Result<Vec<u8>, ProtocolWriteError> {
    encode_frame(response)
}

pub fn decode_request_frame(frame: &[u8]) -> Result<ScoopcRequestEnvelopeV1, ProtocolReadError> {
    let payload = decode_frame_payload(frame).map_err(ProtocolReadError::Frame)?;

    let decoded = decode_canonical::<DecodedScoopcRequestEnvelopeV1>(payload)
        .map_err(ProtocolReadError::Wire)?;
    let request = decoded.validate().map_err(ProtocolReadError::Validation)?;
    Ok(request)
}

pub fn decode_response_frame(frame: &[u8]) -> Result<ScoopcResponseEnvelopeV1, ProtocolReadError> {
    let payload = decode_frame_payload(frame).map_err(ProtocolReadError::Frame)?;

    let decoded = decode_canonical::<DecodedScoopcResponseEnvelopeV1>(payload)
        .map_err(ProtocolReadError::Wire)?;
    let response = decoded.validate().map_err(ProtocolReadError::Validation)?;
    Ok(response)
}

pub(crate) fn encode_frame(
    value: &impl scoop_wire::WireEncode,
) -> Result<Vec<u8>, ProtocolWriteError> {
    let payload = encode(value).map_err(ProtocolWriteError::Wire)?;
    let payload_length = u64::try_from(payload.len())
        .map_err(|_| ProtocolWriteError::Frame(ProtocolFrameError::Allocation))?;
    let frame_length = FRAME_LENGTH_BYTES
        .checked_add(payload.len())
        .ok_or(ProtocolWriteError::Frame(ProtocolFrameError::Allocation))?;
    let mut frame = Vec::new();
    frame
        .try_reserve_exact(frame_length)
        .map_err(|_| ProtocolWriteError::Frame(ProtocolFrameError::Allocation))?;
    frame.extend_from_slice(&payload_length.to_le_bytes());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

pub(crate) fn decode_frame_payload(frame: &[u8]) -> Result<&[u8], ProtocolFrameError> {
    let length_bytes = frame
        .get(..FRAME_LENGTH_BYTES)
        .ok_or(ProtocolFrameError::MissingLength)?;
    let mut length_array = [0_u8; FRAME_LENGTH_BYTES];
    length_array.copy_from_slice(length_bytes);
    let declared = u64::from_le_bytes(length_array);

    let declared_usize =
        usize::try_from(declared).map_err(|_| ProtocolFrameError::LengthOutOfRange(declared))?;
    let actual = frame.len() - FRAME_LENGTH_BYTES;
    if declared_usize != actual {
        return Err(ProtocolFrameError::LengthMismatch { declared, actual });
    }
    Ok(&frame[FRAME_LENGTH_BYTES..])
}

pub(crate) fn expect_sum_length(
    decoder: &Decoder<'_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        return Ok(());
    }
    Err(WireError::new(
        WireErrorKind::InvalidLength { expected, actual },
        decoder.path().clone(),
        Some(decoder.position()),
    ))
}

pub(crate) fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        DiagnosticNoteV1, DiagnosticOriginV1, DiagnosticOutputPolicyV1, DiagnosticSeverityV1,
        EmittedDumpDescriptorV1, EmittedDumpDestinationV1, HostPathCarrier,
        ProtocolValidationError, RequestCorrelationId, ScoopcBuildRequestV1,
        ScoopcRequestEnvelopeV1, ScoopcResponseEnvelopeV1, ScoopcSuccessV1, StageDumpKindV1,
        StageDumpPolicyV1, StructuredDiagnosticV1, TargetSelectionRequestV1, TrustedCoreRequestV1,
    };

    use super::*;

    fn path(value: &str) -> HostPathCarrier {
        HostPathCarrier::from_path(Path::new(value)).unwrap()
    }

    fn warning() -> StructuredDiagnosticV1 {
        StructuredDiagnosticV1::new(
            DiagnosticSeverityV1::Warning,
            "SCOOPC_SAMPLE_WARNING".to_owned(),
            "sample warning".to_owned(),
            DiagnosticOriginV1::None,
            vec![
                DiagnosticNoteV1::new("sample note".to_owned(), DiagnosticOriginV1::None).unwrap(),
            ],
        )
        .unwrap()
    }

    fn error() -> StructuredDiagnosticV1 {
        StructuredDiagnosticV1::new(
            DiagnosticSeverityV1::Error,
            "SCOOPC_SAMPLE_ERROR".to_owned(),
            "sample error".to_owned(),
            DiagnosticOriginV1::None,
            Vec::new(),
        )
        .unwrap()
    }

    fn request() -> ScoopcRequestEnvelopeV1 {
        request_with_emit(StageDumpPolicyV1::None)
    }

    fn request_with_emit(emit: StageDumpPolicyV1) -> ScoopcRequestEnvelopeV1 {
        ScoopcRequestEnvelopeV1::new(
            RequestCorrelationId::from_array([7; 16]),
            ScoopcBuildRequestV1::new(
                crate::test_support::manifest("project"),
                vec![path("deps/direct.slib")],
                vec![path("deps/support.slib")],
                TrustedCoreRequestV1::ArtifactSlot {
                    artifact: path("sysroot/core.slib"),
                },
                TargetSelectionRequestV1::new("aarch64-apple-darwin".to_owned()).unwrap(),
                path("out/current.slib"),
                DiagnosticOutputPolicyV1::Structured,
                emit,
            )
            .unwrap(),
        )
    }

    #[test]
    fn request_frame_round_trips_canonical_payload() {
        let request = request();
        let frame = encode_request_frame(&request).unwrap();
        assert_eq!(decode_request_frame(&frame).unwrap(), request);
        assert_eq!(
            u64::from_le_bytes(frame[..8].try_into().unwrap()) as usize,
            frame.len() - 8
        );
    }

    #[test]
    fn debug_level_round_trips_independently_of_optimization() {
        for level in [
            crate::DebugInfoLevel::None,
            crate::DebugInfoLevel::LineTables,
            crate::DebugInfoLevel::Full,
        ] {
            for mode in [
                crate::OptimizationMode::Debug,
                crate::OptimizationMode::Release,
            ] {
                let original = request();
                let request = ScoopcRequestEnvelopeV1::new(
                    original.request_id(),
                    original
                        .build()
                        .clone()
                        .with_optimization(mode)
                        .with_debug_info(level),
                );
                let decoded =
                    decode_request_frame(&encode_request_frame(&request).unwrap()).unwrap();
                assert_eq!(decoded.build().debug_info(), level);
                assert_eq!(decoded.build().optimization(), mode);
                assert_eq!(decoded, request);
            }
        }
    }

    #[test]
    fn observation_request_preserves_only_the_missing_stages() {
        let request = request_with_emit(StageDumpPolicyV1::Files {
            stages: crate::StageDumpSet::all(),
            directory: path("dump"),
        });
        let request = request
            .observe(crate::StageDumpSet::one(StageDumpKindV1::Hir))
            .unwrap();
        assert!(request.observation_only());
        assert_eq!(
            decode_request_frame(&encode_request_frame(&request).unwrap()).unwrap(),
            request
        );
    }

    #[test]
    fn success_and_failure_response_frames_round_trip() {
        let request_id = RequestCorrelationId::from_array([9; 16]);
        let success = ScoopcSuccessV1::new(
            crate::test_summary(),
            vec![warning()],
            StageDumpKindV1::ALL
                .into_iter()
                .map(|stage| {
                    EmittedDumpDescriptorV1::new(
                        stage,
                        EmittedDumpDestinationV1::File(path(&format!(
                            "dump/{}",
                            stage.file_name()
                        ))),
                    )
                })
                .collect(),
        )
        .unwrap();
        let success = ScoopcResponseEnvelopeV1::success(request_id, success);
        let frame = encode_response_frame(&success).unwrap();
        assert_eq!(decode_response_frame(&frame).unwrap(), success);

        let failure = ScoopcResponseEnvelopeV1::failure(request_id, vec![error()]).unwrap();
        let frame = encode_response_frame(&failure).unwrap();
        assert_eq!(decode_response_frame(&frame).unwrap(), failure);
    }

    #[test]
    fn frame_decode_preserves_each_canonical_document() {
        let request = request();
        let request_frame = encode_request_frame(&request).unwrap();
        let decoded_request = decode_request_frame(&request_frame).unwrap();
        assert_eq!(decoded_request, request);

        let response = ScoopcResponseEnvelopeV1::failure(
            RequestCorrelationId::from_array([7; 16]),
            vec![error()],
        )
        .unwrap();
        let response_frame = encode_response_frame(&response).unwrap();
        let decoded_response = decode_response_frame(&response_frame).unwrap();
        assert_eq!(decoded_response, response);
    }

    #[test]
    fn framing_rejects_truncation_trailing_bytes_and_false_lengths() {
        let frame = encode_request_frame(&request()).unwrap();
        let mut truncated = frame.clone();
        truncated.pop();
        assert!(matches!(
            decode_request_frame(&truncated),
            Err(ProtocolReadError::Frame(
                ProtocolFrameError::LengthMismatch { .. }
            ))
        ));

        let mut trailing = frame;
        trailing.push(0);
        assert!(matches!(
            decode_request_frame(&trailing),
            Err(ProtocolReadError::Frame(
                ProtocolFrameError::LengthMismatch { .. }
            ))
        ));

        let false_length = u64::MAX.to_le_bytes();
        assert!(matches!(
            decode_request_frame(&false_length),
            Err(ProtocolReadError::Frame(
                ProtocolFrameError::LengthMismatch { .. } | ProtocolFrameError::LengthOutOfRange(_)
            ))
        ));
    }

    #[test]
    fn reader_rejects_corrupted_magic_and_constructor_rejects_empty_failure() {
        let mut frame = encode_request_frame(&request()).unwrap();
        let magic = frame
            .windows(8)
            .position(|window| window == b"SCOOPREQ")
            .unwrap();
        frame[magic] ^= 1;
        assert!(matches!(
            decode_request_frame(&frame),
            Err(ProtocolReadError::Validation(
                ProtocolValidationError::InvalidRequestMagic
            ))
        ));

        assert_eq!(
            ScoopcResponseEnvelopeV1::failure(
                RequestCorrelationId::from_array([0; 16]),
                Vec::new(),
            )
            .unwrap_err(),
            ProtocolValidationError::FailureRequiresDiagnostic
        );
    }

    #[test]
    fn reader_rejects_unsupported_version_and_noncanonical_payload() {
        let mut version_frame = encode_request_frame(&request()).unwrap();
        let magic = version_frame
            .windows(8)
            .position(|window| window == b"SCOOPREQ")
            .unwrap();
        let version_start = magic + 9;
        let mut version_decoder = Decoder::new(&version_frame[version_start..]).unwrap();
        assert_eq!(version_decoder.u32().unwrap(), crate::PROTOCOL_VERSION);
        let version_length = usize::try_from(version_decoder.position()).unwrap();
        version_frame.splice(version_start..version_start + version_length, [1]);
        let payload_length = u64::try_from(version_frame.len() - FRAME_LENGTH_BYTES).unwrap();
        version_frame[..FRAME_LENGTH_BYTES].copy_from_slice(&payload_length.to_le_bytes());
        assert!(matches!(
            decode_request_frame(&version_frame),
            Err(ProtocolReadError::Validation(
                ProtocolValidationError::UnsupportedVersion(1)
            ))
        ));

        let frame = encode_request_frame(&request()).unwrap();
        let mut noncanonical = Vec::with_capacity(frame.len() + 1);
        noncanonical.extend_from_slice(&u64::try_from(frame.len() - 7).unwrap().to_le_bytes());
        noncanonical.extend_from_slice(&[0xb8, 4]);
        noncanonical.extend_from_slice(&frame[9..]);
        assert!(matches!(
            decode_request_frame(&noncanonical),
            Err(ProtocolReadError::Wire(error))
                if error.kind() == &WireErrorKind::NonCanonicalCbor
        ));
    }
}
