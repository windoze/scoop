//! Resolve the actual dyld export trie, which also selects weak ODR winners.
use super::*;

pub(super) fn read(bytes: &[u8], base: u64) -> Result<BTreeMap<String, u64>, LinkError> {
    crate::macho_exports::read(bytes)?
        .into_iter()
        .map(|(name, export)| {
            let address = match export.target {
                crate::macho_exports::ExportTarget::Image { offset, .. } => base
                    .checked_add(offset)
                    .ok_or_else(|| error("final export address overflow"))?,
                crate::macho_exports::ExportTarget::Absolute(value) => value,
                crate::macho_exports::ExportTarget::Reexport { .. } => {
                    return Err(error("final export uses an unexpected re-export kind"));
                }
            };
            Ok((name, address))
        })
        .collect()
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
