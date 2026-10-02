//! Atomic user outputs, independent from immutable compile-cache entries.
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use fs4::FileExt;
use tempfile::TempPath;

pub(crate) struct OutputLocation {
    path: PathBuf,
    parent: PathBuf,
}

impl OutputLocation {
    pub(crate) fn new(path: &Path) -> io::Result<Self> {
        let path = std::path::absolute(path)?;
        let name = path
            .file_name()
            .ok_or_else(|| invalid("output has no file name"))?;
        let parent = path
            .parent()
            .ok_or_else(|| invalid("output has no parent"))?;
        std::fs::create_dir_all(parent)?;
        let parent = std::fs::canonicalize(parent)?;
        Ok(Self {
            path: parent.join(name),
            parent,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn parent(&self) -> &Path {
        &self.parent
    }

    pub(crate) fn check_inputs(&self, inputs: &[PathBuf], cache: Option<&Path>) -> io::Result<()> {
        let output_metadata = match std::fs::symlink_metadata(&self.path) {
            Ok(metadata) => {
                if !(metadata.is_file() || metadata.is_symlink()) {
                    return Err(invalid("existing output is not a regular file"));
                }
                Some(File::open(&self.path)?.metadata()?)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        let resolved = if output_metadata.is_some() {
            std::fs::canonicalize(&self.path)?
        } else {
            self.path.clone()
        };
        if let Some(cache) = cache {
            let cache = std::fs::canonicalize(cache)?;
            if self.path.starts_with(&cache) || resolved.starts_with(cache) {
                return Err(invalid("output would overwrite a cache file"));
            }
        }
        for input in inputs {
            let absolute = std::path::absolute(input)?;
            let canonical = std::fs::canonicalize(input).unwrap_or(absolute);
            let same_inode = output_metadata.as_ref().is_some_and(|output| {
                File::open(input)
                    .and_then(|file| file.metadata())
                    .is_ok_and(|input| same_file(output, &input))
            });
            if self.path == canonical || resolved == canonical || same_inode {
                return Err(invalid(format!("output aliases input {}", input.display())));
            }
        }
        Ok(())
    }

    pub(crate) fn publish_bytes(
        &self,
        bytes: &[u8],
        inputs: &[PathBuf],
        cache: Option<&Path>,
    ) -> io::Result<()> {
        scoop_process::check_interrupted()?;
        let _lock = self.lock()?;
        self.check_inputs(inputs, cache)?;
        let mut candidate = tempfile::NamedTempFile::new_in(&self.parent)?;
        candidate.write_all(bytes)?;
        candidate.as_file().sync_all()?;
        candidate.persist(&self.path).map_err(|error| error.error)?;
        File::open(&self.parent)?.sync_all()
    }

    pub(crate) fn publish_program(
        &self,
        candidate: &Path,
        inputs: &[PathBuf],
        cache: Option<&Path>,
        keep_for_run: bool,
    ) -> io::Result<Option<TempPath>> {
        scoop_process::check_interrupted()?;
        let _lock = self.lock()?;
        self.check_inputs(inputs, cache)?;
        let execution = if keep_for_run {
            Some(self.copy_program(candidate, ".scoop-run-")?)
        } else {
            None
        };
        let published = self.copy_program(candidate, ".scoop-publish-")?;
        published.persist(&self.path).map_err(|error| error.error)?;
        File::open(&self.parent)?.sync_all()?;
        Ok(execution)
    }

    fn copy_program(&self, source: &Path, prefix: &str) -> io::Result<TempPath> {
        let file = tempfile::Builder::new()
            .prefix(prefix)
            .tempfile_in(&self.parent)?;
        std::fs::copy(source, file.path())?;
        file.as_file().sync_all()?;
        Ok(file.into_temp_path())
    }

    fn lock(&self) -> io::Result<File> {
        let digest = scoop_wire::sha256(
            self.path
                .file_name()
                .expect("resolved output has a name")
                .as_encoded_bytes(),
        );
        let file = crate::cache::open_build_lock(
            &self.parent.join(format!(".scoop-output-{digest}.lock")),
        )
        .map_err(|error| io::Error::other(error.to_string()))?;
        FileExt::lock(&file)?;
        Ok(file)
    }
}

#[cfg(unix)]
fn same_file(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}
#[cfg(not(unix))]
fn same_file(_left: &std::fs::Metadata, _right: &std::fs::Metadata) -> bool {
    false
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
