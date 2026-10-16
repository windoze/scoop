use super::*;
use crate::callable_atom_boundaries::{BoundaryDefinitionV1, MachOLayout};

pub(super) fn materialize(
    mut bytes: Vec<u8>,
    target: LirTargetProfile,
    atoms: &[&DefinitionSymbolPlanV1],
) -> Result<Vec<u8>, CodegenError> {
    let layout = MachOLayout::parse(&bytes)?;
    let normalization = target.contract().native_symbol_normalization();
    let mut additions = Vec::with_capacity(atoms.len() * 2);
    for definition in atoms {
        let boundary = definition.atom_boundaries()[0];
        let primary_name = normalization
            .compiler_generated_object_symbol(definition.primary_symbol().symbol().as_str())
            .into_bytes();
        let primary = layout.require_external_definition(&primary_name)?;
        let section = layout.require_section_ordinal(primary.section_ordinal())?;
        let end = section.checked_end()?;
        for (symbol, value) in [(boundary.start(), primary.value()), (boundary.end(), end)] {
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
    layout.add_external_definitions(&mut bytes, additions)?;
    Ok(bytes)
}
