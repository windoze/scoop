use std::fmt;

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
        }
    }
}

impl std::error::Error for LocalValueIdentityError {}
