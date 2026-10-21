use std::io::{self, Write};
use std::path::Path;
use std::rc::Rc;

use scoop_lir::ValidatedLirTargetSelection;
use scoop_slib::{
    ArtifactFile, ArtifactManifestSummaryError, ArtifactManifestSummaryV1, SlibReadError,
};
use scoop_wire::sha256;

use crate::locator::{DependencyLocatorError, LocatorIoOperation};
use crate::{ArtifactCacheRoot, BuildArtifact};

impl ArtifactCacheRoot {
    pub(crate) fn load_artifact(
        &self,
        path: &Path,
        target: ValidatedLirTargetSelection,
    ) -> Result<Rc<BuildArtifact>, DependencyLocatorError> {
        let inspect = |error| io_error(path, LocatorIoOperation::Inspect, error);
        let metadata = path.metadata().map_err(inspect)?;
        let modified = match metadata
            .modified()
            .map_err(inspect)?
            .duration_since(std::time::UNIX_EPOCH)
        {
            Ok(elapsed) => (false, elapsed),
            Err(error) => (true, error.duration()),
        };
        let target_key = target
            .target()
            .fingerprint()
            .map_err(|error| inspect(io::Error::other(error)))?;
        let backend_key = target
            .backend()
            .fingerprint()
            .map_err(|error| inspect(io::Error::other(error)))?;
        let input = serde_json::to_vec(&(
            path.as_os_str().as_encoded_bytes(),
            modified,
            metadata.len(),
            target_key.as_array(),
            backend_key.as_array(),
        ))
        .map_err(|error| inspect(error.into()))?;
        let name = sha256(&input).to_string();
        let directory = self.as_path().join("artifact-summaries-v2");
        let record = directory.join(format!("{name}.cbor"));
        if let Some(summary) = read_record(&record)? {
            return Ok(BuildArtifact::from_result(path.to_path_buf(), summary));
        }
        let write = |error| io_error(&record, LocatorIoOperation::Write, error);
        std::fs::create_dir_all(&directory).map_err(write)?;
        let lock_path = directory.join(format!("{name}.lock"));
        let lock_error = |error| io_error(&lock_path, LocatorIoOperation::Lock, error);
        let lock = super::open_build_lock(&lock_path).map_err(lock_error)?;
        fs4::FileExt::lock(&lock).map_err(lock_error)?;
        if let Some(summary) = read_record(&record)? {
            return Ok(BuildArtifact::from_result(path.to_path_buf(), summary));
        }
        let file = ArtifactFile::open(path, target)
            .map_err(|error| io_error(path, LocatorIoOperation::Open, error))?;
        let artifact =
            BuildArtifact::from_file(path.to_path_buf(), file).map_err(|source| match source {
                SlibReadError::Io(source) => io_error(path, LocatorIoOperation::Read, source),
                source => DependencyLocatorError::Summary {
                    path: path.to_path_buf(),
                    source: match source {
                        SlibReadError::TestDirectory(source) => {
                            ArtifactManifestSummaryError::TestDirectory(source)
                        }
                        source => ArtifactManifestSummaryError::Envelope(source),
                    },
                },
            })?;
        let encoded = scoop_wire::encode(artifact.summary())
            .map_err(|error| write(io::Error::other(error)))?;
        let mut output = tempfile::NamedTempFile::new_in(directory).map_err(write)?;
        output.write_all(&encoded).map_err(write)?;
        output
            .persist(&record)
            .map_err(|error| write(error.error))?;
        Ok(artifact)
    }
}

fn read_record(path: &Path) -> Result<Option<ArtifactManifestSummaryV1>, DependencyLocatorError> {
    let read = |error| io_error(path, LocatorIoOperation::Read, error);
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(read(error)),
    };
    scoop_wire::decode_canonical(&bytes)
        .map(Some)
        .map_err(|error| read(io::Error::new(io::ErrorKind::InvalidData, error)))
}

fn io_error(
    path: &Path,
    operation: LocatorIoOperation,
    source: io::Error,
) -> DependencyLocatorError {
    DependencyLocatorError::Io {
        operation,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests;
