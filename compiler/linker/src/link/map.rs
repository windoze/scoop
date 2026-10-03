use super::*;
use std::collections::BTreeSet;

pub(super) fn check(
    text: &str,
    objects: &[PathBuf],
    stub: &Path,
    system: &scoop_toolchain::SystemProvider,
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
            || path == stub.to_string_lossy()
            || system.reexports().contains(path)
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
