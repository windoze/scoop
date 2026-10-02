use super::*;
use std::collections::{BTreeMap, BTreeSet};

use crate::native_input::NativeObjectId;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(crate) struct MapSymbol {
    pub address: u64,
    pub size: u64,
}

#[derive(Default)]
pub(crate) struct LinkMap {
    pub native: BTreeMap<NativeObjectId, BTreeMap<String, Vec<MapSymbol>>>,
    pub synthesized: BTreeMap<String, Vec<MapSymbol>>,
}

enum MapOwner {
    Native(NativeObjectId),
    Synthesized,
    Other,
}

pub(super) fn check(
    text: &str,
    objects: &[PathBuf],
    stubs: &BTreeMap<PathBuf, String>,
    native: &BTreeMap<PathBuf, NativeObjectId>,
) -> Result<LinkMap, LinkError> {
    let mut expected: BTreeSet<_> = objects.iter().map(PathBuf::as_path).collect();
    let mut indices = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut section = "";
    let mut saw_objects = false;
    let mut map = LinkMap::default();
    for line in text.lines() {
        if line.starts_with("# ") {
            if matches!(
                line,
                "# Object files:" | "# Sections:" | "# Symbols:" | "# Dead Stripped Symbols:"
            ) {
                section = line;
                saw_objects |= line == "# Object files:";
            }
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        match section {
            "# Object files:" => {
                let (index, path) = indexed(line)?;
                let path = Path::new(path);
                let owner = match native.get(path) {
                    Some(id) => MapOwner::Native(*id),
                    None if path == Path::new("linker synthesized") => MapOwner::Synthesized,
                    None => MapOwner::Other,
                };
                if indices.insert(index, owner).is_some() {
                    return Err(error(format!("link map repeats object index {index}")));
                }
                let key = stubs.get(path).map_or(path, Path::new);
                if !seen.insert(key) {
                    return Err(error(format!("link map repeats input {}", path.display())));
                }
                if path == Path::new("linker synthesized")
                    || stubs.contains_key(path)
                    || stubs.values().any(|name| Path::new(name) == path)
                {
                    continue;
                }
                if !expected.remove(path) {
                    return Err(error(format!(
                        "link map contains an unexpected or duplicate input {}",
                        path.display()
                    )));
                }
            }
            "# Symbols:" => {
                let (numbers, owner) = line
                    .split_once('[')
                    .ok_or_else(|| error("invalid link map symbol row"))?;
                let mut numbers = numbers.split_whitespace();
                let address = hex(numbers.next())?;
                let size = hex(numbers.next())?;
                if numbers.next().is_some() || address.checked_add(size).is_none() {
                    return Err(error("invalid link map symbol range"));
                }
                let (index, name) = indexed_body(owner)?;
                let owner = indices.get(&index).ok_or_else(|| {
                    error(format!(
                        "link map symbol {name} has unknown object index {index}"
                    ))
                })?;
                let symbols = match owner {
                    MapOwner::Native(id) => map.native.entry(*id).or_default(),
                    MapOwner::Synthesized => &mut map.synthesized,
                    MapOwner::Other => continue,
                };
                symbols
                    .entry(name.to_owned())
                    .or_default()
                    .push(MapSymbol { address, size });
            }
            _ => {}
        }
    }
    if !saw_objects || !expected.is_empty() {
        return Err(error(format!(
            "link map omitted {} explicit objects",
            expected.len()
        )));
    }
    Ok(map)
}

fn indexed(line: &str) -> Result<(u32, &str), LinkError> {
    indexed_body(
        line.strip_prefix('[')
            .ok_or_else(|| error("invalid link map object row"))?,
    )
}
fn indexed_body(line: &str) -> Result<(u32, &str), LinkError> {
    let (index, value) = line
        .split_once(']')
        .ok_or_else(|| error("invalid link map object index"))?;
    let index = index.trim().parse().map_err(error)?;
    let value = value.trim();
    if value.is_empty() {
        return Err(error("empty link map input or symbol"));
    }
    Ok((index, value))
}
fn hex(value: Option<&str>) -> Result<u64, LinkError> {
    let value = value
        .and_then(|value| value.strip_prefix("0x"))
        .ok_or_else(|| error("invalid link map symbol address or size"))?;
    u64::from_str_radix(value, 16).map_err(error)
}

pub(super) fn trace(
    text: &str,
    objects: &[PathBuf],
    stubs: &BTreeMap<PathBuf, String>,
) -> Result<(), LinkError> {
    let expected: BTreeSet<_> = objects
        .iter()
        .chain(stubs.keys())
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    let mut seen = BTreeSet::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if !expected.contains(line) || !seen.insert(line.to_owned()) {
            return Err(error(format!(
                "link trace contains an unexpected or repeated input {line}"
            )));
        }
    }
    if seen != expected {
        return Err(error(format!(
            "link trace omitted explicit inputs: {:?}",
            expected.difference(&seen).collect::<Vec<_>>()
        )));
    }
    Ok(())
}
