use super::*;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, decode_canonical};

struct Index {
    schema: u64,
    target: Vec<u8>,
    toolchain: Vec<u8>,
    configuration: RuntimeBuildConfiguration,
    abi: Digest256,
    fingerprint: Digest256,
    objects: Vec<RuntimeObject>,
    merged: NativeObjectInfo,
    deployment: super::deployment::RuntimeDeployment,
    archive: String,
}

impl RuntimeObjectSet {
    /// Read the saved link data without opening the native runtime archive.
    pub fn read_index(
        path: &Path,
        target: LirTargetProfile,
        toolchain: &CBridgeToolchainProfileV1,
    ) -> Result<Self, LinkError> {
        let path = std::path::absolute(path).map_err(error)?;
        let bytes = std::fs::read(&path).map_err(|cause| {
            error(format!(
                "runtime index {} for {}: {cause}",
                path.display(),
                target.contract().canonical_triple()
            ))
        })?;
        let index: Index = decode_canonical(&bytes).map_err(error)?;
        if index.schema != 9
            || index.target != encode(&target.wire_id()).map_err(error)?
            || index.abi.as_array() != RuntimeAbiContract.fingerprint().map_err(error)?.as_array()
            || toolchain.contract().target() != &target.wire_id()
        {
            return Err(error(
                "runtime index schema, target or ABI is incompatible; rebuild runtime",
            ));
        }
        index.deployment.check(toolchain)?;
        let directory = path
            .parent()
            .ok_or_else(|| error("runtime index has no parent"))?;
        let archive = directory.join(archive_path(&index.archive)?);
        let profile_runtime = index
            .configuration
            .enable_profiling
            .then(|| directory.join("profile.a"));
        let mut input_paths = vec![path, archive];
        input_paths.extend(profile_runtime);
        Ok(Self {
            target,
            toolchain: index.toolchain,
            deployment: index.deployment,
            configuration: index.configuration,
            objects: index.objects,
            merged: index.merged,
            fingerprint: RuntimeArtifactFingerprint(index.fingerprint),
            input_paths,
        })
    }

    pub(super) fn write_index(&self) -> Result<(), LinkError> {
        let index = Index {
            schema: 9,
            target: encode(&self.target.wire_id()).map_err(error)?,
            toolchain: self.toolchain.clone(),
            configuration: self.configuration.clone(),
            abi: Digest256::from_array(
                *RuntimeAbiContract.fingerprint().map_err(error)?.as_array(),
            ),
            fingerprint: self.fingerprint.0,
            objects: self.objects.clone(),
            merged: self.merged.clone(),
            deployment: self.deployment,
            archive: self
                .archive_path()
                .file_name()
                .expect("runtime archive filename")
                .to_str()
                .expect("generated archive name")
                .to_owned(),
        };
        std::fs::write(self.index_path(), encode(&index).map_err(error)?).map_err(error)
    }
}

fn archive_path(locator: &str) -> Result<&Path, LinkError> {
    if locator.is_empty() || locator.starts_with('/') || locator.contains(['\\', ':', '\0']) {
        return Err(error(
            "runtime index archive path must be a relative UTF-8 path with / separators",
        ));
    }
    Ok(Path::new(locator))
}

impl WireEncode for Index {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(11)?;
        e.field(1)?;
        e.unsigned(self.schema)?;
        e.field(2)?;
        e.bytes(&self.target)?;
        e.field(3)?;
        e.bytes(&self.toolchain)?;
        e.field(4)?;
        self.configuration.encode(e)?;
        e.field(5)?;
        self.abi.encode(e)?;
        e.field(6)?;
        self.fingerprint.encode(e)?;
        e.field(7)?;
        e.array(self.objects.len() as u64)?;
        for object in &self.objects {
            object.encode(e)?;
        }
        self.merged.encode_fields(e, 8)?;
        e.field(10)?;
        self.deployment.encode(e)?;
        e.field(11)?;
        e.text(&self.archive)
    }
}
impl WireDecode for Index {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(11)?;
        Ok(Self {
            schema: d.field(1, Decoder::unsigned)?,
            target: d.field(2, |d| Ok(d.bytes()?.to_vec()))?,
            toolchain: d.field(3, |d| Ok(d.bytes()?.to_vec()))?,
            configuration: d.field(4, RuntimeBuildConfiguration::decode)?,
            abi: d.field(5, Digest256::decode)?,
            fingerprint: d.field(6, Digest256::decode)?,
            objects: d.field(7, |d| d.decode_array(|d, _| RuntimeObject::decode(d)))?,
            merged: NativeObjectInfo::decode_fields(d, 8)?,
            deployment: d.field(10, super::deployment::RuntimeDeployment::decode)?,
            archive: d.field(11, |d| Ok(d.text()?.to_owned()))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_locator_is_host_independent_utf8() {
        assert_eq!(
            archive_path("../runtime/根 file.lib").unwrap(),
            Path::new("..").join("runtime").join("根 file.lib")
        );
    }

    #[test]
    fn archive_locators_reject_host_specific_or_absolute_spellings() {
        for locator in [
            "",
            "/runtime.a",
            "C:/runtime.lib",
            "runtime\\runtime.lib",
            "runtime\0.lib",
        ] {
            assert!(archive_path(locator).is_err(), "{locator:?}");
        }
    }
}
