use super::*;
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

pub(super) struct Entry {
    directory: PathBuf,
}

impl Entry {
    pub fn new(cache: &ArtifactCacheRoot, key: &str) -> Self {
        Self {
            directory: cache.as_path().join("documentation-v2").join(key),
        }
    }

    pub fn input(&self) -> PathBuf {
        self.directory.join("input.json")
    }

    fn record(&self) -> PathBuf {
        self.directory.join("warnings.cbor")
    }

    pub fn ready(&self) -> BuildResult<bool> {
        if !self
            .input()
            .try_exists()
            .map_err(|e| io_error(&self.input(), e))?
            || !self
                .record()
                .try_exists()
                .map_err(|e| io_error(&self.record(), e))?
        {
            return Ok(false);
        }
        Ok(scoop_toolchain::environment_inputs_current(
            &self.read()?.environment_inputs,
        ))
    }

    pub fn lock(&self) -> BuildResult<std::fs::File> {
        std::fs::create_dir_all(&self.directory).map_err(|e| io_error(&self.directory, e))?;
        let path = self.directory.join("build.lock");
        let file = crate::cache::open_build_lock(&path).map_err(|e| io_error(&path, e))?;
        fs4::FileExt::lock(&file).map_err(|e| io_error(&path, e))?;
        Ok(file)
    }

    pub fn read(&self) -> BuildResult<Warnings> {
        let path = self.record();
        let bytes = std::fs::read(&path).map_err(|e| io_error(&path, e))?;
        scoop_wire::decode_canonical(&bytes).map_err(|e| io_error(&path, e))
    }

    pub fn save(
        &self,
        result: &scoop_protocol::ScoopcSuccessV1,
        dependencies: &[StructuredDiagnosticV1],
        dependency_environment: &[scoop_protocol::EnvironmentInputV1],
    ) -> BuildResult<()> {
        let documentation = result
            .documentation()
            .ok_or_else(|| error("compiler did not return documentation"))?;
        let mut environment_inputs = dependency_environment.to_vec();
        environment_inputs.extend_from_slice(result.environment_inputs());
        environment_inputs.sort();
        environment_inputs.dedup();
        let warnings = Warnings {
            ordinary: dependencies
                .iter()
                .chain(result.warnings())
                .cloned()
                .collect(),
            documentation: documentation.warnings().to_vec(),
            environment_inputs,
        };
        let path = self.record();
        let bytes = scoop_wire::encode(&warnings).map_err(error)?;
        let mut file =
            tempfile::NamedTempFile::new_in(&self.directory).map_err(|e| io_error(&path, e))?;
        std::io::Write::write_all(&mut file, &bytes).map_err(|e| io_error(&path, e))?;
        file.persist(&path).map_err(|e| io_error(&path, e))?;
        Ok(())
    }
}

pub(super) struct Warnings {
    pub ordinary: Vec<StructuredDiagnosticV1>,
    pub documentation: Vec<StructuredDiagnosticV1>,
    pub environment_inputs: Vec<scoop_protocol::EnvironmentInputV1>,
}

impl WireEncode for Warnings {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(3)?;
        for warnings in [&self.ordinary, &self.documentation] {
            encoder.array(warnings.len() as u64)?;
            for warning in warnings {
                warning.encode(encoder)?;
            }
        }
        encoder.array(self.environment_inputs.len() as u64)?;
        for input in &self.environment_inputs {
            input.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for Warnings {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let actual = decoder.array()?;
        if actual != 3 {
            return Err(WireError::new(
                scoop_wire::WireErrorKind::InvalidLength {
                    expected: 3,
                    actual,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        Ok(Self {
            ordinary: decoder.decode_array(|d, _| StructuredDiagnosticV1::decode(d))?,
            documentation: decoder.decode_array(|d, _| StructuredDiagnosticV1::decode(d))?,
            environment_inputs: decoder
                .decode_array(|d, _| scoop_protocol::EnvironmentInputV1::decode(d))?,
        })
    }
}
