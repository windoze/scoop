use super::*;
use scoop_protocol::HostPathCarrier;
use serde::{Deserialize, Serialize};
use std::io::{BufReader, BufWriter, Write};

#[derive(Serialize, Deserialize)]
struct Record {
    inputs: Vec<NativeInputMetadata>,
    executable: HostPathCarrier,
    symbol_map: Option<HostPathCarrier>,
    #[serde(default)]
    pdb: Option<HostPathCarrier>,
    fingerprint: [u8; 32],
    result_key: [u8; 32],
}

pub(super) fn load(path: &Path) -> BuildResult<Option<Executable>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(failure(error)),
    };
    let record: Record = serde_json::from_reader(BufReader::new(file)).map_err(failure)?;
    for input in &record.inputs {
        if !input.is_current().map_err(failure)? {
            return Ok(None);
        }
    }
    let path = record.executable.to_path_buf().map_err(failure)?;
    let symbol_map = record
        .symbol_map
        .as_ref()
        .map(HostPathCarrier::to_path_buf)
        .transpose()
        .map_err(failure)?;
    let pdb = record
        .pdb
        .as_ref()
        .map(HostPathCarrier::to_path_buf)
        .transpose()
        .map_err(failure)?;
    if !path.try_exists().map_err(failure)? {
        return Ok(None);
    }
    if let Some(symbol_map) = &symbol_map
        && !symbol_map.try_exists().map_err(failure)?
    {
        return Ok(None);
    }
    if let Some(pdb) = &pdb
        && !pdb.try_exists().map_err(failure)?
    {
        return Ok(None);
    }
    Ok(Some(Executable {
        path,
        symbol_map,
        pdb,
        fingerprint: ResolvedLinkPlanFingerprint::from_array(record.fingerprint),
        result_key: record.result_key,
    }))
}

pub(super) fn save(
    path: &Path,
    executable: &Executable,
    inputs: Vec<NativeInputMetadata>,
) -> BuildResult<()> {
    let record = Record {
        inputs,
        executable: HostPathCarrier::from_path(&executable.path).map_err(failure)?,
        symbol_map: executable
            .symbol_map
            .as_deref()
            .map(HostPathCarrier::from_path)
            .transpose()
            .map_err(failure)?,
        fingerprint: *executable.fingerprint.as_array(),
        pdb: executable
            .pdb
            .as_deref()
            .map(HostPathCarrier::from_path)
            .transpose()
            .map_err(failure)?,
        result_key: executable.result_key,
    };
    let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap()).map_err(failure)?;
    {
        let mut writer = BufWriter::new(&mut file);
        serde_json::to_writer(&mut writer, &record).map_err(failure)?;
        writer.flush().map_err(failure)?;
    }
    file.persist(path).map_err(|error| failure(error.error))?;
    Ok(())
}
