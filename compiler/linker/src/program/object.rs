use super::ObjectOrigin;
use crate::{LinkError, error};
use scoop_wire::Digest256;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) enum ObjectBytes<'a> {
    Borrowed(&'a [u8]),
    Native {
        file: Arc<[u8]>,
        range: Range<usize>,
    },
    File {
        path: PathBuf,
    },
}

pub(crate) struct InputObject<'a> {
    pub origin: ObjectOrigin,
    pub bytes: ObjectBytes<'a>,
    pub digest: Digest256,
}
impl InputObject<'_> {
    pub fn digest(&self) -> Digest256 {
        self.digest
    }

    pub fn path(&self, cache: &Path) -> Result<PathBuf, LinkError> {
        let bytes = match &self.bytes {
            ObjectBytes::Borrowed(bytes) => *bytes,
            ObjectBytes::Native { file, range } => &file[range.clone()],
            ObjectBytes::File { path, .. } => return Ok(path.clone()),
        };
        scoop_toolchain::store_object(cache, self.digest, bytes).map_err(error)
    }
}
