//! Materialize stable symbols for LLVM-owned callable atoms in Mach-O.

use std::collections::BTreeSet;
use std::path::Path;

use scoop_lir::{
    DefinitionAtomRole, DefinitionSymbolPlanV1, LirTargetProfile, PersistentCallableBodyId,
    StrongDefinitionEntityKind,
};

pub(crate) use self::macho::{BoundaryDefinitionV1, MachOLayout};
use crate::CodegenError;

mod macho;

pub(crate) fn materialize_v1(
    path: &Path,
    target: LirTargetProfile,
    plan: &DefinitionSymbolPlanV1,
    expected_body: PersistentCallableBodyId,
) -> Result<(), CodegenError> {
    if plan.definition_role() != scoop_lir::StrongDefinitionRole::CallableBody
        || plan.owner().kind() != StrongDefinitionEntityKind::CallableBody(expected_body)
    {
        return Err(CodegenError(format!(
            "definition {} is not the selected callable body {expected_body}",
            plan.definition_plan()
        )));
    }

    let mut bytes = std::fs::read(path).map_err(|error| {
        CodegenError(format!(
            "cannot read callable object {} for atom boundary materialization: {error}",
            path.display()
        ))
    })?;
    let layout = MachOLayout::parse(&bytes)?;
    let normalization = target.contract().native_symbol_normalization();
    let primary_name = normalization
        .compiler_generated_object_symbol(plan.primary_symbol().symbol().as_str())
        .into_bytes();
    let primary = layout.require_external_definition(&primary_name)?;
    let text = layout.require_section(b"__TEXT", b"__text")?;
    if primary.section_ordinal() != text.ordinal() || primary.value() != text.address() {
        return Err(CodegenError(format!(
            "callable primary `{}` does not start its unique __text section",
            String::from_utf8_lossy(&primary_name)
        )));
    }

    let mut additions = Vec::new();
    let mut planned_backend_sections = BTreeSet::new();
    for boundary in plan.atom_boundaries() {
        let start_name = normalization
            .compiler_generated_object_symbol(boundary.start().symbol().as_str())
            .into_bytes();
        let end_name = normalization
            .compiler_generated_object_symbol(boundary.end().symbol().as_str())
            .into_bytes();
        match boundary.atom_role() {
            DefinitionAtomRole::Primary => {
                if boundary.atom() != plan.primary_atom() {
                    return Err(CodegenError(format!(
                        "callable definition {} assigns Primary to a non-primary atom",
                        plan.definition_plan()
                    )));
                }
                additions.push(BoundaryDefinitionV1::new(
                    start_name,
                    text.ordinal(),
                    text.address(),
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
                planned_backend_sections.insert((segment, section));
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
                    layout.materialize_odr_definition(&mut bytes, &start_name)?;
                    layout.materialize_odr_definition(&mut bytes, &end_name)?;
                }
                layout
                    .require_existing_boundary_pair(&start_name, &end_name)
                    .map_err(|error| {
                        CodegenError(format!(
                            "callable atom {} ({:?}) has invalid materialized boundaries: {error}",
                            boundary.atom(),
                            boundary.atom_role()
                        ))
                    })?;
            }
        }
    }
    for (segment, section) in BACKEND_SECTIONS {
        if layout.find_section(segment, section)?.is_some()
            && !planned_backend_sections.contains(&(segment, section))
        {
            return Err(CodegenError(format!(
                "callable object contains unplanned backend section {},{}",
                String::from_utf8_lossy(segment),
                String::from_utf8_lossy(section)
            )));
        }
    }
    layout.add_external_definitions(&mut bytes, additions)?;
    std::fs::write(path, bytes).map_err(|error| {
        CodegenError(format!(
            "cannot write callable atom boundaries to {}: {error}",
            path.display()
        ))
    })
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
        | DefinitionAtomRole::ContextKeyTable => {
            return Err(CodegenError(format!(
                "atom role {role:?} has no callable backend section"
            )));
        }
    };
    Ok(section)
}
