use std::{
    collections::BTreeSet,
    io::{self, Read},
    path::Component,
};

use super::*;

pub(super) fn extract(input: &Path, output: &Path) -> PmResult<()> {
    let file = fs::File::open(input).map_err(|error| PmError::io(input, error))?;
    let decoder =
        zstd::stream::read::Decoder::new(file).map_err(|error| PmError::io(input, error))?;
    let mut archive = tar::Archive::new(decoder);
    let mut entries = BTreeSet::new();
    for entry in archive
        .entries()
        .map_err(|error| PmError::io(input, error))?
    {
        let mut entry = entry.map_err(|error| PmError::io(input, error))?;
        let path = entry
            .path()
            .map_err(|error| PmError::io(input, error))?
            .into_owned();
        if path
            .components()
            .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
        {
            return Err(PmError::new(format!(
                "archive path escapes the package: {}",
                path.display()
            )));
        }
        let path: PathBuf = path
            .components()
            .filter_map(|part| match part {
                Component::Normal(part) => Some(part),
                _ => None,
            })
            .collect();
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err(PmError::new(format!(
                "unsupported archive entry type: {}",
                path.display()
            )));
        }
        if path.as_os_str().is_empty() && kind.is_dir() {
            continue;
        }
        if path.as_os_str().is_empty() || !entries.insert(path.clone()) {
            return Err(PmError::new(format!(
                "duplicate or empty archive path: {}",
                path.display()
            )));
        }
        let destination = output.join(&path);
        if kind.is_dir() {
            fs::create_dir_all(&destination).map_err(|error| PmError::io(&destination, error))?;
        } else {
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| PmError::io(parent, error))?;
            }
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
                .map_err(|error| PmError::io(&destination, error))?;
            io::copy(&mut entry, &mut file).map_err(|error| PmError::io(&destination, error))?;
        }
    }
    // Read the compressed stream through its checksum/trailer after the tar end marker.
    io::copy(&mut archive.into_inner(), &mut io::sink())
        .map_err(|error| PmError::io(input, error))?;
    Ok(())
}

pub(super) fn download(http: &ureq::Agent, url: &str, path: &Path, expected: &str) -> PmResult<()> {
    use sha2::{Digest, Sha256};
    use std::io::Write;

    let mut response = super::http::get(http, url)?;
    let mut input = response.body_mut().as_reader();
    let mut output = fs::File::create(path).map_err(|error| PmError::io(path, error))?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        scoop_process::check_interrupted().map_err(|error| PmError::new(error.to_string()))?;
        let length = input
            .read(&mut buffer)
            .map_err(|error| PmError::new(format!("GET {url}: {error}")))?;
        if length == 0 {
            break;
        }
        digest.update(&buffer[..length]);
        output
            .write_all(&buffer[..length])
            .map_err(|error| PmError::io(path, error))?;
    }
    if scoop_wire::Digest256::from_array(digest.finalize().into()).to_string() != expected {
        return Err(PmError::new(format!("SHA-256 mismatch downloading {url}")));
    }
    output.flush().map_err(|error| PmError::io(path, error))?;
    Ok(())
}
