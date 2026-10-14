use std::fs::{File, OpenOptions};
use std::io::ErrorKind;
use std::path::Path;
use std::time::SystemTime;

use scoop_protocol::{ScoopcMachineCapabilityV1, decode_machine_capability_frame};
use scoop_wire::{Digest256, domain_separated_cbor_hash, sha256};
use serde::{Deserialize, Serialize};

use super::{PairedCompilerError, probe_capability};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(super) struct CompilerInput {
    path: Vec<u8>,
    length: u64,
    modified: SystemTime,
}

impl CompilerInput {
    pub(super) fn read(path: &Path) -> Result<Self, PairedCompilerError> {
        let inspect = |source| PairedCompilerError::Inspect {
            path: path.to_path_buf(),
            source,
        };
        let metadata = path.metadata().map_err(inspect)?;
        Ok(Self {
            path: path.as_os_str().as_encoded_bytes().to_vec(),
            length: metadata.len(),
            modified: metadata.modified().map_err(inspect)?,
        })
    }

    pub(super) fn fingerprint(&self) -> Result<Digest256, PairedCompilerError> {
        let bytes = serde_json::to_vec(self).map_err(PairedCompilerError::RecordJson)?;
        Ok(sha256(&bytes))
    }
}

#[derive(Serialize, Deserialize)]
pub(super) struct CompilerRecord {
    input: CompilerInput,
    configuration: [u8; 32],
    capability: Vec<u8>,
}

impl CompilerRecord {
    pub(super) fn load_or_probe(
        cache_root: &Path,
        input: &CompilerInput,
        executable: &Path,
        expected: ScoopcMachineCapabilityV1,
    ) -> Result<ScoopcMachineCapabilityV1, PairedCompilerError> {
        let configuration =
            *domain_separated_cbor_hash("scoopc-capability-configuration-v1", &expected)
                .map_err(PairedCompilerError::ExpectedCapability)?
                .as_array();
        let directory = cache_root.join("tools-v1");
        let name = sha256(&input.path).to_string();
        let path = directory.join(format!("{name}.json"));
        if let Some(capability) = Self::read(&path, input, configuration)? {
            return Ok(capability);
        }

        let io_error = |source| PairedCompilerError::RecordIo {
            path: path.clone(),
            source,
        };
        std::fs::create_dir_all(&directory).map_err(io_error)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join(format!("{name}.lock")))
            .map_err(io_error)?;
        fs4::FileExt::lock(&lock).map_err(io_error)?;
        if let Some(capability) = Self::read(&path, input, configuration)? {
            return Ok(capability);
        }
        let (capability, frame) = probe_capability(executable, expected)?;
        let record = Self {
            input: input.clone(),
            configuration,
            capability: frame,
        };
        let mut output = tempfile::NamedTempFile::new_in(directory).map_err(io_error)?;
        serde_json::to_writer(&mut output, &record).map_err(PairedCompilerError::RecordJson)?;
        output
            .persist(&path)
            .map_err(|error| io_error(error.error))?;
        Ok(capability)
    }

    fn read(
        path: &Path,
        input: &CompilerInput,
        configuration: [u8; 32],
    ) -> Result<Option<ScoopcMachineCapabilityV1>, PairedCompilerError> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(PairedCompilerError::RecordIo {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        let record: Self = serde_json::from_reader(std::io::BufReader::new(file))
            .map_err(PairedCompilerError::RecordJson)?;
        if record.input != *input || record.configuration != configuration {
            return Ok(None);
        }
        decode_machine_capability_frame(&record.capability)
            .map(Some)
            .map_err(PairedCompilerError::CapabilityFrame)
    }
}
