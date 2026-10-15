use super::ObjectOrigin;
use crate::{LinkError, error};
use scoop_wire::Digest256;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) enum ObjectBytes<'a> {
    Artifact {
        artifact: &'a scoop_slib::ProgramLinkArtifact,
        member: scoop_slib::SlibMemberId,
    },
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
        if let ObjectBytes::File { path } = &self.bytes {
            return Ok(path.clone());
        }
        let path = scoop_toolchain::object_path(cache, self.digest).map_err(error)?;
        if path.try_exists().map_err(error)? {
            return Ok(path);
        }
        let bytes = match &self.bytes {
            ObjectBytes::Artifact { artifact, member } => {
                artifact.object_bytes(*member).map_err(error)?
            }
            ObjectBytes::Native { file, range } => &file[range.clone()],
            ObjectBytes::File { path, .. } => return Ok(path.clone()),
        };
        scoop_toolchain::store_object(cache, self.digest, bytes).map_err(error)
    }
}
