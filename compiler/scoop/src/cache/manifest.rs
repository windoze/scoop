use std::fs::File;
use std::io::{BufReader, Error, ErrorKind};
use std::path::Path;

use scoop_manifest::{
    LoadedConeManifest, ManifestRootError, ManifestRootIoOperation, ManifestRootLocator,
    ParsedConeManifest,
};
use scoop_toolchain::SOURCE_TARGETS;
use scoop_wire::sha256;

use crate::ArtifactCacheRoot;

impl ArtifactCacheRoot {
    pub(crate) fn load_manifest(
        &self,
        locator: &ManifestRootLocator,
    ) -> Result<LoadedConeManifest, ManifestRootError> {
        let located = locator.resolve()?;
        let source_path = located.manifest_path();
        let modified = located.metadata().modified().map_err(|source| {
            ManifestRootError::io(
                ManifestRootIoOperation::Inspect,
                source_path.to_path_buf(),
                source,
            )
        })?;
        let modified = match modified.duration_since(std::time::UNIX_EPOCH) {
            Ok(elapsed) => (false, elapsed),
            Err(error) => (true, error.duration()),
        };
        let catalog: Vec<_> = SOURCE_TARGETS
            .iter()
            .map(|target| {
                (
                    target.id.canonical_name(),
                    target.id.major_version(),
                    target.triple,
                    target.os,
                    target.arch,
                    target.env,
                )
            })
            .collect();
        let input = serde_json::to_vec(&(
            source_path.as_os_str().as_encoded_bytes(),
            modified,
            located.metadata().len(),
            catalog,
        ))
        .map_err(|error| record_error(source_path, ManifestRootIoOperation::Write, error.into()))?;
        let directory = self.as_path().join("manifests-v3");
        let name = sha256(&input).to_string();
        let record_path = directory.join(format!("{name}.json"));
        if let Some((source, parsed)) = read_record(&record_path)? {
            return located.with_parsed(source, parsed);
        }

        let write_error = |error| record_error(&record_path, ManifestRootIoOperation::Write, error);
        std::fs::create_dir_all(&directory).map_err(write_error)?;
        let lock_path = directory.join(format!("{name}.lock"));
        let lock = super::open_build_lock(&lock_path)
            .map_err(|error| record_error(&lock_path, ManifestRootIoOperation::Lock, error))?;
        fs4::FileExt::lock(&lock)
            .map_err(|error| record_error(&lock_path, ManifestRootIoOperation::Lock, error))?;
        if let Some((source, parsed)) = read_record(&record_path)? {
            return located.with_parsed(source, parsed);
        }
        let manifest = located.load(SOURCE_TARGETS)?;
        let mut output = tempfile::NamedTempFile::new_in(&directory).map_err(write_error)?;
        serde_json::to_writer(&mut output, &(manifest.source_text(), manifest.parsed()))
            .map_err(|error| write_error(error.into()))?;
        output
            .persist(&record_path)
            .map_err(|error| write_error(error.error))?;
        Ok(manifest)
    }
}

fn read_record(path: &Path) -> Result<Option<(String, ParsedConeManifest)>, ManifestRootError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(record_error(path, ManifestRootIoOperation::Read, error)),
    };
    serde_json::from_reader(BufReader::new(file))
        .map(Some)
        .map_err(|error| record_error(path, ManifestRootIoOperation::Read, error.into()))
}

fn record_error(
    path: &Path,
    operation: ManifestRootIoOperation,
    source: Error,
) -> ManifestRootError {
    ManifestRootError::io(operation, path.to_path_buf(), source)
}

#[cfg(test)]
mod tests;
