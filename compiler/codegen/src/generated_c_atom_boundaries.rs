//! Materialize exact generated-C atom boundaries in native objects.

use std::collections::BTreeMap;
use std::path::Path;

use scoop_lir::{
    DefinitionAtomRole, DefinitionSymbolPlanV1, GeneratedBridgeAtomId, GeneratedBridgeAtomRoleKey,
    GeneratedBridgeUnitPlanV1, LirTargetProfile, ObjectSymbolSurfaceV1, StrongDefinitionEntityKind,
    StrongDefinitionRole,
};

use crate::CodegenError;
mod elf;
mod macho;

pub(crate) fn materialize_v1(
    path: &Path,
    target: LirTargetProfile,
    unit: &GeneratedBridgeUnitPlanV1,
    symbols: &ObjectSymbolSurfaceV1,
) -> Result<(), CodegenError> {
    let expected = std::iter::once(unit.primary_atom_authority())
        .chain(unit.materialized_associated_atom_authorities())
        .map(|atom| (atom.id(), atom))
        .collect::<BTreeMap<_, _>>();
    let mut definitions = BTreeMap::<GeneratedBridgeAtomId, &DefinitionSymbolPlanV1>::new();
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

    let mut atoms = Vec::with_capacity(expected.len());
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
        let kind = match authority.key().atom() {
            GeneratedBridgeAtomRoleKey::PrimaryEntry { unit: owner } if owner == unit.unit() => {
                BridgeAtomKind::Entry
            }
            GeneratedBridgeAtomRoleKey::SignatureDescriptor { unit: owner, .. }
                if owner == unit.unit() =>
            {
                BridgeAtomKind::Signature
            }
            GeneratedBridgeAtomRoleKey::ContextDescriptor { unit: owner, .. }
                if owner == unit.unit() =>
            {
                BridgeAtomKind::Context
            }
            _ => return Err(invalid_definition(unit, atom)),
        };
        atoms.push((kind, definition));
    }
    let bytes = std::fs::read(path).map_err(|error| {
        CodegenError(format!(
            "cannot read generated-C object {}: {error}",
            path.display()
        ))
    })?;
    let output = match target.native_object_format() {
        scoop_lir::NativeObjectFormat::MachO64 => macho::materialize(bytes, target, &atoms)?,
        scoop_lir::NativeObjectFormat::Elf64 => elf::materialize(&bytes, &atoms)?,
    };
    std::fs::write(path, output).map_err(|error| {
        CodegenError(format!(
            "cannot write generated-C boundaries to {}: {error}",
            path.display()
        ))
    })
}

#[derive(Clone, Copy)]
enum BridgeAtomKind {
    Entry,
    Signature,
    Context,
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
