use crate::{ArrayTypeId, TypeDescriptorRef, Value};

/// A source range whose UTF-8 and byte bounds were established by core.
/// Raw memory must remain stable across allocation; objects are relocated.
#[derive(Debug, Clone, Copy)]
pub enum StringCopySource {
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
    String(Value),
}

impl StringCopySource {
    pub fn operands(self) -> impl Iterator<Item = Value> {
        match self {
            Self::Utf8 { pointer, length } => [Some(pointer), Some(length), None],
            Self::Slice { string, start, end } => [Some(string), Some(start), Some(end)],
            Self::Bytes { array, .. } | Self::String(array) => [Some(array), None, None],
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
