//! ELF EH pointers resolve to a section and offset, never a fabricated address.

use object::{
    Object, ObjectSection, ObjectSymbol, RelocationFlags, RelocationTarget, SectionIndex, elf,
};

use crate::CodegenError;
use crate::artifact::elf::error;
use crate::target::CodeArchitecture;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ElfPoint {
    pub(super) section: SectionIndex,
    pub(super) offset: u64,
}

pub(super) fn resolve(
    file: &object::File<'_>,
    relocation: &object::Relocation,
    architecture: CodeArchitecture,
) -> Result<ElfPoint, CodegenError> {
    let expected = match architecture {
        CodeArchitecture::X86_64 => elf::R_X86_64_PC32,
        CodeArchitecture::Aarch64 => {
            return Err(CodegenError(
                "AArch64 backend selects Mach-O EH pointers".into(),
            ));
        }
    };
    if relocation.flags() != (RelocationFlags::Elf { r_type: expected })
        || relocation.has_implicit_addend()
    {
        return Err(CodegenError(format!(
            "ELF EH pointer has unexpected relocation {relocation:?}"
        )));
    }
    let RelocationTarget::Symbol(index) = relocation.target() else {
        return Err(CodegenError("ELF EH pointer has no symbol target".into()));
    };
    let symbol = file.symbol_by_index(index).map_err(error)?;
    let section = symbol.section_index().ok_or_else(|| {
        CodegenError("ELF EH pointer targets an undefined/absolute symbol".into())
    })?;
    let offset = u64::try_from(i128::from(symbol.address()) + i128::from(relocation.addend()))
        .map_err(error)?;
    if offset >= file.section_by_index(section).map_err(error)?.size() {
        return Err(CodegenError(
            "ELF EH pointer lies outside its target section".into(),
        ));
    }
    Ok(ElfPoint { section, offset })
}

pub(super) fn validate_personality(
    file: &object::File<'_>,
    point: ElfPoint,
    architecture: CodeArchitecture,
) -> Result<(), CodegenError> {
    let section = file.section_by_index(point.section).map_err(error)?;
    let start = usize::try_from(point.offset).map_err(error)?;
    let end = start
        .checked_add(8)
        .ok_or_else(|| CodegenError("ELF personality pointer extent overflows".into()))?;
    if section.data().map_err(error)?.get(start..end) != Some(&[0; 8]) {
        return Err(CodegenError(
            "ELF personality indirection has a nonzero/truncated payload".into(),
        ));
    }
    let mut relocations = section
        .relocations()
        .filter(|(offset, _)| *offset >= point.offset && *offset < point.offset + 8);
    let Some((offset, relocation)) = relocations.next() else {
        return Err(CodegenError(
            "ELF personality indirection lacks its relocation".into(),
        ));
    };
    if offset != point.offset
        || relocations.next().is_some()
        || relocation.addend() != 0
        || relocation.has_implicit_addend()
        || !matches!(
            (architecture, relocation.flags()),
            (
                CodeArchitecture::X86_64,
                RelocationFlags::Elf {
                    r_type: elf::R_X86_64_64
                }
            )
        )
    {
        return Err(CodegenError(
            "ELF personality indirection requires one absolute pointer relocation".into(),
        ));
    }
    let RelocationTarget::Symbol(index) = relocation.target() else {
        return Err(CodegenError(
            "ELF personality indirection lacks a symbol".into(),
        ));
    };
    let symbol = file.symbol_by_index(index).map_err(error)?;
    if !symbol.is_undefined()
        || symbol.name().map_err(error)?
            != scoop_lir::TargetEhSupportV1::ScoopPersonality.logical_symbol()
    {
        return Err(CodegenError(
            "ELF CIE does not reference the Scoop personality".into(),
        ));
    }
    Ok(())
}
