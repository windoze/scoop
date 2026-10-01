//! Resolve the actual dyld export trie, which also selects weak ODR winners.
use super::dyld_cursor::Cursor;
use super::*;

pub(super) fn read(bytes: &[u8], base: u64) -> Result<BTreeMap<String, u64>, LinkError> {
    let mut exports = BTreeMap::new();
    if bytes.is_empty() {
        return Ok(exports);
    }
    let mut pending = vec![(String::new(), 0usize)];
    let mut visited = BTreeSet::new();
    while let Some((prefix, offset)) = pending.pop() {
        if !visited.insert(offset) {
            return Err(error("cyclic or repeated final export trie node"));
        }
        let mut node = Cursor::new(
            bytes
                .get(offset..)
                .ok_or_else(|| error("export trie node is outside the file"))?,
        );
        let length = usize::try_from(node.uleb()?).map_err(error)?;
        let mut terminal = Cursor::new(node.take(length)?);
        if length != 0 {
            let flags = terminal.uleb()?;
            if flags & !7 != 0 || flags & 3 == 3 {
                return Err(error(
                    "final export uses an unexpected re-export/resolver kind",
                ));
            }
            let value = terminal.uleb()?;
            let address = if flags & 3 == 2 {
                value
            } else {
                base.checked_add(value)
                    .ok_or_else(|| error("final export address overflow"))?
            };
            if prefix.is_empty()
                || !terminal.done()
                || exports.insert(prefix.clone(), address).is_some()
            {
                return Err(error("invalid or duplicate final export terminal"));
            }
        }
        let count = node.byte()?;
        let mut edges = BTreeSet::new();
        for _ in 0..count {
            let edge = node.name()?;
            if edge.is_empty() || !edges.insert(edge.clone()) {
                return Err(error("empty or duplicate final export edge"));
            }
            let child = usize::try_from(node.uleb()?).map_err(error)?;
            pending.push((format!("{prefix}{edge}"), child));
        }
    }
    Ok(exports)
}

pub(super) fn check(image: &FinalImage<'_>, inputs: &ProgramInputs<'_>) -> Result<(), LinkError> {
    let allowed = |name: &str| {
        inputs.definitions.contains_key(name)
            || matches!(name, "_main" | "_scoop_td_String" | "__mh_execute_header")
    };
    for symbol in image
        .file
        .symbols()
        .filter(|symbol| symbol.is_definition() && symbol.is_global())
    {
        let name = symbol.name().map_err(error)?;
        if !allowed(name) {
            return Err(error(format!(
                "unexpected final non-local definition {name}"
            )));
        }
    }
    for (name, address) in &image.exports {
        if !allowed(name) || *address != image.symbol(name)? {
            return Err(error(format!(
                "final export {name} does not match its controlled definition"
            )));
        }
    }
    for export in image.file.exports().map_err(error)? {
        let name = std::str::from_utf8(export.name()).map_err(error)?;
        if image.exports.get(name) != Some(&export.address()) {
            return Err(error(format!(
                "final export directory omitted or changed {name}"
            )));
        }
    }
    Ok(())
}
