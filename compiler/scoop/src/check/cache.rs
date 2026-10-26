use super::*;
use scoop_protocol::HostPathCarrier;

mod messages;
pub(super) use messages::Messages;

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Record {
    pub result: Vec<u8>,
    pub native_inputs: Vec<scoop_toolchain::NativeInputMetadata>,
    pub dependencies: Vec<HostPathCarrier>,
}

impl Record {
    pub fn success(&self) -> BuildResult<Messages> {
        scoop_wire::decode_canonical(&self.result).map_err(error)
    }
}

pub(super) struct Entry {
    directory: PathBuf,
}

impl Entry {
    pub fn new(cache: &ArtifactCacheRoot, key: &str) -> Self {
        Self {
            directory: cache.as_path().join("checks-v1").join(key),
        }
    }

    pub fn lock(&self) -> BuildResult<std::fs::File> {
        std::fs::create_dir_all(&self.directory)
            .map_err(|cause| io_error(&self.directory, cause))?;
        let path = self.directory.join("check.lock");
        let file = crate::cache::open_build_lock(&path).map_err(|cause| io_error(&path, cause))?;
        fs4::FileExt::lock(&file).map_err(|cause| io_error(&path, cause))?;
        Ok(file)
    }

    pub fn current(&self, request: &CheckRequest) -> BuildResult<Option<Messages>> {
        let path = self.directory.join("record.json");
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(cause) => return Err(io_error(&path, cause)),
        };
        let record: Record = serde_json::from_reader(std::io::BufReader::new(file))
            .map_err(|cause| io_error(&path, cause))?;
        for input in &record.native_inputs {
            if !input.is_current().map_err(error)? {
                return Ok(None);
            }
        }
        for dependency in &record.dependencies {
            let dependency = dependency.to_path_buf().map_err(error)?;
            if !dependency
                .try_exists()
                .map_err(|cause| io_error(&dependency, cause))?
            {
                return Ok(None);
            }
        }
        let result = record.success()?;
        if !request
            .generators
            .inputs_current(&result.used_generators, request.compiler.as_path())
            .map_err(error)?
        {
            return Ok(None);
        }
        Ok(Some(result))
    }

    pub fn save(&self, record: &Record) -> BuildResult<()> {
        let path = self.directory.join("record.json");
        let mut file = tempfile::NamedTempFile::new_in(&self.directory)
            .map_err(|cause| io_error(&path, cause))?;
        serde_json::to_writer(&mut file, record).map_err(|cause| io_error(&path, cause))?;
        file.persist(&path)
            .map_err(|cause| io_error(&path, cause))?;
        Ok(())
    }
}
