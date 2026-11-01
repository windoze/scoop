use object::{Object, ObjectSection, ObjectSymbol, SectionIndex};

use super::*;
use crate::coff_object::{CoffObject, error};

pub(super) fn materialize(
    bytes: &mut Vec<u8>,
    plan: &DefinitionSymbolPlanV1,
) -> Result<(), CodegenError> {
    let mut output = CoffObject::read(bytes)?;
    let (text, start) = output.extent(plan.primary_symbol().symbol().as_str())?;
    let mut associated = vec![text];
    let lsda = output
        .file
        .symbols()
        .find(|symbol| {
            symbol
                .name_bytes()
                .is_ok_and(|name| name.starts_with(b"GCC_except_table"))
        })
        .map(|symbol| {
            Ok((
                symbol
                    .section_index()
                    .ok_or_else(|| error("undefined LSDA"))?,
                symbol.address(),
            ))
        })
        .transpose()?;
    let constants_atom =
        crate::callable_constants::atom(plan, scoop_lir::CallableConstantKind::Relocatable)?;
    for boundary in plan.atom_boundaries() {
        let extent = if boundary.atom() == constants_atom {
            let section = unique_section(&output, ".rdata")?;
            Some((
                section,
                0,
                output.file.section_by_index(section).map_err(error)?.size(),
            ))
        } else {
            match boundary.atom_role() {
                DefinitionAtomRole::Primary => Some((
                    text,
                    start,
                    output.file.section_by_index(text).map_err(error)?.size(),
                )),
                DefinitionAtomRole::Stackmap
                | DefinitionAtomRole::WindowsUnwindTable
                | DefinitionAtomRole::WindowsUnwindInfo => {
                    let name = match boundary.atom_role() {
                        DefinitionAtomRole::Stackmap => ".llvm_stackmaps",
                        DefinitionAtomRole::WindowsUnwindTable => ".pdata",
                        _ => ".xdata",
                    };
                    let section = unique_section(&output, name)?;
                    let end = if boundary.atom_role() == DefinitionAtomRole::WindowsUnwindInfo {
                        lsda.filter(|(index, _)| *index == section)
                            .map(|(_, offset)| offset)
                    } else {
                        None
                    }
                    .unwrap_or(output.file.section_by_index(section).map_err(error)?.size());
                    Some((section, 0, end))
                }
                DefinitionAtomRole::Lsda => {
                    let (section, start) = lsda.ok_or_else(|| error("missing callable LSDA"))?;
                    Some((
                        section,
                        start,
                        output.file.section_by_index(section).map_err(error)?.size(),
                    ))
                }
                DefinitionAtomRole::EhFrame | DefinitionAtomRole::CompactUnwind => {
                    return Err(error("COFF callable has Unix unwind atoms"));
                }
                DefinitionAtomRole::AddressTakenConstant
                | DefinitionAtomRole::RuntimeRecord
                | DefinitionAtomRole::ContextKeyCell
                | DefinitionAtomRole::ContextKeyTable => None,
            }
        };
        let section = if let Some((section, start, end)) = extent {
            output.boundary(boundary.start().symbol().as_str(), section, start)?;
            output.boundary(boundary.end().symbol().as_str(), section, end)?;
            section
        } else {
            output.existing_pair(*boundary)?
        };
        associated.push(section);
    }
    output.associate(text, &associated, plan.primary_symbol().linkage())?;
    *bytes = output.finish()?;
    Ok(())
}

fn unique_section(output: &CoffObject<'_>, name: &str) -> Result<SectionIndex, CodegenError> {
    let mut sections = output.file.sections().filter(|section| {
        section.size() != 0
            && section.name().is_ok_and(|actual| {
                actual == name
                    || name != ".rdata"
                        && actual
                            .strip_prefix(name)
                            .is_some_and(|suffix| suffix.starts_with('$'))
            })
    });
    let section = sections
        .next()
        .ok_or_else(|| error(format!("missing callable {name} section")))?;
    if sections.next().is_some() {
        return Err(error(format!("callable has multiple {name} contributions")));
    }
    Ok(section.index())
}
