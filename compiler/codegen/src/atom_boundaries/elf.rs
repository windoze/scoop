//! Normalize emitted ELF aliases without changing their owning storage.

use object::{Object, ObjectSection, ObjectSymbol, SectionIndex};

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
    std::fs::write(path, output.finish()?).map_err(error)
}

pub(crate) fn existing_pair(
    output: &mut ElfObject<'_>,
    boundary: AtomBoundarySymbolsV1,
) -> Result<SectionIndex, CodegenError> {
    let start_name = boundary.start().symbol();
    let end_name = boundary.end().symbol();
    let start = output
        .file
        .symbol_by_name(start_name.as_str())
        .ok_or_else(|| CodegenError(format!("missing ELF atom start `{start_name}`")))?;
    let end = output
        .file
        .symbol_by_name(end_name.as_str())
        .ok_or_else(|| CodegenError(format!("missing ELF atom end `{end_name}`")))?;
    let section = start
        .section_index()
        .filter(|section| Some(*section) == end.section_index())
        .ok_or_else(|| CodegenError("ELF atom boundaries do not share a defined section".into()))?;
    if end.address() <= start.address()
        || end.address() > output.file.section_by_index(section).map_err(error)?.size()
    {
        return Err(CodegenError(format!(
            "ELF atom {start_name}..{end_name} has a non-positive or out-of-section extent"
        )));
    }
    output.existing_boundary(start_name.as_str(), boundary.start().linkage(), false)?;
    output.existing_boundary(end_name.as_str(), boundary.end().linkage(), true)?;
    Ok(section)
}
