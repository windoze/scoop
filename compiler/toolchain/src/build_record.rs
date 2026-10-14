//! Ordinary metadata update records for C++ discovery and native objects.
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

use scoop_wire::{Digest256, sha256};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::ToolchainError;
pub(crate) use crate::native_record::inputs::Input;

#[derive(Serialize, Deserialize)]
pub(crate) struct Record<T> {
    pub(crate) inputs: Vec<Input>,
    pub(crate) result: T,
}

pub(crate) fn cached<T: Serialize + DeserializeOwned>(
    directory: &Path,
    key: Digest256,
    available: impl Fn(&T) -> Result<bool, ToolchainError>,
    build: impl FnOnce() -> Result<Record<T>, ToolchainError>,
) -> Result<Record<T>, ToolchainError> {
    let path = directory.join(format!("{key}.json"));
    let lookup = || -> Result<Option<Record<T>>, ToolchainError> {
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(error(e)),
        };
        let record: Record<T> = serde_json::from_reader(BufReader::new(file)).map_err(error)?;
        for input in &record.inputs {
            if !input.is_current()? {
                return Ok(None);
            }
        }
        Ok(available(&record.result)?.then_some(record))
    };
    if let Some(record) = lookup()? {
        return Ok(record);
    }
    std::fs::create_dir_all(directory).map_err(error)?;
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(directory.join(format!("{key}.lock")))
        .map_err(error)?;
    fs4::FileExt::lock(&lock).map_err(error)?;
    if let Some(record) = lookup()? {
        return Ok(record);
    }
    let record = build()?;
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(error)?;
    {
        let mut writer = BufWriter::new(&mut file);
        serde_json::to_writer(&mut writer, &record).map_err(error)?;
        writer.flush().map_err(error)?;
    }
    file.persist(path).map_err(|e| error(e.error))?;
    Ok(record)
}

pub(crate) fn fingerprint(value: &impl Serialize) -> Result<Digest256, ToolchainError> {
    Ok(sha256(&serde_json::to_vec(value).map_err(error)?))
}

pub(crate) fn error(value: impl std::fmt::Display) -> ToolchainError {
    ToolchainError(format!("native build record: {value}"))
}
