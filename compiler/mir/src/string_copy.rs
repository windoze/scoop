/// Allocation and byte copying with source evaluation order preserved.
/// The expression's result type identifies String or the exact byte array.
#[derive(Debug, Clone)]
pub enum StringCopyOperation<E> {
    Utf8 { pointer: E, length: E },
    Slice { string: E, start: E, end: E },
    FromBytes(E),
    ToBytes(E),
}

impl<E> StringCopyOperation<E> {
    pub fn operands(&self) -> impl Iterator<Item = &E> {
        match self {
            Self::Utf8 { pointer, length } => [Some(pointer), Some(length), None],
            Self::Slice { string, start, end } => [Some(string), Some(start), Some(end)],
            Self::FromBytes(value) | Self::ToBytes(value) => [Some(value), None, None],
        }
        .into_iter()
        .flatten()
    }

    pub fn operands_mut(&mut self) -> impl Iterator<Item = &mut E> {
        match self {
            Self::Utf8 { pointer, length } => [Some(pointer), Some(length), None],
            Self::Slice { string, start, end } => [Some(string), Some(start), Some(end)],
            Self::FromBytes(value) | Self::ToBytes(value) => [Some(value), None, None],
        }
        .into_iter()
        .flatten()
    }

    pub fn map<T>(&self, mut map: impl FnMut(&E) -> T) -> StringCopyOperation<T> {
        match self {
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
        }
    }
}
