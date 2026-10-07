//! Project final callable GC/EH plans before emitting image metadata.

use super::*;
use scoop_lir::{
    DefinitionAtomRole, DefinitionAtomSubkey, ObjectDefinitionAtomId, ObjectDefinitionAtomKey,
    StrongDefinitionEntityKind, StrongDefinitionRole,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn finalize_production<'a, D, C, I>(
    input: &scoop_lir::ConeLirOutput,
    production: &mut scoop_lir::ConeProductionSection<D, C, I>,
    bodies: impl IntoIterator<Item = (scoop_lir::PersistentCallableBodyId, &'a CallableCodePlan)>,
) -> Result<scoop_lir::ConeLirFoundation, CodegenError> {
    let bodies = bodies.into_iter().collect::<BTreeMap<_, _>>();
    let mut counts = BTreeMap::new();
    for registration in production
        .registration_production()
        .safepoints()
        .registrations()
    {
        let plan = bodies
            .get(&registration.owner())
            .expect("every callable was prepared before finalizing production");
        if let Some(count) = plan.safepoints.root_count(registration.safepoint().get()) {
            let count = u32::try_from(count)
                .map_err(|_| CodegenError("physical root count exceeds u32::MAX".into()))?;
            counts.insert(registration.safepoint(), count);
        }
    }
    let format = input.module().meta.target_profile.native_object_format();
    let mut removed_atoms = BTreeSet::new();
    for definition in production
        .canonical_definitions()
        .plans()
        .iter()
        .filter(|plan| plan.definition_role() == StrongDefinitionRole::CallableBody)
    {
        let StrongDefinitionEntityKind::CallableBody(body) = definition.owner().kind() else {
            unreachable!("callable definitions have typed callable owners");
        };
        let plan = bodies
            .get(&body)
            .expect("every callable was prepared before finalizing production");
        let mut roles = Vec::new();
        if plan.safepoints.site_count() == 0 {
            roles.push(DefinitionAtomRole::Stackmap);
        }
        if plan.eh.function_count() == 0 {
            roles.push(DefinitionAtomRole::Lsda);
            roles.push(match format {
                scoop_lir::NativeObjectFormat::MachO64 => DefinitionAtomRole::EhFrame,
                scoop_lir::NativeObjectFormat::Elf64 => DefinitionAtomRole::AddressTakenConstant,
            });
        }
        for role in roles {
            // Only the body-scoped backend atom is removed, never a CString or scan.
            let atom = ObjectDefinitionAtomId::from_key(&ObjectDefinitionAtomKey::new(
                definition.definition_plan(),
                role,
                DefinitionAtomSubkey::CallableBody(body),
            ))
            .map_err(|error| CodegenError(format!("cannot derive backend atom: {error}")))?;
            if definition
                .atom_boundaries()
                .iter()
                .any(|boundary| boundary.atom() == atom)
            {
                removed_atoms.insert(atom);
            }
        }
    }
    production
        .finalize_codegen(input.foundation(), &counts, &removed_atoms)
        .map_err(|error| CodegenError(format!("cannot finalize GC/EH production: {error}")))
}
