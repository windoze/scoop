use std::fmt;

use scoop_identity::{SourceOriginError, SourceSpanError};

use super::{CaptureOwnerLocation, LocalValueLocation};

#[derive(Debug)]
pub enum LocalValueIdentityError {
    LocalFunctionRequiresBody {
        local_function: u32,
        function: u32,
    },
    MissingCaptureParameter {
        local_function: u32,
        capture: u32,
    },
    DuplicateCaptureParameter {
        function: u32,
        local: u32,
    },
    DuplicateSelector {
        first: LocalValueLocation,
        second: LocalValueLocation,
    },
    MissingCapturedValue {
        local_function: u32,
        capture: u32,
        binding: u32,
    },
    MissingClosureCapture {
        owner: CaptureOwnerLocation,
        capture: u32,
        binding: u32,
    },
    MissingFunctionLocal {
        function: u32,
        local: u32,
    },
    Hash {
        location: LocalValueLocation,
        reason: String,
    },
    HashCollision {
        first: LocalValueLocation,
        second: LocalValueLocation,
    },
    UnknownDefinitionSource {
        location: LocalValueLocation,
        file: u32,
    },
    DefinitionProviderMismatch {
        location: LocalValueLocation,
    },
    UnknownDefinitionContext {
        location: LocalValueLocation,
        context: u32,
    },
    DefinitionContextSourceMismatch {
        location: LocalValueLocation,
    },
    InvalidDefinitionSpan {
        location: LocalValueLocation,
        error: SourceSpanError,
    },
    InvalidDefinitionOrigin {
        location: LocalValueLocation,
        error: SourceOriginError,
    },
}

impl fmt::Display for LocalValueIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LocalFunctionRequiresBody {
                local_function,
                function,
            } => write!(
                formatter,
                "local function {local_function} targets non-body function {function}"
            ),
            Self::MissingCaptureParameter {
                local_function,
                capture,
            } => write!(
                formatter,
                "local function {local_function} capture {capture} has no ABI parameter"
            ),
            Self::DuplicateCaptureParameter { function, local } => write!(
                formatter,
                "function {function} local {local} is assigned to more than one capture"
            ),
            Self::DuplicateSelector { first, second } => write!(
                formatter,
                "local values {first:?} and {second:?} have the same owner and selector"
            ),
            Self::MissingCapturedValue {
                local_function,
                capture,
                binding,
            } => write!(
                formatter,
                "local function {local_function} capture {capture} references missing binding {binding}"
            ),
            Self::MissingClosureCapture {
                owner,
                capture,
                binding,
            } => write!(
                formatter,
                "closure {owner:?} capture {capture} references missing or ambiguous binding {binding}"
            ),
            Self::MissingFunctionLocal { function, local } => write!(
                formatter,
                "function {function} local {local} has no persistent identity"
            ),
            Self::Hash { location, reason } => {
                write!(
                    formatter,
                    "cannot identify local value {location:?}: {reason}"
                )
            }
            Self::HashCollision { first, second } => write!(
                formatter,
                "local values {first:?} and {second:?} have colliding persistent identities"
            ),
            Self::UnknownDefinitionSource { location, file } => write!(
                formatter,
                "local value {location:?} has unknown definition source file {file}"
            ),
            Self::DefinitionProviderMismatch { location } => write!(
                formatter,
                "local value {location:?} definition provider does not own its source file"
            ),
            Self::UnknownDefinitionContext { location, context } => write!(
                formatter,
                "local value {location:?} has unknown definition context {context}"
            ),
            Self::DefinitionContextSourceMismatch { location } => write!(
                formatter,
                "local value {location:?} definition context belongs to another source"
            ),
            Self::InvalidDefinitionSpan { location, error } => write!(
                formatter,
                "local value {location:?} has an invalid definition span: {error}"
            ),
            Self::InvalidDefinitionOrigin { location, error } => write!(
                formatter,
                "local value {location:?} has an invalid definition origin: {error}"
            ),
        }
    }
}

impl std::error::Error for LocalValueIdentityError {}
