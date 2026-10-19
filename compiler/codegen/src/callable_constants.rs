//! LLVM lookup tables and read-only data associated with a callable.

use std::path::Path;

use object::{Object, ObjectSection};
use scoop_lir::{
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionSymbolPlanV1, LirTargetProfile,
    NativeObjectFormat, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
};

use crate::CodegenError;
use crate::elf_object::error;

#[derive(Clone, Copy, Default)]
pub(crate) struct Contributions {
    pub(crate) relocatable: bool,
    pub(crate) read_only: bool,
}

pub(crate) fn atom(plan: &DefinitionSymbolPlanV1) -> Result<ObjectDefinitionAtomId, CodegenError> {
    constant_atom(plan, DefinitionAtomSubkey::Singleton)
}

pub(crate) fn readonly_atom(
    plan: &DefinitionSymbolPlanV1,
) -> Result<ObjectDefinitionAtomId, CodegenError> {
    constant_atom(plan, DefinitionAtomSubkey::BackendReadOnlyData)
}

fn constant_atom(
    plan: &DefinitionSymbolPlanV1,
    subkey: DefinitionAtomSubkey,
) -> Result<ObjectDefinitionAtomId, CodegenError> {
    ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
        plan.definition_plan(),
        DefinitionAtomRole::AddressTakenConstant,
        subkey,
    ))
    .map_err(error)
}

pub(crate) fn contributions(
    path: &Path,
    target: LirTargetProfile,
    callable: &str,
) -> Result<Contributions, CodegenError> {
    let bytes = std::fs::read(path).map_err(error)?;
    let file = object::File::parse(bytes.as_slice()).map_err(error)?;
    let mut result = Contributions::default();
    for section in file.sections() {
        if section.size() == 0 {
            continue;
        }
        let name = section.name().map_err(error)?;
        match target.native_object_format() {
            NativeObjectFormat::MachO64 => {
                if name == "__const" && section.segment_name().map_err(error)? == Some("__DATA") {
                    result.relocatable = true;
                }
            }
            NativeObjectFormat::Elf64 => {
                result.relocatable |= is_section(name, ".data.rel.ro", callable);
                result.read_only |= is_section(name, ".rodata", callable);
            }
        }
    }
    Ok(result)
}

pub(crate) fn is_section(name: &str, base: &str, callable: &str) -> bool {
    name == base
        || name
            .strip_prefix(base)
            .and_then(|suffix| suffix.strip_prefix('.'))
            == Some(callable)
}
