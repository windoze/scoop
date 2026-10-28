use crate::{ArrayTypeId, TypeDescriptorRef, Value};

/// A source range whose UTF-8 and byte bounds were established by core.
/// Raw memory must remain stable across allocation; objects are relocated.
#[derive(Debug, Clone, Copy)]
pub enum StringCopySource {
    Zeroed(Value),
    Utf8 {
        pointer: Value,
        length: Value,
    },
    Slice {
        string: Value,
        start: Value,
        end: Value,
    },
    Bytes {
        array: Value,
        array_type: ArrayTypeId,
    },
    BytePrefix {
        array: Value,
        array_type: ArrayTypeId,
        length: Value,
    },
    String(Value),
}

impl StringCopySource {
    pub fn operands(self) -> impl Iterator<Item = Value> {
        match self {
            Self::Zeroed(length) => [Some(length), None, None],
            Self::Utf8 { pointer, length } => [Some(pointer), Some(length), None],
            Self::Slice { string, start, end } => [Some(string), Some(start), Some(end)],
            Self::Bytes { array, .. } | Self::String(array) => [Some(array), None, None],
            Self::BytePrefix { array, length, .. } => [Some(array), Some(length), None],
        }
        .into_iter()
        .flatten()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StringCopyTarget {
    String(TypeDescriptorRef),
    Bytes(ArrayTypeId),
}

/// Nonoverlapping source bytes for a write into an existing managed byte array.
#[derive(Debug, Clone, Copy)]
pub enum ByteCopySource {
    String(Value),
    Prefix {
        array: Value,
        array_type: ArrayTypeId,
        length: Value,
    },
    Raw {
        pointer: Value,
        length: Value,
    },
}

impl ByteCopySource {
    pub fn operands(self) -> impl Iterator<Item = Value> {
        match self {
            Self::String(string) => [Some(string), None],
            Self::Prefix { array, length, .. } => [Some(array), Some(length)],
            Self::Raw { pointer, length } => [Some(pointer), Some(length)],
        }
        .into_iter()
        .flatten()
    }
}
