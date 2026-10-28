use super::*;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct FileStamp {
    length: u64,
    modified: std::time::SystemTime,
}

impl FileStamp {
    pub fn optional(path: &Path) -> PmResult<Option<Self>> {
        match fs::metadata(path) {
            Ok(metadata) => Ok(Some(Self::from_metadata(path, metadata)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(PmError::io(path, error)),
        }
    }

    pub fn read(path: &Path) -> PmResult<Self> {
        let metadata = fs::metadata(path).map_err(|error| PmError::io(path, error))?;
        Self::from_metadata(path, metadata)
    }

    fn from_metadata(path: &Path, metadata: fs::Metadata) -> PmResult<Self> {
        Ok(Self {
            length: metadata.len(),
            modified: metadata
                .modified()
                .map_err(|error| PmError::io(path, error))?,
        })
    }

    pub fn unchanged(&self, path: &Path) -> PmResult<()> {
        if self != &Self::read(path)? {
            return Err(PmError::new(format!(
                "input changed while preparing packages: {}; retry the command",
                path.display()
            )));
        }
        Ok(())
    }
}

impl PackageCache {
    pub(crate) fn input_record(
        &self,
        namespace: &str,
        path: &Path,
        stamp: &FileStamp,
        extra: &[u8],
    ) -> PmResult<PathBuf> {
        let bytes = serde_json::to_vec(&(
            path.as_os_str().as_encoded_bytes(),
            stamp,
            extra,
            scoop_toolchain::VERSION,
        ))
        .map_err(|error| PmError::new(error.to_string()))?;
        Ok(self
            .root
            .join("records")
            .join(namespace)
            .join(scoop_wire::sha256(&bytes).to_string()))
    }
}
