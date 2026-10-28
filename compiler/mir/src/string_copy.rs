/// String and byte-buffer copying with source evaluation order preserved.
/// Allocating operations return a String or exact array; writes return Unit.
#[derive(Debug, Clone)]
pub enum StringCopyOperation<E> {
    Zeroed(E),
    Utf8 {
        pointer: E,
        length: E,
    },
    Slice {
        string: E,
        start: E,
        end: E,
    },
    FromBytes(E),
    ToBytes(E),
    FromBytePrefix {
        array: E,
        length: E,
    },
    StringIntoBytes {
        string: E,
        target: E,
        start: E,
    },
    BytePrefixIntoBytes {
        source: E,
        target: E,
        length: E,
    },
    RawIntoBytes {
        pointer: E,
        length: E,
        target: E,
        start: E,
    },
}

impl<E> StringCopyOperation<E> {
    pub fn operands(&self) -> impl Iterator<Item = &E> {
        match self {
            Self::Zeroed(length) => [Some(length), None, None, None],
            Self::Utf8 { pointer, length } => [Some(pointer), Some(length), None, None],
            Self::Slice { string, start, end } => [Some(string), Some(start), Some(end), None],
            Self::FromBytes(value) | Self::ToBytes(value) => [Some(value), None, None, None],
            Self::FromBytePrefix { array, length } => [Some(array), Some(length), None, None],
            Self::StringIntoBytes {
                string,
                target,
                start,
            } => [Some(string), Some(target), Some(start), None],
            Self::BytePrefixIntoBytes {
                source,
                target,
                length,
            } => [Some(source), Some(target), Some(length), None],
            Self::RawIntoBytes {
                pointer,
                length,
                target,
                start,
            } => [Some(pointer), Some(length), Some(target), Some(start)],
        }
        .into_iter()
        .flatten()
    }

    pub fn operands_mut(&mut self) -> impl Iterator<Item = &mut E> {
        match self {
            Self::Zeroed(length) => [Some(length), None, None, None],
            Self::Utf8 { pointer, length } => [Some(pointer), Some(length), None, None],
            Self::Slice { string, start, end } => [Some(string), Some(start), Some(end), None],
            Self::FromBytes(value) | Self::ToBytes(value) => [Some(value), None, None, None],
            Self::FromBytePrefix { array, length } => [Some(array), Some(length), None, None],
            Self::StringIntoBytes {
                string,
                target,
                start,
            } => [Some(string), Some(target), Some(start), None],
            Self::BytePrefixIntoBytes {
                source,
                target,
                length,
            } => [Some(source), Some(target), Some(length), None],
            Self::RawIntoBytes {
                pointer,
                length,
                target,
                start,
            } => [Some(pointer), Some(length), Some(target), Some(start)],
        }
        .into_iter()
        .flatten()
    }

    pub fn map<T>(&self, mut map: impl FnMut(&E) -> T) -> StringCopyOperation<T> {
        match self {
            Self::Zeroed(length) => StringCopyOperation::Zeroed(map(length)),
            Self::Utf8 { pointer, length } => StringCopyOperation::Utf8 {
                pointer: map(pointer),
                length: map(length),
            },
            Self::Slice { string, start, end } => StringCopyOperation::Slice {
                string: map(string),
                start: map(start),
                end: map(end),
            },
            Self::FromBytes(value) => StringCopyOperation::FromBytes(map(value)),
            Self::ToBytes(value) => StringCopyOperation::ToBytes(map(value)),
            Self::FromBytePrefix { array, length } => StringCopyOperation::FromBytePrefix {
                array: map(array),
                length: map(length),
            },
            Self::StringIntoBytes {
                string,
                target,
                start,
            } => StringCopyOperation::StringIntoBytes {
                string: map(string),
                target: map(target),
                start: map(start),
            },
            Self::BytePrefixIntoBytes {
                source,
                target,
                length,
            } => StringCopyOperation::BytePrefixIntoBytes {
                source: map(source),
                target: map(target),
                length: map(length),
            },
            Self::RawIntoBytes {
                pointer,
                length,
                target,
                start,
            } => StringCopyOperation::RawIntoBytes {
                pointer: map(pointer),
                length: map(length),
                target: map(target),
                start: map(start),
            },
        }
    }
}
