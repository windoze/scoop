use super::ObjectOrigin;
use std::ops::Range;
use std::sync::Arc;

pub(crate) enum ObjectBytes<'a> {
    Borrowed(&'a [u8]),
    Native {
        file: Arc<[u8]>,
        range: Range<usize>,
    },
}

pub(crate) struct InputObject<'a> {
    pub origin: ObjectOrigin,
    pub bytes: ObjectBytes<'a>,
}
impl InputObject<'_> {
    pub fn bytes(&self) -> &[u8] {
        match &self.bytes {
            ObjectBytes::Borrowed(bytes) => bytes,
            ObjectBytes::Native { file, range } => &file[range.clone()],
        }
    }
}
