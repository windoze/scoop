use super::*;
use scoop_protocol::HostPathCarrier;
use serde::{Deserialize, Serialize};
use std::io::{BufReader, BufWriter, Write};

#[derive(Serialize, Deserialize)]
pub(super) struct Record {
    pub dependencies: dependencies::Dependencies,
    pub index: HostPathCarrier,
}

pub(super) fn load(
    path: &Path,
    target: &ResolvedTargetProfile,
) -> Result<Option<RuntimeBuildOutput>, RuntimeBuildError> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(error(e)),
    };
    let record: Record = serde_json::from_reader(BufReader::new(file)).map_err(error)?;
    if !record.dependencies.is_current()? {
        return Ok(None);
    }
    let index = record.index.to_path_buf().map_err(error)?;
    if !index.try_exists().map_err(error)? {
        return Ok(None);
    }
    let objects = RuntimeObjectSet::read_index(
        &index,
        target.lir_target(),
        target.c_bridge_toolchain().profile(),
    )
    .map_err(error)?;
    for object in objects.objects() {
        if !object.path().try_exists().map_err(error)? {
            return Ok(None);
        }
    }
    Ok(Some(RuntimeBuildOutput {
        objects,
        index,
        cache_hit: true,
        input_paths: record.dependencies.input_paths()?,
    }))
}

pub(super) fn save(path: &Path, record: &Record) -> Result<(), RuntimeBuildError> {
    let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(error)?;
    {
        let mut writer = BufWriter::new(&mut file);
        serde_json::to_writer(&mut writer, record).map_err(error)?;
        writer.flush().map_err(error)?;
    }
    file.persist(path).map_err(|e| error(e.error))?;
    Ok(())
}
