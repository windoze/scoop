//! ELF callable extents and backend contributions.

use object::{Object, ObjectSection, ObjectSymbol};

use super::*;
use crate::elf_object::{ElfObject, error};

mod personality;

pub(super) fn materialize(path: &Path, plan: &DefinitionSymbolPlanV1) -> Result<(), CodegenError> {
    let bytes = std::fs::read(path).map_err(error)?;
    let mut output = ElfObject::read(&bytes)?;
    let primary_name = plan.primary_symbol().symbol();
    let primary = output
        .file
        .symbol_by_name(primary_name.as_str())
        .ok_or_else(|| CodegenError(format!("ELF callable `{primary_name}` is missing")))?;
    let primary_section = primary
        .section_index()
        .ok_or_else(|| CodegenError("ELF callable is undefined".into()))?;
    let primary_start = primary.address();
    let primary_end = primary_start
        .checked_add(primary.size())
        .ok_or_else(|| CodegenError("ELF callable extent overflows".into()))?;
    let mut associated = vec![primary_section];
    let personality_atom = personality::atom(plan)?;
    let constants_atom = crate::callable_constants::atom(plan)?;
    let readonly_atom = crate::callable_constants::readonly_atom(plan)?;
    for boundary in plan.atom_boundaries() {
        if boundary.atom() == personality_atom {
            let section = personality::materialize(&mut output, *boundary)?;
            associated.push(section);
            continue;
        }
        let extent = if boundary.atom() == constants_atom {
            let section = output
                .file
                .section_by_name(".data.rel.ro")
                .ok_or_else(|| error("missing callable constants"))?;
            let index = section.index();
            let size = section.size();
            output.rename_section(index, ".data.rel.ro.scoop.constants")?;
            Some((index, 0, size))
        } else if boundary.atom() == readonly_atom {
            let section = output
                .file
                .section_by_name(".rodata")
                .ok_or_else(|| error("missing callable read-only constants"))?;
            let index = section.index();
            let size = section.size();
            output.rename_section(index, ".rodata.scoop.constants")?;
            Some((index, 0, size))
        } else {
            match boundary.atom_role() {
                DefinitionAtomRole::Primary => Some((primary_section, primary_start, primary_end)),
                role @ (DefinitionAtomRole::Stackmap
                | DefinitionAtomRole::Lsda
                | DefinitionAtomRole::EhFrame) => {
                    let sections = output
                        .file
                        .sections()
                        .filter(|section| section.name().ok().and_then(backend_role) == Some(role))
                        .collect::<Vec<_>>();
                    let [section] = sections.as_slice() else {
                        return Err(CodegenError(format!(
                            "ELF callable requires exactly one {role:?} contribution"
                        )));
                    };
                    if section.size() == 0 {
                        return Err(CodegenError("ELF backend atom is empty".into()));
                    }
                    Some((section.index(), 0, section.size()))
                }
                DefinitionAtomRole::CompactUnwind => {
                    return Err(CodegenError(
                        "ELF callable plan contains a Mach-O compact-unwind atom".into(),
                    ));
                }
                DefinitionAtomRole::AddressTakenConstant
                | DefinitionAtomRole::RuntimeRecord
                | DefinitionAtomRole::ContextKeyCell
                | DefinitionAtomRole::ContextKeyTable => None,
            }
        };
        if let Some((section, start, end)) = extent {
            associated.push(section);
            output.boundary(
                boundary.start().symbol().as_str(),
                section,
                start,
                boundary.start().linkage(),
            )?;
            output.boundary(
                boundary.end().symbol().as_str(),
                section,
                end,
                boundary.end().linkage(),
            )?;
        } else {
            let section = crate::atom_boundaries::elf::existing_pair(&mut output, *boundary)?;
            associated.push(section);
        }
    }
    if plan.primary_symbol().linkage() == scoop_lir::LinkageClass::OdrWeak {
        associated.sort_by_key(|section| section.0);
        associated.dedup();
        output.associate_sections(primary_name.as_str(), &associated)?;
    }
    output.relocatable_metadata()?;
    std::fs::write(path, output.finish()?).map_err(error)
}

fn backend_role(name: &str) -> Option<DefinitionAtomRole> {
    match name {
        ".llvm_stackmaps" => Some(DefinitionAtomRole::Stackmap),
        ".eh_frame" => Some(DefinitionAtomRole::EhFrame),
        ".data.DW.ref.scoop_eh_personality" => Some(DefinitionAtomRole::AddressTakenConstant),
        name if name == ".gcc_except_table" || name.starts_with(".gcc_except_table.") => {
            Some(DefinitionAtomRole::Lsda)
        }
        _ => None,
    }
}
