//! Registrations and Context cells selected together with one callable body.

use super::*;

pub(crate) fn emit_callable_metadata_v1<'ctx, D, C, I>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    profile: ValidatedBackendProfile,
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    body: scoop_lir::PersistentCallableBodyId,
) -> Result<(), CodegenError> {
    let callables = emit_strong_callable_registrations_v1(
        context,
        llvm,
        production.registration_production().callables(),
        production.canonical_definitions(),
        profile,
        body,
    )?;
    let mut atoms = callables.context_atoms.clone();
    for registration in callables.registrations() {
        atoms.push(GlobalAtomMaterializationV1::new(
            registration.primary_atom(),
            registration.descriptor(),
        ));
    }
    emit_global_atom_boundaries_v1(llvm, production.canonical_definitions(), atoms)?;
    Ok(())
}

pub(crate) fn emit_callable_safepoints<'ctx, D, C, I>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    body: scoop_lir::PersistentCallableBodyId,
    expected: &crate::statepoint::LoweredSafepoints,
) -> Result<(), CodegenError> {
    let safepoints = emit_strong_safepoint_registrations_v1(
        context,
        llvm,
        production.registration_production().safepoints(),
        body,
        |id| expected.root_count(id.get()),
    )?;
    let mut atoms = Vec::new();
    for registration in safepoints.registrations() {
        atoms.push(GlobalAtomMaterializationV1::new(
            registration.primary_atom(),
            registration.descriptor(),
        ));
    }
    emit_global_atom_boundaries_v1(llvm, production.canonical_definitions(), atoms)?;
    Ok(())
}
