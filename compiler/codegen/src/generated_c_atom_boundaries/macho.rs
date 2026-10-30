use super::*;
use crate::callable_atom_boundaries::{BoundaryDefinitionV1, MachOLayout};

pub(super) fn materialize(
    mut bytes: Vec<u8>,
    target: LirTargetProfile,
    atoms: &[&DefinitionSymbolPlanV1],
) -> Result<Vec<u8>, CodegenError> {
    let layout = MachOLayout::parse(&bytes, target)?;
    let normalization = target.contract().native_symbol_normalization();
    let mut additions = Vec::with_capacity(atoms.len() * 2);
    for definition in atoms {
        let primary_name = normalization
            .compiler_generated_object_symbol(definition.primary_symbol().symbol().as_str())
            .into_bytes();
        let primary = layout.require_external_definition(&primary_name)?;
        for boundary in definition.atom_boundaries() {
            let (section, start) = match boundary.atom_role() {
                scoop_lir::DefinitionAtomRole::Primary => (
                    layout.require_section_ordinal(primary.section_ordinal())?,
                    primary.value(),
                ),
                scoop_lir::DefinitionAtomRole::CompactUnwind => {
                    let section = layout.require_section(b"__LD", b"__compact_unwind")?;
                    (section, section.address())
                }
                scoop_lir::DefinitionAtomRole::EhFrame => {
                    let section = layout.require_section(b"__TEXT", b"__eh_frame")?;
                    (section, section.address())
                }
                role => {
                    return Err(CodegenError(format!(
                        "unexpected generated-C Mach-O atom {role:?}"
                    )));
                }
            };
            for (symbol, value) in [
                (boundary.start(), start),
                (boundary.end(), section.checked_end()?),
            ] {
                additions.push(BoundaryDefinitionV1::new(
                    normalization
                        .compiler_generated_object_symbol(symbol.symbol().as_str())
                        .into_bytes(),
                    section.ordinal(),
                    value,
                    scoop_lir::LinkageClass::ConeStrong,
                ));
            }
        }
    }
    layout.add_external_definitions(&mut bytes, additions)?;
    Ok(bytes)
}
