//! Constants synthesized by LLVM while emitting a callable.

use std::collections::BTreeSet;

use object::{Object, ObjectSection, ObjectSymbol, SectionIndex};
use scoop_lir::{
    CallableConstantKind, DefinitionAtomRole, DefinitionSymbolPlanV1, LirTargetProfile,
    NativeObjectFormat, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
};

use crate::CodegenError;
use crate::elf_object::error;

pub(crate) fn atom(
    plan: &DefinitionSymbolPlanV1,
    kind: CallableConstantKind,
) -> Result<ObjectDefinitionAtomId, CodegenError> {
    ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
        plan.definition_plan(),
        DefinitionAtomRole::AddressTakenConstant,
        kind.subkey(),
    ))
    .map_err(error)
}

pub(crate) fn present(
    bytes: &[u8],
    target: LirTargetProfile,
    plan: &DefinitionSymbolPlanV1,
) -> Result<BTreeSet<CallableConstantKind>, CodegenError> {
    let file = object::File::parse(bytes).map_err(error)?;
    let mut kinds = BTreeSet::new();
    for section in file.sections() {
        let name = section.name().map_err(error)?;
        let kind = match (target.native_object_format(), name) {
            (NativeObjectFormat::MachO64, "__const")
                if section.segment_name().map_err(error)? == Some("__DATA") =>
            {
                Some(CallableConstantKind::Relocatable)
            }
            (NativeObjectFormat::Elf64, name)
                if is_section(
                    name,
                    ".data.rel.ro",
                    plan.primary_symbol().symbol().as_str(),
                ) =>
            {
                Some(CallableConstantKind::Relocatable)
            }
            (NativeObjectFormat::Coff64, ".rdata") => Some(CallableConstantKind::Relocatable),
            _ => None,
        };
        if let Some(kind) = kind.filter(|_| section.size() != 0) {
            kinds.insert(kind);
        }
    }
    if target.native_object_format() == NativeObjectFormat::Elf64
        && elf_readonly_extent(&file, plan).is_some()
    {
        kinds.insert(CallableConstantKind::ReadOnly);
    }
    Ok(kinds)
}

pub(crate) fn elf_readonly_extent<'data>(
    file: &impl Object<'data>,
    plan: &DefinitionSymbolPlanV1,
) -> Option<(SectionIndex, u64)> {
    let name = format!(".rodata.{}", plan.primary_symbol().symbol());
    let section = file
        .section_by_name(&name)
        .or_else(|| file.section_by_name(".rodata"))?;
    // LLVM emits its tables before the module's already-owned globals. A plain
    // .rodata section can contain only those globals, with no backend prefix.
    let end = plan
        .atom_boundaries()
        .iter()
        .filter_map(|boundary| file.symbol_by_name(boundary.start().symbol().as_str()))
        .filter(|symbol| symbol.section_index() == Some(section.index()))
        .map(|symbol| symbol.address())
        .min()
        .unwrap_or(section.size());
    (end != 0).then_some((section.index(), end))
}

pub(crate) fn is_section(name: &str, base: &str, callable: &str) -> bool {
    name == base
        || name
            .strip_prefix(base)
            .and_then(|suffix| suffix.strip_prefix('.'))
            == Some(callable)
}
