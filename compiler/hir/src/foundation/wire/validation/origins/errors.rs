use std::fmt;

use scoop_identity::{ConeIdentity, DefinitionOriginSubject, SourceIdentity};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DefinitionOriginValidationError {
    DuplicateRequirement {
        subject: DefinitionOriginSubject,
    },
    DuplicateSubject {
        subject: DefinitionOriginSubject,
    },
    MissingSubject {
        subject: DefinitionOriginSubject,
    },
    UnexpectedSubject {
        subject: DefinitionOriginSubject,
    },
    MissingSourceAnchor {
        subject: DefinitionOriginSubject,
    },
    SourceConeMismatch {
        subject: DefinitionOriginSubject,
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    SourceMismatch {
        subject: DefinitionOriginSubject,
        expected: Box<SourceIdentity>,
        actual: Box<SourceIdentity>,
    },
    UnknownSource {
        subject: DefinitionOriginSubject,
        source: Box<SourceIdentity>,
    },
    MissingPoint {
        subject: DefinitionOriginSubject,
        source: Box<SourceIdentity>,
        byte_offset: u64,
    },
}

impl fmt::Display for DefinitionOriginValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateRequirement { subject } => {
                write!(
                    formatter,
                    "HIR identity graph repeats origin subject {subject:?}"
                )
            }
            Self::DuplicateSubject { subject } => {
                write!(
                    formatter,
                    "HIR definition origins repeat subject {subject:?}"
                )
            }
            Self::MissingSubject { subject } => {
                write!(
                    formatter,
                    "HIR definition origin is missing for {subject:?}"
                )
            }
            Self::UnexpectedSubject { subject } => {
                write!(
                    formatter,
                    "HIR definition origin is not allowed for {subject:?}"
                )
            }
            Self::MissingSourceAnchor { subject } => write!(
                formatter,
                "HIR definition origin for {subject:?} has no unique source-backed owner"
            ),
            Self::SourceConeMismatch {
                subject,
                expected,
                actual,
            } => write!(
                formatter,
                "HIR definition origin for {subject:?} belongs to Cone {actual}, expected {expected}"
            ),
            Self::SourceMismatch {
                subject,
                expected,
                actual,
            } => write!(
                formatter,
                "HIR definition origin for {subject:?} uses source {actual:?}, expected {expected:?}"
            ),
            Self::UnknownSource { subject, source } => write!(
                formatter,
                "HIR definition origin for {subject:?} refers to absent source {source:?}"
            ),
            Self::MissingPoint {
                subject,
                source,
                byte_offset,
            } => write!(
                formatter,
                "HIR definition origin for {subject:?} refers to missing point {byte_offset} in {source:?}"
            ),
        }
    }
}

impl std::error::Error for DefinitionOriginValidationError {}
