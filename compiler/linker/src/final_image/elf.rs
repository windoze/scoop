//! ELF facts introduced by the final link, independent of libc selection.
use super::*;
use object::read::elf::{ElfFile64, ProgramHeader};
use object::{SymbolKind, elf};
use scoop_toolchain::{LinkMode, LinuxFinalLinkProfile};

mod stackmaps;
#[cfg(all(test, target_os = "linux", target_arch = "x86_64"))]
mod tests;

pub(crate) struct ElfImage<'a> {
    file: ElfFile64<'a>,
    symbols: BTreeMap<String, Vec<(u64, bool)>>,
    mode: LinkMode,
    relocations: BTreeMap<u64, object::Relocation>,
}

pub(crate) fn verify(
    bytes: &[u8],
    inputs: &ProgramInputs<'_>,
    profile: &LinuxFinalLinkProfile,
) -> Result<(), LinkError> {
    profile.check_image(bytes).map_err(error)?;
    let file: ElfFile64<'_> = ElfFile64::parse(bytes).map_err(error)?;
    let mut symbols: BTreeMap<String, Vec<(u64, bool)>> = BTreeMap::new();
    for symbol in file.symbols().filter(|s| {
        !s.is_undefined() && s.kind() != SymbolKind::Section && s.kind() != SymbolKind::File
    }) {
        symbols
            .entry(symbol.name().map_err(error)?.into())
            .or_default()
            .push((symbol.address(), symbol.kind() == SymbolKind::Tls));
    }
    let relocations = file.dynamic_relocations().into_iter().flatten().collect();
    let image = ElfImage {
        file,
        relocations,
        symbols,
        mode: profile.mode(),
    };
    let mut distinct = BTreeMap::new();
    for (symbol, owner) in &inputs.definitions {
        if matches!(
            owner,
            DefinitionOwner::Scoop(LinkDefinitionOwnerV1::VerifierBoundary { .. })
        ) {
            continue;
        }
        let address = image.symbol(symbol)?;
        if let DefinitionOwner::Scoop(owner) = owner {
            let addressable = match owner {
                LinkDefinitionOwnerV1::StrongDefinition(owner) => matches!(
                    owner.role(),
                    StrongDefinitionRole::CallableBody
                        | StrongDefinitionRole::StaticStorage
                        | StrongDefinitionRole::TypeDescriptor
                ),
                LinkDefinitionOwnerV1::OdrDefinition(_) => true,
                _ => false,
            };
            let tls = image.symbols[symbol][0].1;
            if addressable
                && let Some(previous) = distinct.insert((tls, address), *owner)
                && previous != *owner
            {
                return Err(error(format!(
                    "different Scoop definitions share an ELF address: {previous:?} and {owner:?}"
                )));
            }
        }
    }
    if image.symbol("scoop_td_String")? != image.symbol(&inputs.string_target)? {
        return Err(error("ELF String alias differs from its actual TD address"));
    }
    let array = image.symbol("scoop_program_images")?;
    image.read_only(array, inputs.images.len() as u64 * 8)?;
    for (index, name) in inputs.images.iter().enumerate() {
        if image.pointer(array + index as u64 * 8)? != image.symbol(name)? {
            return Err(error(format!(
                "ELF startup image array differs at {index}: {name}"
            )));
        }
    }
    for name in inputs.images.iter().chain(std::iter::once(&inputs.root)) {
        image.read_only(image.symbol(name)?, 1)?;
    }
    metadata::check(
        &inputs.final_images,
        |name| image.symbol(name),
        |address, size| image.at(address, size),
        |address| image.pointer(address),
        |address, size| image.read_only(address, size),
    )?;
    image.symbol("main")?;
    image.symbol("scoop_rt_run_program")?;
    let crate::namespace::NativeNamespace::Elf(namespace) = &inputs.namespace else {
        return Err(error("ELF output requires an ELF namespace"));
    };
    namespace.check_final(&image.file)?;
    stackmaps::check(&image, inputs)?;
    Ok(())
}

impl ElfImage<'_> {
    fn symbol(&self, name: &str) -> Result<u64, LinkError> {
        match self.symbols.get(name).map(Vec::as_slice) {
            Some([(address, _)]) => Ok(*address),
            _ => Err(error(format!(
                "ELF symbol {name} is missing or has multiple definitions"
            ))),
        }
    }

    fn at(&self, address: u64, size: usize) -> Result<&[u8], LinkError> {
        let endian = self.file.endian();
        for segment in self.file.elf_program_headers() {
            if segment.p_type(endian) != elf::PT_LOAD {
                continue;
            }
            if let Some(offset) = address.checked_sub(segment.p_vaddr(endian))
                && offset
                    .checked_add(size as u64)
                    .is_some_and(|end| end <= segment.p_filesz(endian))
            {
                let start = usize::try_from(
                    segment
                        .p_offset(endian)
                        .checked_add(offset)
                        .ok_or_else(|| error("ELF file offset overflow"))?,
                )
                .map_err(error)?;
                return self
                    .file
                    .data()
                    .get(
                        start
                            ..start
                                .checked_add(size)
                                .ok_or_else(|| error("ELF file range overflow"))?,
                    )
                    .ok_or_else(|| error("ELF file mapping exceeds input"));
            }
        }
        Err(error(format!(
            "ELF address {address:#x} is not file-backed"
        )))
    }

    fn pointer(&self, address: u64) -> Result<u64, LinkError> {
        let value = u64::from_le_bytes(self.at(address, 8)?.try_into().map_err(error)?);
        if self.mode == LinkMode::Dynamic {
            let relocation = self
                .relocations
                .get(&address)
                .ok_or_else(|| error(format!("ELF PIE pointer {address:#x} has no relocation")))?;
            if relocation.flags()
                != (object::RelocationFlags::Elf {
                    r_type: elf::R_X86_64_RELATIVE,
                })
                || relocation.addend() as u64 != value
            {
                return Err(error(format!(
                    "ELF PIE pointer {address:#x} has an unexpected binding"
                )));
            }
        }
        Ok(value)
    }

    fn read_only(&self, address: u64, size: u64) -> Result<(), LinkError> {
        let endian = self.file.endian();
        let covers = |segment: &object::elf::ProgramHeader64<object::Endianness>| {
            address
                .checked_sub(segment.p_vaddr(endian))
                .and_then(|offset| offset.checked_add(size))
                .is_some_and(|end| end <= segment.p_memsz(endian))
        };
        let load = self
            .file
            .elf_program_headers()
            .iter()
            .find(|s| s.p_type(endian) == elf::PT_LOAD && covers(s))
            .ok_or_else(|| error("ELF metadata is outside PT_LOAD"))?;
        if load.p_flags(endian) & elf::PF_X != 0 || load.p_flags(endian) & elf::PF_R == 0 {
            return Err(error(
                "ELF metadata lacks a readable, non-executable mapping",
            ));
        }
        if load.p_flags(endian) & elf::PF_W != 0
            && !(self.mode == LinkMode::Dynamic
                && self
                    .file
                    .elf_program_headers()
                    .iter()
                    .any(|s| s.p_type(endian) == elf::PT_GNU_RELRO && covers(s)))
        {
            return Err(error("ELF metadata remains writable after relocation"));
        }
        Ok(())
    }
}
