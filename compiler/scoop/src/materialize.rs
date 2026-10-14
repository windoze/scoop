//! Publish user outputs once for each completed build result.
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use fs4::FileExt;

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

    pub(crate) fn publish(&self, source: &Path, result: Option<&[u8; 32]>) -> io::Result<()> {
        scoop_process::check_interrupted()?;
        let _lock = self.lock()?;
        let record = self.record_path();
        if let Some(result) = result {
            let previous = match std::fs::read(&record) {
                Ok(bytes) => Some(bytes),
                Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                Err(error) => return Err(error),
            };
            if previous.as_deref() == Some(result) && self.path.try_exists()? {
                return Ok(());
            }
        }
        let candidate = tempfile::NamedTempFile::new_in(&self.parent)?;
        std::fs::copy(source, candidate.path())?;
        candidate.persist(&self.path).map_err(|error| error.error)?;
        if let Some(result) = result {
            let mut candidate = tempfile::NamedTempFile::new_in(&self.parent)?;
            candidate.write_all(result)?;
            candidate.persist(record).map_err(|error| error.error)?;
        } else {
            match std::fs::remove_file(record) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn record_path(&self) -> PathBuf {
        let key = scoop_wire::sha256(self.path.as_os_str().as_encoded_bytes());
        self.parent.join(format!(".scoop-output-{key}.record"))
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

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
