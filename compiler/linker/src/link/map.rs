use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn check(
    text: &str,
    objects: &[PathBuf],
    stubs: &BTreeMap<PathBuf, String>,
) -> Result<(), LinkError> {
    let mut expected: BTreeSet<_> = objects
        .iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect();
    let mut in_objects = false;
    let mut seen = BTreeSet::new();
    for line in text.lines() {
        if line == "# Object files:" {
            in_objects = true;
            continue;
        }
        if in_objects && line.starts_with('#') {
            break;
        }
        if !in_objects {
            continue;
        }
        let Some((_, path)) = line.split_once(']') else {
            continue;
        };
        let path = path.trim();
        if !seen.insert(path) {
            return Err(error(format!("link map repeats input {path}")));
        }
        if path == "linker synthesized"
            || stubs.contains_key(Path::new(path))
            || stubs.values().any(|name| name == path)
        {
            continue;
        }
        if !expected.remove(path) {
            return Err(error(format!(
                "link map contains an unexpected or duplicate input {path}"
            )));
        }
    }
    if !in_objects || !expected.is_empty() {
        return Err(error(format!(
            "link map omitted {} explicit objects",
            expected.len()
        )));
    }
    Ok(())
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
