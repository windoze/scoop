//! Normalize emitted ELF aliases without changing their owning storage.

use object::{ObjectSymbol, SectionIndex};

use super::*;
use crate::elf_object::{ElfObject, error};

pub(super) fn materialize(
    path: &std::path::Path,
    surface: &ObjectSymbolSurfaceV1,
    definitions: &[scoop_lir::ObjectDefinitionPlanId],
) -> Result<(), CodegenError> {
    let bytes = std::fs::read(path).map_err(error)?;
    let mut output = ElfObject::read(&bytes)?;
    for definition in definitions {
        let plan = surface
            .plan(*definition)
            .ok_or_else(|| CodegenError(format!("missing ELF atom definition {definition}")))?;
        for boundary in plan.atom_boundaries() {
            existing_pair(&mut output, *boundary)?;
        }
    }
    output.relocatable_metadata()?;
    std::fs::write(path, output.finish()?).map_err(error)
}

pub(crate) fn existing_pair(
    output: &mut ElfObject<'_>,
    boundary: AtomBoundarySymbolsV1,
) -> Result<SectionIndex, CodegenError> {
    let start_name = boundary.start().symbol();
    let end_name = boundary.end().symbol();
    let start = output
        .original_symbol(start_name.as_str())
        .ok_or_else(|| CodegenError(format!("missing ELF atom start `{start_name}`")))?;
    let section = start
        .section_index()
        .ok_or_else(|| CodegenError(format!("ELF atom `{start_name}` has no section")))?;
    output.existing_boundary(start_name.as_str(), boundary.start().linkage(), false)?;
    output.existing_boundary(end_name.as_str(), boundary.end().linkage(), true)?;
    Ok(section)
}
