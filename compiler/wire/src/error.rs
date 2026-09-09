use std::fmt;

use crate::{ResourceKind, WirePath};

/// The wire kinds admitted by Wire CBOR v1.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WireType {
    Unsigned,
    Bytes,
    Text,
    Array,
    Map,
}

impl fmt::Display for WireType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unsigned => "unsigned integer",
            Self::Bytes => "byte string",
            Self::Text => "text string",
            Self::Array => "array",
            Self::Map => "map",
        })
    }
}

/// Closed, dependency-independent failures produced by the wire layer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WireErrorKind {
    UnexpectedEnd,
    WrongType {
        expected: WireType,
    },
    IndefiniteLength {
        expected: WireType,
    },
    NonCanonicalCbor,
    TrailingData,
    DuplicateField {
        field: u32,
    },
    MissingField {
        field: u32,
    },
    ExtraField {
        field: u64,
    },
    UnexpectedField {
        expected: u32,
        actual: u64,
    },
    UnknownTag {
        tag: u64,
    },
    InvalidLength {
        expected: u64,
        actual: u64,
    },
    IntegerOutOfRange,
    LimitExceeded {
        resource: ResourceKind,
        limit: u64,
        observed: u64,
    },
    ResourceAllocation {
        requested_logical_bytes: u64,
        requested_slots: u64,
    },
}

/// A structured wire failure with a stable payload path and byte offset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WireError {
    kind: WireErrorKind,
    path: WirePath,
    byte_offset: Option<u64>,
}

impl WireError {
    pub fn new(kind: WireErrorKind, path: WirePath, byte_offset: Option<u64>) -> Self {
        Self {
            kind,
            path,
            byte_offset,
        }
    }

    pub fn kind(&self) -> &WireErrorKind {
        &self.kind
    }

    pub fn path(&self) -> &WirePath {
        &self.path
    }

    pub fn byte_offset(&self) -> Option<u64> {
        self.byte_offset
    }
}

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "wire error at {}", self.path)?;
        if let Some(offset) = self.byte_offset {
            write!(formatter, " (byte {offset})")?;
        }
        write!(formatter, ": ")?;
        match &self.kind {
            WireErrorKind::UnexpectedEnd => formatter.write_str("unexpected end of input"),
            WireErrorKind::WrongType { expected } => write!(formatter, "expected {expected}"),
            WireErrorKind::IndefiniteLength { expected } => {
                write!(formatter, "indefinite-length {expected} is forbidden")
            }
            WireErrorKind::NonCanonicalCbor => formatter.write_str("non-canonical CBOR encoding"),
            WireErrorKind::TrailingData => formatter.write_str("trailing data"),
            WireErrorKind::DuplicateField { field } => write!(formatter, "duplicate field {field}"),
            WireErrorKind::MissingField { field } => write!(formatter, "missing field {field}"),
            WireErrorKind::ExtraField { field } => write!(formatter, "extra field {field}"),
            WireErrorKind::UnexpectedField { expected, actual } => {
                write!(formatter, "expected field {expected}, found {actual}")
            }
            WireErrorKind::UnknownTag { tag } => write!(formatter, "unknown tag {tag}"),
            WireErrorKind::InvalidLength { expected, actual } => {
                write!(formatter, "expected length {expected}, found {actual}")
            }
            WireErrorKind::IntegerOutOfRange => formatter.write_str("integer out of range"),
            WireErrorKind::LimitExceeded {
                resource,
                limit,
                observed,
            } => write!(
                formatter,
                "{resource} limit exceeded: limit {limit}, observed {observed}"
            ),
            WireErrorKind::ResourceAllocation {
                requested_logical_bytes,
                requested_slots,
            } => write!(
                formatter,
                "allocation failed for {requested_logical_bytes} logical bytes and {requested_slots} slots"
            ),
        }
    }
}

impl std::error::Error for WireError {}
