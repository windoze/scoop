use std::fmt;

use scoop_hir::HirFoundationValidationError;
use scoop_identity::CapabilityId;
use scoop_lir::LirFoundationValidationError;
use scoop_mir::MirFoundationValidationError;
use scoop_wire::WireError;

use crate::{MetadataLocation, MetadataReadError, SemanticFingerprintError, SlibMemberId};

#[derive(Debug)]
pub enum IdentityFoundationDecodeError {
    MissingMetadataMember {
        location: MetadataLocation,
    },
    MissingMemberPayload {
        location: MetadataLocation,
        member: SlibMemberId,
    },
    OuterEnvelope {
        location: MetadataLocation,
        source: MetadataReadError,
    },
    MissingFoundationSection {
        location: MetadataLocation,
        capability: CapabilityId,
    },
    UnknownCompileCapability {
        location: Option<MetadataLocation>,
        index: usize,
        capability: CapabilityId,
    },
    InnerFoundation {
        location: MetadataLocation,
        source: WireError,
    },
    SemanticFingerprints(SemanticFingerprintError),
    SemanticFingerprintMismatch {
        location: MetadataLocation,
        expected: [u8; 32],
        actual: [u8; 32],
    },
}

impl fmt::Display for IdentityFoundationDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingMetadataMember { location } => {
                write!(formatter, "artifact has no {location} metadata member")
            }
            Self::MissingMemberPayload { location, member } => {
                write!(
                    formatter,
                    "{location} metadata member {member} has no payload range"
                )
            }
            Self::OuterEnvelope { location, source } => {
                write!(formatter, "invalid {location} metadata envelope: {source}")
            }
            Self::MissingFoundationSection {
                location,
                capability,
            } => write!(
                formatter,
                "{location} metadata is missing required capability {}/{}/{}",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
            ),
            Self::UnknownCompileCapability {
                location,
                index,
                capability,
            } => {
                if let Some(location) = location {
                    write!(formatter, "{location} metadata section {index}")?;
                } else {
                    write!(formatter, "manifest section {index}")?;
                }
                write!(
                    formatter,
                    " requires unknown Compile capability {}/{}/{}",
                    capability.namespace(),
                    capability.name(),
                    capability.major_version(),
                )
            }
            Self::InnerFoundation { location, source } => {
                write!(
                    formatter,
                    "invalid {location} identity foundation: {source}"
                )
            }
            Self::SemanticFingerprints(source) => source.fmt(formatter),
            Self::SemanticFingerprintMismatch {
                location,
                expected,
                actual,
            } => {
                write!(
                    formatter,
                    "{location} semantic fingerprint mismatch: expected "
                )?;
                write_hex(expected, formatter)?;
                formatter.write_str(", found ")?;
                write_hex(actual, formatter)
            }
        }
    }
}

impl std::error::Error for IdentityFoundationDecodeError {}

#[derive(Debug)]
pub enum FoundationStructureValidationError {
    Hir(HirFoundationValidationError),
    Mir(MirFoundationValidationError),
    Lir(LirFoundationValidationError),
}

impl fmt::Display for FoundationStructureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hir(error) => write!(formatter, "invalid HIR foundation structure: {error}"),
            Self::Mir(error) => write!(formatter, "invalid MIR foundation structure: {error}"),
            Self::Lir(error) => write!(formatter, "invalid LIR foundation structure: {error}"),
        }
    }
}

impl std::error::Error for FoundationStructureValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Hir(error) => error,
            Self::Mir(error) => error,
            Self::Lir(error) => error,
        })
    }
}

fn write_hex(bytes: &[u8; 32], formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}
