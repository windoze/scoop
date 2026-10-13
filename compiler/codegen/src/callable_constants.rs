//! Relocatable constants synthesized by LLVM while emitting a callable.

use std::path::Path;

use object::{Object, ObjectSection};
use scoop_lir::{
    DefinitionAtomRole, DefinitionAtomSubkey, DefinitionSymbolPlanV1, LirTargetProfile,
    NativeObjectFormat, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
};

use crate::CodegenError;
use crate::elf_object::error;

pub(crate) fn atom(plan: &DefinitionSymbolPlanV1) -> Result<ObjectDefinitionAtomId, CodegenError> {
    ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
        plan.definition_plan(),
        DefinitionAtomRole::AddressTakenConstant,
        DefinitionAtomSubkey::Singleton,
    ))
    .map_err(error)
}

pub(crate) fn present(path: &Path, target: LirTargetProfile) -> Result<bool, CodegenError> {
    let bytes = std::fs::read(path).map_err(error)?;
    let file = object::File::parse(bytes.as_slice()).map_err(error)?;
    for section in file.sections() {
        let name = section.name().map_err(error)?;
        let matches = match target.native_object_format() {
            NativeObjectFormat::MachO64 => {
                name == "__const" && section.segment_name().map_err(error)? == Some("__DATA")
            }
            NativeObjectFormat::Elf64 => name == ".data.rel.ro",
        };
        if matches && section.size() != 0 {
            return Ok(true);
        }
    }
    Ok(false)
}
