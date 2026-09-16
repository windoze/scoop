//! Versioned, bounded same-host child protocol for single-Cone `scoopc` builds.
//!
//! Host paths are opaque transport values. They never become persistent
//! semantic identities or artifact fingerprint inputs.

mod capability;
mod diagnostic;
mod framing;
mod path;
mod request;
mod response;

pub use diagnostic::{
    DiagnosticNoteV1, DiagnosticOriginV1, DiagnosticSeverityV1, ProtocolByteSpan,
    StructuredDiagnosticV1,
};
pub use framing::{
    PROTOCOL_MAX_FRAME_BYTES, ProtocolFrameError, ProtocolReadError, ProtocolWriteError,
    decode_request_frame, decode_request_frame_with_usage, decode_response_frame,
    decode_response_frame_with_usage, encode_request_frame, encode_response_frame,
};
pub use path::{HostPathCarrier, HostPathEncoding, HostPathError};
pub use request::{
    CurrentConeRequestV1, DiagnosticOutputPolicyV1, ScoopcBuildRequestV1, ScoopcRequestEnvelopeV1,
    StageDumpKindV1, StageDumpPolicyV1, TargetSelectionRequestV1, TrustedCoreRequestV1,
};
pub use response::{
    EmittedDumpDescriptorV1, EmittedDumpDestinationV1, ProtocolArtifactFingerprint,
    ProtocolCodeFingerprint, ProtocolConeIdentity, ProtocolDumpContentDigest,
    ProtocolHirFingerprint, ProtocolLirFingerprint, ProtocolMirFingerprint,
    ProtocolRuntimeImageFingerprint, RequestCorrelationId, ScoopcResponseEnvelopeV1,
    ScoopcSuccessV1,
};

/// Version selected by the unique `scoopc` machine transport entrypoint.
pub const PROTOCOL_VERSION: u32 = 1;
pub(crate) const MAX_INPUT_ARTIFACTS: usize = 4_096;
pub(crate) const MAX_DIAGNOSTICS: usize = 4_096;
pub(crate) const MAX_DIAGNOSTIC_NOTES: usize = 64;
pub(crate) const MAX_DIAGNOSTIC_TEXT_BYTES: usize = 1_048_576;
pub(crate) const MAX_EMITTED_DUMPS: usize = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProtocolValidationError {
    InvalidCapabilityMagic,
    InvalidRequestMagic,
    InvalidResponseMagic,
    UnsupportedVersion(u32),
    InvalidRequestIdLength(usize),
    UnknownEnumTag { kind: &'static str, tag: u64 },
    InvalidDigestLength { field: &'static str, actual: usize },
    HostPath(HostPathError),
    TooManyInputs { role: &'static str, actual: usize },
    InvalidCurrentCoreCombination,
    SingleFileHasDependencies,
    BootstrapHasDependencies,
    InvalidTargetTriple,
    TooManyDiagnostics(usize),
    TooManyEmittedDumps(usize),
    FailureRequiresDiagnostic,
    FailureRequiresErrorDiagnostic,
    SuccessContainsErrorDiagnostic,
    InvalidDiagnosticCode,
    InvalidDiagnosticMessage,
    InvalidDiagnosticSpan,
    InvalidSemanticSourcePath,
    TooManyDiagnosticNotes(usize),
    InvalidArtifactSemanticPath,
}

impl std::fmt::Display for ProtocolValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCapabilityMagic => formatter.write_str("invalid scoopc capability magic"),
            Self::InvalidRequestMagic => formatter.write_str("invalid scoopc request magic"),
            Self::InvalidResponseMagic => formatter.write_str("invalid scoopc response magic"),
            Self::UnsupportedVersion(version) => {
                write!(formatter, "unsupported scoopc protocol version {version}")
            }
            Self::InvalidRequestIdLength(actual) => {
                write!(
                    formatter,
                    "request id must contain 16 bytes, found {actual}"
                )
            }
            Self::UnknownEnumTag { kind, tag } => {
                write!(formatter, "unknown {kind} tag {tag}")
            }
            Self::InvalidDigestLength { field, actual } => {
                write!(formatter, "{field} must contain 32 bytes, found {actual}")
            }
            Self::HostPath(error) => error.fmt(formatter),
            Self::TooManyInputs { role, actual } => {
                write!(
                    formatter,
                    "too many {role} inputs: limit 4096, found {actual}"
                )
            }
            Self::InvalidCurrentCoreCombination => formatter
                .write_str("current Cone input and trusted core request are not a permitted pair"),
            Self::SingleFileHasDependencies => {
                formatter.write_str("single-file input cannot carry dependency artifacts")
            }
            Self::BootstrapHasDependencies => {
                formatter.write_str("trusted core bootstrap cannot carry dependency artifacts")
            }
            Self::InvalidTargetTriple => formatter.write_str(
                "target triple must be 1..255 printable ASCII bytes without path separators",
            ),
            Self::TooManyDiagnostics(actual) => write!(
                formatter,
                "too many structured diagnostics: limit 4096, found {actual}"
            ),
            Self::TooManyEmittedDumps(actual) => write!(
                formatter,
                "too many emitted dump descriptors: limit 1, found {actual}"
            ),
            Self::FailureRequiresDiagnostic => {
                formatter.write_str("a failure response requires at least one diagnostic")
            }
            Self::FailureRequiresErrorDiagnostic => {
                formatter.write_str("a failure response requires at least one error diagnostic")
            }
            Self::SuccessContainsErrorDiagnostic => {
                formatter.write_str("a success response may contain only warning diagnostics")
            }
            Self::InvalidDiagnosticCode => {
                formatter.write_str("diagnostic code must match [A-Z][A-Z0-9_]{0,127}")
            }
            Self::InvalidDiagnosticMessage => {
                formatter.write_str("diagnostic message must contain 1..1048576 UTF-8 bytes")
            }
            Self::InvalidDiagnosticSpan => {
                formatter.write_str("diagnostic byte span start must not exceed end")
            }
            Self::InvalidSemanticSourcePath => {
                formatter.write_str("diagnostic semantic source path is not canonical")
            }
            Self::TooManyDiagnosticNotes(actual) => write!(
                formatter,
                "too many diagnostic notes: limit 64, found {actual}"
            ),
            Self::InvalidArtifactSemanticPath => {
                formatter.write_str("artifact diagnostic path must contain 1..1048576 UTF-8 bytes")
            }
        }
    }
}

impl std::error::Error for ProtocolValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::HostPath(error) => Some(error),
            _ => None,
        }
    }
}
pub use capability::{
    MachineTransportCapabilityV1, ScoopcMachineCapabilityV1, ScoopcProtocolCapabilityV1,
    decode_capability_frame, decode_machine_capability_frame, encode_capability_frame,
    encode_machine_capability_frame,
};
