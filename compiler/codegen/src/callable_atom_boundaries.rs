//! Materialize stable symbols for LLVM-owned callable atoms in Mach-O.

use scoop_lir::{DefinitionAtomRole, DefinitionSymbolPlanV1, LirTargetProfile};

pub(crate) use self::macho::{BoundaryDefinitionV1, MachOLayout};
use crate::CodegenError;

mod coff;
mod elf;
mod macho;

pub(crate) fn materialize_v1(
    bytes: &mut Vec<u8>,
    target: LirTargetProfile,
    plan: &DefinitionSymbolPlanV1,
) -> Result<(), CodegenError> {
    if target.native_object_format() == scoop_lir::NativeObjectFormat::Elf64 {
        return elf::materialize(bytes, plan);
    }
    if target.native_object_format() == scoop_lir::NativeObjectFormat::Coff64 {
        return coff::materialize(bytes, plan);
    }

    let layout = MachOLayout::parse(bytes, target)?;
    let normalization = target.contract().native_symbol_normalization();
    let primary_name = normalization
        .compiler_generated_object_symbol(plan.primary_symbol().symbol().as_str())
        .into_bytes();
    let primary = layout.require_external_definition(&primary_name)?;
    let text = layout.require_section_ordinal(primary.section_ordinal())?;

    let mut additions = Vec::new();
    let constants_atom =
        crate::callable_constants::atom(plan, scoop_lir::CallableConstantKind::Relocatable)?;
    for boundary in plan.atom_boundaries() {
        let start_name = normalization
            .compiler_generated_object_symbol(boundary.start().symbol().as_str())
            .into_bytes();
        let end_name = normalization
            .compiler_generated_object_symbol(boundary.end().symbol().as_str())
            .into_bytes();
        if boundary.atom() == constants_atom {
            let section = layout.require_section(b"__DATA", b"__const")?;
            section.place_callable_constants(bytes);
            additions.push(BoundaryDefinitionV1::new(
                start_name,
                section.ordinal(),
                section.address(),
                boundary.start().linkage(),
            ));
            additions.push(BoundaryDefinitionV1::new(
                end_name,
                section.ordinal(),
                section.checked_end()?,
                boundary.end().linkage(),
            ));
            continue;
        }
        match boundary.atom_role() {
            DefinitionAtomRole::Primary => {
                additions.push(BoundaryDefinitionV1::new(
                    start_name,
                    text.ordinal(),
                    primary.value(),
                    boundary.start().linkage(),
                ));
                additions.push(BoundaryDefinitionV1::new(
                    end_name,
                    text.ordinal(),
                    text.checked_end()?,
                    boundary.end().linkage(),
                ));
            }
            role @ (DefinitionAtomRole::Stackmap
            | DefinitionAtomRole::Lsda
            | DefinitionAtomRole::EhFrame
            | DefinitionAtomRole::CompactUnwind) => {
                let (segment, section) = backend_section(role)?;
                let physical = layout.require_section(segment, section)?;
                additions.push(BoundaryDefinitionV1::new(
                    start_name,
                    physical.ordinal(),
                    physical.address(),
                    boundary.start().linkage(),
                ));
                additions.push(BoundaryDefinitionV1::new(
                    end_name,
                    physical.ordinal(),
                    physical.checked_end()?,
                    boundary.end().linkage(),
                ));
            }
            DefinitionAtomRole::AddressTakenConstant
            | DefinitionAtomRole::RuntimeRecord
            | DefinitionAtomRole::ContextKeyCell
            | DefinitionAtomRole::ContextKeyTable => {
                if plan.primary_symbol().linkage() == scoop_lir::LinkageClass::OdrWeak {
                    layout.materialize_odr_definition(bytes, &start_name)?;
                    layout.materialize_odr_definition(bytes, &end_name)?;
                }
            }
            DefinitionAtomRole::WindowsUnwindTable | DefinitionAtomRole::WindowsUnwindInfo => {
                return Err(CodegenError("Mach-O callable has COFF unwind atoms".into()));
            }
        }
    }
    layout.add_external_definitions(bytes, additions)
}

const BACKEND_SECTIONS: [(&[u8], &[u8]); 4] = [
    (b"__LLVM_STACKMAPS", b"__llvm_stackmaps"),
    (b"__TEXT", b"__gcc_except_tab"),
    (b"__TEXT", b"__eh_frame"),
    (b"__LD", b"__compact_unwind"),
];

fn backend_section(
    role: DefinitionAtomRole,
) -> Result<(&'static [u8], &'static [u8]), CodegenError> {
    let section = match role {
        DefinitionAtomRole::Stackmap => BACKEND_SECTIONS[0],
        DefinitionAtomRole::Lsda => BACKEND_SECTIONS[1],
        DefinitionAtomRole::EhFrame => BACKEND_SECTIONS[2],
        DefinitionAtomRole::CompactUnwind => BACKEND_SECTIONS[3],
        DefinitionAtomRole::Primary
        | DefinitionAtomRole::RuntimeRecord
        | DefinitionAtomRole::AddressTakenConstant
        | DefinitionAtomRole::ContextKeyCell
        | DefinitionAtomRole::ContextKeyTable
        | DefinitionAtomRole::WindowsUnwindTable
        | DefinitionAtomRole::WindowsUnwindInfo => {
            return Err(CodegenError(format!(
                "atom role {role:?} has no callable backend section"
            )));
        }
    };
    Ok(section)
}
