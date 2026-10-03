//! Facts introduced by the system linker, checked before atomic publication.
use std::collections::{BTreeMap, BTreeSet};

use object::{
    Architecture, Object, ObjectKind, ObjectSection, ObjectSymbol, macho, read::macho::MachOFile64,
};
use scoop_identity::StrongDefinitionRole;
use scoop_slib::LinkDefinitionOwnerV1;
use scoop_toolchain::ValidatedFinalLinkProfile;

use crate::{
    LinkError, error,
    program::{DefinitionOwner, ProgramInputs},
    startup::StartupObject,
};

mod commands;
mod dyld_cursor;
mod exports;
mod fixups;
mod references;
mod sections;
mod signature;
mod startup;

#[cfg(test)]
mod tests;

struct Segment {
    address: u64,
    size: u64,
    offset: u64,
    file_size: u64,
}
pub(crate) struct FinalImage<'a> {
    bytes: &'a [u8],
    file: MachOFile64<'a>,
    symbols: BTreeMap<String, Vec<u64>>,
    segments: Vec<Segment>,
    rebases: BTreeSet<u64>,
    bindings: BTreeMap<u64, fixups::Binding>,
    exports: BTreeMap<String, u64>,
}

pub(crate) fn verify(
    bytes: &[u8],
    inputs: &ProgramInputs<'_>,
    startup_object: &StartupObject,
    profile: &ValidatedFinalLinkProfile,
) -> Result<(), LinkError> {
    let file: MachOFile64<'_> = MachOFile64::parse(bytes).map_err(error)?;
    if file.kind() != ObjectKind::Executable
        || file.architecture() != Architecture::Aarch64
        || !file.is_little_endian()
        || file.macho_header().cpusubtype.get(file.endian()) != macho::CPU_SUBTYPE_ARM64_ALL
    {
        return Err(error(
            "final output is not a little-endian arm64 MH_EXECUTE",
        ));
    }
    let mut symbols: BTreeMap<String, Vec<u64>> = BTreeMap::new();
    for symbol in file.symbols().filter(|symbol| symbol.is_definition()) {
        symbols
            .entry(symbol.name().map_err(error)?.to_owned())
            .or_default()
            .push(symbol.address());
    }
    let mut image = FinalImage {
        bytes,
        file,
        symbols,
        segments: Vec::new(),
        rebases: BTreeSet::new(),
        bindings: BTreeMap::new(),
        exports: BTreeMap::new(),
    };
    image.read_commands(profile)?;
    exports::check(&image, inputs)?;
    for (symbol, owner) in &inputs.definitions {
        // Object-verifier boundaries are not program definitions. ld may
        // discard them when rebuilding unwind sections; live uses below are
        // checked against their corresponding retained atoms.
        if !matches!(
            owner,
            DefinitionOwner::Scoop(LinkDefinitionOwnerV1::VerifierBoundary { .. })
        ) {
            image.symbol(symbol)?;
        }
    }
    let mut distinct = BTreeMap::new();
    for (symbol, owner) in &inputs.definitions {
        if let DefinitionOwner::Scoop(link_owner) = owner {
            let addressable = match link_owner {
                LinkDefinitionOwnerV1::StrongDefinition(owner) => matches!(
                    owner.role(),
                    StrongDefinitionRole::CallableBody
                        | StrongDefinitionRole::StaticStorage
                        | StrongDefinitionRole::TypeDescriptor
                ),
                LinkDefinitionOwnerV1::OdrDefinition(_) => true,
                _ => false,
            };
            if addressable
                && let Some(previous) = distinct.insert(image.symbol(symbol)?, *link_owner)
                && previous != *link_owner
            {
                return Err(error(format!(
                    "different Scoop definitions share an address: {previous:?} and {link_owner:?}"
                )));
            }
        }
    }
    if image.symbol("_scoop_td_String")? != image.symbol(&inputs.string_target)? {
        return Err(error(
            "runtime String alias does not share its actual TD address",
        ));
    }
    for import in image.file.imports().map_err(error)? {
        let name = std::str::from_utf8(import.name()).map_err(error)?;
        if import.library() != profile.system_provider().install_name().as_bytes()
            || !inputs.dynamic.contains(name)
        {
            return Err(error(format!("unexpected final dynamic import {name}")));
        }
    }
    for binding in image.bindings.values() {
        if binding.weak && inputs.definitions.contains_key(&binding.symbol) {
            if image.exports.get(&binding.symbol) != Some(&image.symbol(&binding.symbol)?) {
                return Err(error(format!(
                    "weak binding {} has no matching final export",
                    binding.symbol
                )));
            }
            continue;
        }
        if binding.ordinal != 1
            || !inputs.dynamic.contains(&binding.symbol)
            || !profile
                .system_provider()
                .exports()
                .contains_key(&binding.symbol)
            || binding.symbol.starts_with("___cxa_")
            || binding.symbol.starts_with("___gxx_personality")
        {
            return Err(error(format!(
                "unexpected final binding {} from ordinal {}",
                binding.symbol, binding.ordinal
            )));
        }
    }
    let array = image.symbol(crate::startup::IMAGE_ARRAY)?;
    for (index, symbol) in inputs.images.iter().enumerate() {
        if image.pointer(array + index as u64 * 8)? != image.symbol(symbol)? {
            return Err(error(format!(
                "startup image array differs at {index}: {symbol}"
            )));
        }
    }
    startup::check(&image, startup_object, inputs)?;
    references::check(&image, inputs)?;
    sections::check(&image, inputs)?;
    Ok(())
}

impl FinalImage<'_> {
    fn symbol(&self, symbol: &str) -> Result<u64, LinkError> {
        match self.symbols.get(symbol).map(Vec::as_slice) {
            Some([address]) => Ok(*address),
            _ => Err(error(format!(
                "final symbol {symbol} is missing or has multiple definitions"
            ))),
        }
    }
    fn at(&self, address: u64, size: usize) -> Result<&[u8], LinkError> {
        for segment in &self.segments {
            if let Some(relative) = address.checked_sub(segment.address)
                && relative
                    .checked_add(size as u64)
                    .is_some_and(|end| end <= segment.file_size)
            {
                let start = segment
                    .offset
                    .checked_add(relative)
                    .and_then(|value| usize::try_from(value).ok())
                    .ok_or_else(|| error("final address/file mapping overflow"))?;
                return self
                    .bytes
                    .get(
                        start
                            ..start
                                .checked_add(size)
                                .ok_or_else(|| error("final range overflow"))?,
                    )
                    .ok_or_else(|| error("final address is outside the file"));
            }
        }
        Err(error(format!(
            "final address {address:#x} is not file-backed"
        )))
    }
    fn pointer(&self, address: u64) -> Result<u64, LinkError> {
        if let Some(binding) = self.bindings.get(&address) {
            return self
                .symbol(&binding.symbol)?
                .checked_add_signed(binding.addend)
                .ok_or_else(|| error("bound address overflow"));
        }
        let value = u64::from_le_bytes(self.at(address, 8)?.try_into().map_err(error)?);
        if !self.rebases.contains(&address)
            || !self
                .segments
                .iter()
                .any(|segment| value >= segment.address && value - segment.address <= segment.size)
        {
            return Err(error(format!(
                "missing or invalid pointer rebase at {address:#x}"
            )));
        }
        Ok(value)
    }
}
