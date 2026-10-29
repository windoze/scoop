use super::*;
use std::path::{Path, PathBuf};

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, decode_canonical};

struct Index {
    schema: u64,
    target: Vec<u8>,
    toolchain: Vec<u8>,
    configuration: RuntimeBuildConfiguration,
    abi: Digest256,
    fingerprint: Digest256,
    objects: Vec<IndexObject>,
    merged: NativeObjectInfo,
    deployment: super::deployment::RuntimeDeployment,
}

struct IndexObject {
    id: RuntimeObjectId,
    digest: Digest256,
    info: NativeObjectInfo,
    locator: String,
}

impl RuntimeObjectSet {
    /// Read the saved link data; native tools consume the original object files.
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
        if index.schema != 7
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
        let objects: Vec<_> = index
            .objects
            .into_iter()
            .map(|record| {
                let relative = object_path(&record.locator)?;
                Ok(RuntimeObject {
                    id: record.id,
                    digest: record.digest,
                    info: record.info,
                    path: directory.join(relative),
                })
            })
            .collect::<Result<_, LinkError>>()?;
        let input_paths = std::iter::once(path.to_owned())
            .chain(objects.iter().map(|object| object.path.clone()))
            .collect();
        Ok(Self {
            target,
            toolchain: index.toolchain,
            deployment: index.deployment,
            configuration: index.configuration,
            objects,
            merged: index.merged,
            fingerprint: RuntimeArtifactFingerprint(index.fingerprint),
            input_paths,
        })
    }

    pub fn write_index(&self, directory: &Path) -> Result<PathBuf, LinkError> {
        std::fs::create_dir_all(directory).map_err(error)?;
        let absolute_directory = std::path::absolute(directory).map_err(error)?;
        let objects = self
            .objects
            .iter()
            .map(|object| {
                let path = std::path::absolute(&object.path).map_err(error)?;
                let relative =
                    pathdiff::diff_paths(path, &absolute_directory).ok_or_else(|| {
                        error("runtime object and index must use the same filesystem root")
                    })?;
                let locator = relative
                    .to_str()
                    .ok_or_else(|| error("runtime index object path must be UTF-8"))?;
                let locator = if cfg!(windows) {
                    locator.replace('\\', "/")
                } else {
                    locator.to_owned()
                };
                object_path(&locator)?;
                Ok(IndexObject {
                    id: object.id,
                    digest: object.digest,
                    info: object.info.clone(),
                    locator,
                })
            })
            .collect::<Result<_, LinkError>>()?;
        let index = Index {
            schema: 7,
            target: encode(&self.target.wire_id()).map_err(error)?,
            toolchain: self.toolchain.clone(),
            configuration: self.configuration.clone(),
            abi: Digest256::from_array(
                *RuntimeAbiContract.fingerprint().map_err(error)?.as_array(),
            ),
            fingerprint: self.fingerprint.0,
            objects,
            merged: self.merged.clone(),
            deployment: self.deployment,
        };
        let path = directory.join("index.cbor");
        std::fs::write(&path, encode(&index).map_err(error)?).map_err(error)?;
        Ok(path)
    }
}

fn object_path(locator: &str) -> Result<&Path, LinkError> {
    if locator.is_empty() || locator.starts_with('/') || locator.contains(['\\', ':', '\0']) {
        return Err(error(
            "runtime index object path must be a relative UTF-8 path with / separators",
        ));
    }
    Ok(Path::new(locator))
}

impl WireEncode for Index {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(10)?;
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
        self.deployment.encode(e)
    }
}
impl WireDecode for Index {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(10)?;
        Ok(Self {
            schema: d.field(1, Decoder::unsigned)?,
            target: d.field(2, |d| Ok(d.bytes()?.to_vec()))?,
            toolchain: d.field(3, |d| Ok(d.bytes()?.to_vec()))?,
            configuration: d.field(4, RuntimeBuildConfiguration::decode)?,
            abi: d.field(5, Digest256::decode)?,
            fingerprint: d.field(6, Digest256::decode)?,
            objects: d.field(7, |d| d.decode_array(|d, _| IndexObject::decode(d)))?,
            merged: NativeObjectInfo::decode_fields(d, 8)?,
            deployment: d.field(10, super::deployment::RuntimeDeployment::decode)?,
        })
    }
}
impl WireEncode for IndexObject {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(2)?;
        e.field(1)?;
        e.map(4)?;
        e.field(1)?;
        self.id.0.encode(e)?;
        e.field(2)?;
        self.digest.encode(e)?;
        self.info.encode_fields(e, 3)?;
        e.field(2)?;
        e.text(&self.locator)
    }
}
impl WireDecode for IndexObject {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(2)?;
        let (id, digest, info) = d.field(1, |d| {
            d.expect_map(4)?;
            Ok((
                RuntimeObjectId(d.field(1, Digest256::decode)?),
                d.field(2, Digest256::decode)?,
                NativeObjectInfo::decode_fields(d, 3)?,
            ))
        })?;
        let locator = d.field(2, |d| Ok(d.text()?.to_owned()))?;
        Ok(Self {
            id,
            digest,
            info,
            locator,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_locator_wire_format_is_host_independent_utf8() {
        let record = IndexObject {
            id: RuntimeObjectId(Digest256::from_array([0; 32])),
            digest: Digest256::from_array([1; 32]),
            info: NativeObjectInfo {
                definitions: BTreeMap::new(),
                requirements: BTreeSet::new(),
            },
            locator: "../objects/根 file.o".into(),
        };
        let bytes = encode(&record).unwrap();
        // Field 2 is a 21-byte CBOR text string, with no host encoding tag.
        assert!(bytes.ends_with(b"\x02\x75../objects/\xe6\xa0\xb9 file.o"));
        let decoded: IndexObject = decode_canonical(&bytes).unwrap();
        assert_eq!(
            object_path(&decoded.locator).unwrap(),
            Path::new("..").join("objects").join("根 file.o")
        );
    }

    #[test]
    fn object_locators_reject_host_specific_or_absolute_spellings() {
        for locator in [
            "",
            "/objects/a.o",
            "C:/a.o",
            "objects\\a.o",
            "objects/a\0.o",
        ] {
            assert!(object_path(locator).is_err(), "{locator:?}");
        }
    }
}
