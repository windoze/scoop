use super::*;
use crate::c_bridge::{GENERATED_BRIDGE_CONTEXT_SECTION, GENERATED_BRIDGE_SIGNATURE_SECTION};
use crate::callable_atom_boundaries::{BoundaryDefinitionV1, MachOLayout};

pub(super) fn materialize(
    mut bytes: Vec<u8>,
    target: LirTargetProfile,
    atoms: &[(BridgeAtomKind, &DefinitionSymbolPlanV1)],
) -> Result<Vec<u8>, CodegenError> {
    let layout = MachOLayout::parse(&bytes)?;
    let normalization = target.contract().native_symbol_normalization();
    let mut additions = Vec::with_capacity(atoms.len() * 2);
    for (kind, definition) in atoms {
        let boundary = definition.atom_boundaries()[0];
        let primary_name = normalization
            .compiler_generated_object_symbol(definition.primary_symbol().symbol().as_str())
            .into_bytes();
        let primary = layout.require_external_definition(&primary_name)?;
        let section = layout.require_section_ordinal(primary.section_ordinal())?;
        let expected_section = match kind {
            BridgeAtomKind::Entry => (b"__TEXT".as_slice(), b"__text".as_slice()),
            BridgeAtomKind::Signature => GENERATED_BRIDGE_SIGNATURE_SECTION,
            BridgeAtomKind::Context => GENERATED_BRIDGE_CONTEXT_SECTION,
        };
        let canonical_section = layout.require_section(expected_section.0, expected_section.1)?;
        if section.ordinal() != canonical_section.ordinal()
            || primary.value() != canonical_section.address()
        {
            return Err(CodegenError(format!(
                "generated-C bridge atom {} does not start its canonical section {},{}",
                definition.primary_atom(),
                String::from_utf8_lossy(expected_section.0),
                String::from_utf8_lossy(expected_section.1)
            )));
        }
        let end = canonical_section.checked_end()?;
        if !matches!(kind, BridgeAtomKind::Entry) && end != canonical_section.address() + 1 {
            return Err(CodegenError(format!(
                "generated-C descriptor atom {} is not an isolated one-byte section",
                definition.primary_atom()
            )));
        }
        for (symbol, value) in [
            (boundary.start(), canonical_section.address()),
            (boundary.end(), end),
        ] {
            additions.push(BoundaryDefinitionV1::new(
                normalization
                    .compiler_generated_object_symbol(symbol.symbol().as_str())
                    .into_bytes(),
                canonical_section.ordinal(),
                value,
                scoop_lir::LinkageClass::ConeStrong,
            ));
        }
    }
    layout.add_external_definitions(&mut bytes, additions)?;
    Ok(bytes)
}
