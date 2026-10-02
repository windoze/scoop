use super::*;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, decode_canonical};

struct Index {
    schema: u64,
    target: Vec<u8>,
    toolchain: Vec<u8>,
    configuration: RuntimeBuildConfiguration,
    abi: Digest256,
    fingerprint: Digest256,
    objects: Vec<IndexObject>,
}

struct IndexObject {
    id: Digest256,
    length: u64,
    digest: Digest256,
    info: NativeObjectInfo,
    locator: String,
}

impl RuntimeObjectSet {
    /// Read external objects once; subsequent linking consumes these same bytes.
    pub fn read_index(
        path: &Path,
        target: LirTargetProfile,
        toolchain: &CBridgeToolchainProfileV1,
    ) -> Result<Self, LinkError> {
        let bytes = std::fs::read(path)
            .map_err(|err| error(format!("runtime index {}: {err}", path.display())))?;
        let index: Index = decode_canonical(&bytes)
            .map_err(|err| error(format!("runtime index {}: {err}", path.display())))?;
        if index.schema != 1
            || index.target != encode(&target.wire_id()).map_err(error)?
            || index.abi.as_array() != RuntimeAbiContract.fingerprint().map_err(error)?.as_array()
            || index.toolchain != encode(toolchain.contract()).map_err(error)?
        {
            return Err(error(
                "runtime index schema, target, ABI or C toolchain is incompatible; rebuild runtime",
            ));
        }
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        if index
            .objects
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
        {
            return Err(error("runtime index object IDs are not unique and sorted"));
        }
        let mut objects = Vec::with_capacity(index.objects.len());
        let mut input_paths = vec![path.to_owned()];
        for record in index.objects {
            let relative = Path::new(&record.locator);
            if relative.as_os_str().is_empty()
                || relative
                    .components()
                    .any(|part| !matches!(part, Component::Normal(_)))
            {
                return Err(error(format!(
                    "invalid runtime object locator {:?}",
                    record.locator
                )));
            }
            let path = parent.join(relative);
            input_paths.push(path.clone());
            let bytes = std::fs::read(&path)
                .map_err(|err| error(format!("runtime object {}: {err}", path.display())))?;
            let digest = sha256(&bytes);
            if bytes.len() as u64 != record.length || digest != record.digest {
                return Err(error(format!(
                    "runtime object {} length or digest mismatch",
                    record.id
                )));
            }
            let info = NativeObjectInfo::read(&bytes, toolchain.contract().deployment())
                .map_err(|err| error(format!("runtime object {}: {err}", record.id)))?;
            let id = domain_separated_cbor_hash(
                "scoop-runtime-object-v1",
                &fingerprint::ObjectKey {
                    digest,
                    info: &info,
                },
            )
            .map_err(error)?;
            if id != record.id || info != record.info {
                return Err(error(format!(
                    "runtime object {} ID or symbol records mismatch",
                    record.id
                )));
            }
            objects.push(RuntimeObject {
                id: RuntimeObjectId(id),
                bytes,
                digest,
                info,
            });
        }
        let mut set = Self::assemble(target, index.toolchain, index.configuration, objects)?;
        if set.fingerprint.0 != index.fingerprint {
            return Err(error("runtime artifact fingerprint mismatch"));
        }
        set.input_paths = input_paths;
        Ok(set)
    }

    pub fn write_index(&self, directory: &Path) -> Result<PathBuf, LinkError> {
        std::fs::create_dir_all(directory).map_err(error)?;
        let mut objects = Vec::with_capacity(self.objects.len());
        for object in &self.objects {
            let locator = format!("{}.o", object.id);
            write_new(&directory.join(&locator), &object.bytes)?;
            objects.push(IndexObject {
                id: object.id.0,
                length: object.bytes.len() as u64,
                digest: object.digest,
                info: object.info.clone(),
                locator,
            });
        }
        let index = Index {
            schema: 1,
            target: encode(&self.target.wire_id()).map_err(error)?,
            toolchain: self.toolchain.clone(),
            configuration: self.configuration.clone(),
            abi: Digest256::from_array(
                *RuntimeAbiContract.fingerprint().map_err(error)?.as_array(),
            ),
            fingerprint: self.fingerprint.0,
            objects,
        };
        let path = directory.join("index.cbor");
        write_new(&path, &encode(&index).map_err(error)?)?;
        Ok(path)
    }
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), LinkError> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(error)?;
    file.write_all(bytes).map_err(error)?;
    file.sync_all().map_err(error)
}

impl WireEncode for Index {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(7)?;
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
        Ok(())
    }
}
impl WireDecode for Index {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(7)?;
        Ok(Self {
            schema: d.field(1, Decoder::unsigned)?,
            target: d.field(2, |d| Ok(d.bytes()?.to_vec()))?,
            toolchain: d.field(3, |d| Ok(d.bytes()?.to_vec()))?,
            configuration: d.field(4, RuntimeBuildConfiguration::decode)?,
            abi: d.field(5, Digest256::decode)?,
            fingerprint: d.field(6, Digest256::decode)?,
            objects: d.field(7, |d| d.decode_array(|d, _| IndexObject::decode(d)))?,
        })
    }
}
impl WireEncode for IndexObject {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(2)?;
        e.field(1)?;
        e.map(5)?;
        e.field(1)?;
        self.id.encode(e)?;
        e.field(2)?;
        e.unsigned(self.length)?;
        e.field(3)?;
        self.digest.encode(e)?;
        self.info.encode_fields(e, 4)?;
        e.field(2)?;
        e.text(&self.locator)
    }
}
impl WireDecode for IndexObject {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(2)?;
        let (id, length, digest, info) = d.field(1, |d| {
            d.expect_map(5)?;
            Ok((
                d.field(1, Digest256::decode)?,
                d.field(2, Decoder::unsigned)?,
                d.field(3, Digest256::decode)?,
                NativeObjectInfo::decode_fields(d, 4)?,
            ))
        })?;
        let locator = d.field(2, |d| Ok(d.text()?.to_owned()))?;
        Ok(Self {
            id,
            length,
            digest,
            info,
            locator,
        })
    }
}
