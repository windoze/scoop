//! The native provider and final executable share the actual export-trie format.
use crate::{LinkError, error, macho_cursor::Cursor};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) enum ExportTarget {
    Image { offset: u64, thread_local: bool },
    Absolute(u64),
    Reexport { ordinal: usize, imported: String },
}
pub(crate) struct Export {
    pub weak: bool,
    pub target: ExportTarget,
}

pub(crate) fn read(bytes: &[u8]) -> Result<BTreeMap<String, Export>, LinkError> {
    let mut exports = BTreeMap::new();
    if bytes.is_empty() {
        return Ok(exports);
    }
    let mut pending = vec![(String::new(), 0usize)];
    let mut visited = BTreeSet::new();
    while let Some((prefix, offset)) = pending.pop() {
        if !visited.insert(offset) {
            return Err(error("cyclic or repeated Mach-O export trie node"));
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
            if flags & !15 != 0 || flags & 3 == 3 {
                return Err(error("Mach-O export uses an unknown or resolver kind"));
            }
            let value = terminal.uleb()?;
            let target = if flags & 8 != 0 {
                let imported = terminal.name()?;
                ExportTarget::Reexport {
                    ordinal: usize::try_from(value).map_err(error)?,
                    imported: if imported.is_empty() {
                        prefix.clone()
                    } else {
                        imported
                    },
                }
            } else if flags & 3 == 2 {
                ExportTarget::Absolute(value)
            } else {
                ExportTarget::Image {
                    offset: value,
                    thread_local: flags & 3 == 1,
                }
            };
            if prefix.is_empty()
                || !terminal.done()
                || exports
                    .insert(
                        prefix.clone(),
                        Export {
                            weak: flags & 4 != 0,
                            target,
                        },
                    )
                    .is_some()
            {
                return Err(error("invalid or duplicate Mach-O export terminal"));
            }
        }
        let count = node.byte()?;
        let mut edges = BTreeSet::new();
        for _ in 0..count {
            let edge = node.name()?;
            if edge.is_empty() || !edges.insert(edge.clone()) {
                return Err(error("empty or duplicate Mach-O export edge"));
            }
            let child = usize::try_from(node.uleb()?).map_err(error)?;
            pending.push((format!("{prefix}{edge}"), child));
        }
    }
    Ok(exports)
}
