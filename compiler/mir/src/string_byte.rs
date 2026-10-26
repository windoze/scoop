/// One byte in an established String range. Writes are restricted to private
/// construction storage; managed operands remain roots across surrounding GC.
#[derive(Debug, Clone)]
pub enum StringByteOperation<E> {
    Read { string: E, index: E },
    Write { string: E, index: E, value: E },
}

impl<E> StringByteOperation<E> {
    pub fn operands(&self) -> impl Iterator<Item = &E> {
        match self {
            Self::Read { string, index } => [Some(string), Some(index), None],
            Self::Write {
                string,
                index,
                value,
            } => [Some(string), Some(index), Some(value)],
        }
        .into_iter()
        .flatten()
    }

    pub fn operands_mut(&mut self) -> impl Iterator<Item = &mut E> {
        match self {
            Self::Read { string, index } => [Some(string), Some(index), None],
            Self::Write {
                string,
                index,
                value,
            } => [Some(string), Some(index), Some(value)],
        }
        .into_iter()
        .flatten()
    }

    pub fn map<T>(&self, mut map: impl FnMut(&E) -> T) -> StringByteOperation<T> {
        match self {
            Self::Read { string, index } => StringByteOperation::Read {
                string: map(string),
                index: map(index),
            },
            Self::Write {
                string,
                index,
                value,
            } => StringByteOperation::Write {
                string: map(string),
                index: map(index),
                value: map(value),
            },
        }
    }
}
