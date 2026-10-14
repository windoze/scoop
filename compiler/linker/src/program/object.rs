use super::ObjectOrigin;
use crate::{LinkError, error};
use scoop_wire::{Digest256, sha256};
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
        path: &'a Path,
        digest: Digest256,
    },
}

pub(crate) struct InputObject<'a> {
    pub origin: ObjectOrigin,
    pub bytes: ObjectBytes<'a>,
}
impl InputObject<'_> {
    pub fn digest(&self) -> Digest256 {
        match &self.bytes {
            ObjectBytes::Borrowed(bytes) => sha256(bytes),
            ObjectBytes::Native { file, range } => sha256(&file[range.clone()]),
            ObjectBytes::File { digest, .. } => *digest,
        }
    }

    pub fn materialize(&self, directory: &Path) -> Result<PathBuf, LinkError> {
        let bytes = match &self.bytes {
            ObjectBytes::Borrowed(bytes) => *bytes,
            ObjectBytes::Native { file, range } => &file[range.clone()],
            ObjectBytes::File { path, .. } => return Ok(path.to_path_buf()),
        };
        let path = directory.join(format!("{}.o", self.origin));
        std::fs::write(&path, bytes).map_err(error)?;
        Ok(path)
    }
}
