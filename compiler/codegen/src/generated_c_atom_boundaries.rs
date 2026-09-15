//! Materialize exact generated-C atom boundaries in canonical Mach-O objects.

use std::collections::BTreeMap;
use std::path::Path;

use scoop_lir::{
    DefinitionAtomRole, GeneratedBridgeAtomId, GeneratedBridgeAtomRoleKey,
    GeneratedBridgeUnitPlanV1, LirTargetProfile, StrongDefinitionEntityKind, StrongDefinitionRole,
    StrongDefinitionSymbolPlanV1, StrongObjectSymbolSurfaceV1,
};

use crate::CodegenError;
use crate::c_bridge::{GENERATED_BRIDGE_CONTEXT_SECTION, GENERATED_BRIDGE_SIGNATURE_SECTION};
use crate::callable_atom_boundaries::{BoundaryDefinitionV1, MachOLayout};

pub(crate) fn materialize_v1(
    path: &Path,
    target: LirTargetProfile,
    unit: &GeneratedBridgeUnitPlanV1,
    symbols: &StrongObjectSymbolSurfaceV1,
) -> Result<(), CodegenError> {
    let expected = std::iter::once(unit.primary_atom_authority())
        .chain(unit.materialized_associated_atom_authorities())
        .map(|atom| (atom.id(), atom))
        .collect::<BTreeMap<_, _>>();
    let mut definitions = BTreeMap::<GeneratedBridgeAtomId, &StrongDefinitionSymbolPlanV1>::new();
    for definition in symbols.plans() {
        let StrongDefinitionEntityKind::GeneratedBridgeAtom(atom) = definition.owner().kind()
        else {
            continue;
        };
        if expected.contains_key(&atom) && definitions.insert(atom, definition).is_some() {
            return Err(CodegenError(format!(
                "generated-C bridge atom {atom} has multiple strong symbol plans"
            )));
        }
    }
    if definitions.len() != expected.len() {
        let missing = expected
            .keys()
            .find(|atom| !definitions.contains_key(atom))
            .copied();
        return Err(CodegenError(format!(
            "generated-C unit {} has no strong symbol plan for atom {missing:?}",
            unit.unit()
        )));
    }

    let mut bytes = std::fs::read(path).map_err(|error| {
        CodegenError(format!(
            "cannot read generated-C object {} for atom boundary materialization: {error}",
            path.display()
        ))
    })?;
    let layout = MachOLayout::parse(&bytes)?;
    let normalization = target.contract().native_symbol_normalization();
    let mut additions = Vec::with_capacity(expected.len() * 2);
    for (atom, authority) in expected {
        let definition = definitions[&atom];
        let [boundary] = definition.atom_boundaries() else {
            return Err(invalid_definition(unit, atom));
        };
        if definition.definition_role() != StrongDefinitionRole::GeneratedBridge
            || definition.primary_atom() != boundary.atom()
            || boundary.atom_role() != DefinitionAtomRole::Primary
        {
            return Err(invalid_definition(unit, atom));
        }

        let primary_name = normalization
            .compiler_generated_object_symbol(definition.primary_symbol().symbol().as_str())
            .into_bytes();
        let primary = layout.require_external_definition(&primary_name)?;
        let section = layout.require_section_ordinal(primary.section_ordinal())?;
        let expected_section = match authority.key().atom() {
            GeneratedBridgeAtomRoleKey::PrimaryEntry { unit: owner } if owner == unit.unit() => {
                (b"__TEXT".as_slice(), b"__text".as_slice())
            }
            GeneratedBridgeAtomRoleKey::SignatureDescriptor { unit: owner, .. }
                if owner == unit.unit() =>
            {
                GENERATED_BRIDGE_SIGNATURE_SECTION
            }
            GeneratedBridgeAtomRoleKey::ContextDescriptor { unit: owner, .. }
                if owner == unit.unit() =>
            {
                GENERATED_BRIDGE_CONTEXT_SECTION
            }
            GeneratedBridgeAtomRoleKey::PrimaryEntry { .. }
            | GeneratedBridgeAtomRoleKey::SignatureDescriptor { .. }
            | GeneratedBridgeAtomRoleKey::ContextDescriptor { .. }
            | GeneratedBridgeAtomRoleKey::StaticAssertSupport { .. } => {
                return Err(CodegenError(format!(
                    "generated-C unit {} contains non-materializable or foreign atom {atom}",
                    unit.unit()
                )));
            }
        };
        let canonical_section = layout.require_section(expected_section.0, expected_section.1)?;
        if section.ordinal() != canonical_section.ordinal()
            || primary.value() != canonical_section.address()
        {
            return Err(CodegenError(format!(
                "generated-C bridge atom {atom} does not start its canonical section {},{}",
                String::from_utf8_lossy(expected_section.0),
                String::from_utf8_lossy(expected_section.1)
            )));
        }
        let end = canonical_section.checked_end()?;
        if !matches!(
            authority.key().atom(),
            GeneratedBridgeAtomRoleKey::PrimaryEntry { .. }
        ) && end != canonical_section.address() + 1
        {
            return Err(CodegenError(format!(
                "generated-C descriptor atom {atom} is not an isolated one-byte section"
            )));
        }

        let start_name = normalization
            .compiler_generated_object_symbol(boundary.start().symbol().as_str())
            .into_bytes();
        let end_name = normalization
            .compiler_generated_object_symbol(boundary.end().symbol().as_str())
            .into_bytes();
        additions.push(BoundaryDefinitionV1::new(
            start_name,
            canonical_section.ordinal(),
            canonical_section.address(),
        ));
        additions.push(BoundaryDefinitionV1::new(
            end_name,
            canonical_section.ordinal(),
            end,
        ));
    }
    layout.add_external_definitions(&mut bytes, additions)?;
    std::fs::write(path, bytes).map_err(|error| {
        CodegenError(format!(
            "cannot write generated-C atom boundaries to {}: {error}",
            path.display()
        ))
    })
}

fn invalid_definition(
    unit: &GeneratedBridgeUnitPlanV1,
    atom: GeneratedBridgeAtomId,
) -> CodegenError {
    CodegenError(format!(
        "generated-C unit {} has an invalid strong definition plan for atom {atom}",
        unit.unit()
    ))
}
