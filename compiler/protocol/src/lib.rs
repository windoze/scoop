//! Versioned, bounded same-host child protocol for single-Cone `scoopc` builds.
//!
//! Host paths are opaque transport values. They never become persistent
//! semantic identities or artifact fingerprint inputs.

mod capability;
mod diagnostic;
mod dump;
pub use dump::{StageDumpKindV1, StageDumpPolicyV1, StageDumpSet};
mod framing;
mod generators;
pub use generators::{GeneratorRegistrationV1, GeneratorToolInputV1};
mod path;
mod purpose;
pub use purpose::CompilationPurpose;
mod request;
pub use scoop_lir::OptimizationMode;
mod response;

pub use diagnostic::{
    DiagnosticNoteV1, DiagnosticOriginV1, DiagnosticSeverityV1, ProtocolByteSpan,
    StructuredDiagnosticV1,
};
pub use framing::{
    ProtocolFrameError, ProtocolReadError, ProtocolWriteError, decode_request_frame,
    decode_response_frame, encode_request_frame, encode_response_frame,
};
pub use path::{DecodedHostPathCarrier, HostPathCarrier, HostPathEncoding, HostPathError};
pub use request::{
    CurrentConeRequestV1, DiagnosticOutputPolicyV1, ScoopcBuildRequestV1, ScoopcOutputRequestV1,
    ScoopcRequestEnvelopeV1, TargetSelectionRequestV1, TrustedCoreRequestV1,
};
pub use response::{
    EmittedDumpDescriptorV1, EmittedDumpDestinationV1, ProtocolConeIdentity, RequestCorrelationId,
    ScoopcDocumentationOutputV1, ScoopcObservationV1, ScoopcProductV1, ScoopcResponseEnvelopeV1,
    ScoopcSuccessV1,
};

/// Version selected by the unique `scoopc` machine transport entrypoint.
pub const PROTOCOL_VERSION: u32 = 14;

pub(crate) const MAX_EMITTED_DUMPS: usize = 4;

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

    InvalidCurrentCoreCombination,
    TestRequiresManifest,
    DocumentationRequiresProductionManifest,
    DocumentationObservation,
    DocumentationMachineDump,
    InvalidTargetTriple,
    InvalidGeneratorRegistration,

    TooManyEmittedDumps(usize),
    InvalidDumpStages,
    InvalidDumpDescriptors,
    FailureRequiresDiagnostic,
    FailureRequiresErrorDiagnostic,
    SuccessContainsErrorDiagnostic,
    InvalidDiagnosticCode,
    InvalidDiagnosticMessage,
    InvalidDiagnosticSpan,
    InvalidSemanticSourcePath,

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
            Self::DocumentationRequiresProductionManifest => {
                formatter.write_str("documentation requires a Production manifest Cone")
            }
            Self::DocumentationObservation => {
                formatter.write_str("documentation cannot be combined with observation-only")
            }
            Self::DocumentationMachineDump => {
                formatter.write_str("documentation-only output cannot emit MIR or LIR")
            }

            Self::InvalidCurrentCoreCombination => formatter
                .write_str("current Cone input and trusted core request are not a permitted pair"),
            Self::TestRequiresManifest => {
                formatter.write_str("test compilation requires a manifest Cone")
            }
            Self::InvalidTargetTriple => formatter.write_str(
                "target triple must be 1..255 printable ASCII bytes without path separators",
            ),
            Self::InvalidGeneratorRegistration => formatter
                .write_str("generator registrations require nonempty, unique names in name order"),

            Self::TooManyEmittedDumps(actual) => write!(
                formatter,
                "too many emitted dump descriptors: limit 4, found {actual}"
            ),
            Self::InvalidDumpStages => {
                formatter.write_str("dump stages must be nonempty, unique, and in stage order")
            }
            Self::InvalidDumpDescriptors => {
                formatter.write_str("dump descriptors must name unique files in stage order")
            }
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

#[cfg(test)]
pub(crate) fn test_summary() -> scoop_slib::ArtifactManifestSummaryV1 {
    use scoop_slib::*;
    ArtifactManifestSummaryV1::new(
        ConeRecord::new(
            scoop_identity::ConeCoordinate::reserved_core(),
            ConeKind::Library,
            ConeSourceForm::Manifest,
        )
        .unwrap(),
        Vec::new(),
        scoop_target_data::DARWIN_AARCH64_LLVM_22_1,
        scoop_identity::ArtifactCapabilityProfileId::cross_cone_generic(),
        SemanticFingerprintRecord::from_digests(
            HirFingerprint::from_array([3; 32]),
            MirFingerprint::from_array([4; 32]),
            LirFingerprint::from_array([5; 32]),
            FingerprintAvailability::Available(CodeFingerprint::from_array([6; 32])),
            FingerprintAvailability::Available(RuntimeImageFingerprint::from_array([7; 32])),
        ),
        ArtifactFingerprint::from_array([1; 32]),
        None,
    )
}
