//! Constants synthesized by LLVM while emitting a callable.

use std::collections::BTreeSet;
use std::path::Path;

use object::{Object, ObjectSection};
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
    path: &Path,
    target: LirTargetProfile,
    primary_symbol: &str,
) -> Result<BTreeSet<CallableConstantKind>, CodegenError> {
    let bytes = std::fs::read(path).map_err(error)?;
    let file = object::File::parse(bytes.as_slice()).map_err(error)?;
    let mut kinds = BTreeSet::new();
    for section in file.sections() {
        let name = section.name().map_err(error)?;
        let kind = match (target.native_object_format(), name) {
            (NativeObjectFormat::MachO64, "__const")
                if section.segment_name().map_err(error)? == Some("__DATA") =>
            {
                Some(CallableConstantKind::Relocatable)
            }
            (NativeObjectFormat::Elf64, ".data.rel.ro") => Some(CallableConstantKind::Relocatable),
            (NativeObjectFormat::Elf64, name) if is_elf_readonly_section(name, primary_symbol) => {
                Some(CallableConstantKind::ReadOnly)
            }
            _ => None,
        };
        if let Some(kind) = kind.filter(|_| section.size() != 0) {
            kinds.insert(kind);
        }
    }
    Ok(kinds)
}

pub(crate) fn is_elf_readonly_section(name: &str, primary_symbol: &str) -> bool {
    name == ".rodata" || name.strip_prefix(".rodata.") == Some(primary_symbol)
}
