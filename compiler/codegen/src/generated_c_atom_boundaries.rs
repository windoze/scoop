//! Materialize exact generated-C atom boundaries in native objects.

use std::collections::BTreeMap;
use std::path::Path;

use scoop_lir::{
    DefinitionSymbolPlanV1, GeneratedBridgeUnitPlanV1, LirTargetProfile, ObjectSymbolSurfaceV1,
    StrongDefinitionEntityKind,
};

use crate::CodegenError;
mod coff;
mod elf;
mod macho;

pub(crate) fn materialize_v1(
    path: &Path,
    target: LirTargetProfile,
    unit: &GeneratedBridgeUnitPlanV1,
    symbols: &ObjectSymbolSurfaceV1,
) -> Result<(), CodegenError> {
    let definitions = symbols
        .plans()
        .iter()
        .filter_map(|definition| match definition.owner().kind() {
            StrongDefinitionEntityKind::GeneratedBridgeAtom(atom) => Some((atom, definition)),
            _ => None,
        })
        .collect::<BTreeMap<_, _>>();
    let atoms = std::iter::once(unit.primary_atom_authority())
        .chain(unit.materialized_associated_atom_authorities())
        .map(|atom| {
            definitions.get(&atom.id()).copied().ok_or_else(|| {
                CodegenError(format!(
                    "generated-C unit {} has no symbol plan for atom {}",
                    unit.unit(),
                    atom.id()
                ))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bytes = std::fs::read(path).map_err(|error| {
        CodegenError(format!(
            "cannot read generated-C object {}: {error}",
            path.display()
        ))
    })?;
    let output = match target.native_object_format() {
        scoop_lir::NativeObjectFormat::MachO64 => macho::materialize(bytes, target, &atoms)?,
        scoop_lir::NativeObjectFormat::Elf64 => elf::materialize(&bytes, &atoms)?,
        scoop_lir::NativeObjectFormat::Coff64 => coff::materialize(&bytes, &atoms)?,
    };
    std::fs::write(path, output).map_err(|error| {
        CodegenError(format!(
            "cannot write generated-C boundaries to {}: {error}",
            path.display()
        ))
    })
}
