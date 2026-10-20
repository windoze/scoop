use inkwell::context::Context;
use inkwell::module::Module as LlvmModule;
use inkwell::values::GlobalValue;

use super::{
    EmittedEntryProductionV1, EmittedStaticStorageInitialStateV1,
    EmittedStaticStorageRelocationTableV1, EmittedStrongInitializationUnitRegistrationSetV1,
    emit_cone_image_v1, emit_entry_production_v1, emit_strong_callable_registrations_v1,
    emit_strong_immortal_object_registrations_v1, emit_strong_initialization_unit_registrations_v1,
    emit_strong_safepoint_registrations_v1, emit_strong_static_storage_registrations_v1,
    emit_strong_type_registrations_v1,
};
use crate::CodegenError;
use crate::atom_boundaries::{GlobalAtomMaterializationV1, emit_global_atom_boundaries_v1};
use crate::target::ValidatedBackendProfile;

mod callable;
pub(crate) use callable::{emit_callable_metadata_v1, emit_callable_safepoints};
/// Emit complete runtime records and return initialization globals needed by codegen.
pub(crate) fn emit_strong_runtime_metadata_v1<
    'ctx,
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
    I: Clone,
>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    profile: ValidatedBackendProfile,
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    array_bounds_message: GlobalValue<'ctx>,
    array_size_overflow_message: GlobalValue<'ctx>,
) -> Result<EmittedStrongInitializationUnitRegistrationSetV1<'ctx>, CodegenError> {
    let types = emit_strong_type_registrations_v1(
        context,
        llvm,
        production.registration_production().types(),
    )?;
    let immortal_objects = emit_strong_immortal_object_registrations_v1(
        context,
        llvm,
        production.registration_production().immortal_objects(),
    )?;
    let static_storages = emit_strong_static_storage_registrations_v1(
        context,
        llvm,
        production.registration_production().static_storages(),
    )?;
    let initialization_units = emit_strong_initialization_unit_registrations_v1(
        context,
        llvm,
        profile,
        production.registration_production().initialization_units(),
    )?;
    let entry = emit_entry_production_v1(context, llvm, production.entry_plan())?;
    let image = emit_cone_image_v1(
        context,
        llvm,
        production.image_plan(),
        production.registration_production().identities(),
        array_bounds_message,
        array_size_overflow_message,
    );

    emit_global_atom_boundaries_v1(
        llvm,
        production.canonical_definitions(),
        runtime_global_atoms(
            production,
            &types,
            &immortal_objects,
            &static_storages,
            &initialization_units,
            entry,
            image,
        )?,
    )?;
    Ok(initialization_units)
}

#[allow(clippy::too_many_arguments)]
fn runtime_global_atoms<'ctx, D, C, I>(
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    types: &super::EmittedStrongTypeRegistrationSetV1<'ctx>,
    immortal_objects: &super::EmittedStrongImmortalObjectRegistrationSetV1<'ctx>,
    static_storages: &super::EmittedStrongStaticStorageRegistrationSetV1<'ctx>,
    initialization_units: &super::EmittedStrongInitializationUnitRegistrationSetV1<'ctx>,
    entry: EmittedEntryProductionV1<'ctx>,
    image: super::EmittedConeImageV1<'ctx>,
) -> Result<Vec<GlobalAtomMaterializationV1<'ctx>>, CodegenError> {
    let mut atoms = Vec::new();
    atoms.extend(types.registrations().iter().map(|registration| {
        GlobalAtomMaterializationV1::new(registration.primary_atom(), registration.descriptor())
    }));

    for (emitted, plan) in immortal_objects.registrations().iter().zip(
        production
            .registration_production()
            .immortal_objects()
            .registrations(),
    ) {
        atoms.push(GlobalAtomMaterializationV1::new(
            plan.registration_primary_atom(),
            emitted.descriptor(),
        ));
        atoms.push(GlobalAtomMaterializationV1::new(
            plan.object_primary_atom(),
            emitted.object_value(),
        ));
    }

    for (emitted, plan) in static_storages.registrations().iter().zip(
        production
            .registration_production()
            .static_storages()
            .registrations(),
    ) {
        atoms.extend([
            GlobalAtomMaterializationV1::new(
                plan.registration_primary_atom(),
                emitted.descriptor(),
            ),
            GlobalAtomMaterializationV1::new(plan.storage_primary_atom(), emitted.storage_value()),
        ]);
        if plan.semantic().value_layout().local().is_some() {
            atoms.push(GlobalAtomMaterializationV1::new(
                plan.scan_primary_atom(),
                emitted.scan_program(),
            ));
        }
        if let EmittedStaticStorageInitialStateV1::EncodedStaticValue {
            template_atom,
            template,
            relocations,
        } = emitted.initial_state()
        {
            atoms.push(GlobalAtomMaterializationV1::new(template_atom, template));
            if let EmittedStaticStorageRelocationTableV1::Defined { atom, global } = relocations {
                atoms.push(GlobalAtomMaterializationV1::new(atom, global));
            }
        }
    }

    for (emitted, plan) in initialization_units.registrations().iter().zip(
        production
            .registration_production()
            .initialization_units()
            .registrations(),
    ) {
        atoms.extend([
            GlobalAtomMaterializationV1::new(
                plan.registration_primary_atom(),
                emitted.registration_descriptor(),
            ),
            GlobalAtomMaterializationV1::new(plan.cell_primary_atom(), emitted.cell()),
            GlobalAtomMaterializationV1::new(emitted.diagnostic_atom(), emitted.diagnostic()),
        ]);
    }

    if let EmittedEntryProductionV1::Executable(emitted) = entry {
        let definition = production
            .canonical_definitions()
            .plan(emitted.definition())
            .ok_or_else(|| {
                CodegenError(
                    "root entry definition is absent from canonical symbol plan".to_string(),
                )
            })?;
        atoms.push(GlobalAtomMaterializationV1::new(
            definition.primary_atom(),
            emitted.descriptor(),
        ));
    }

    atoms.push(GlobalAtomMaterializationV1::new(
        image.primary_atom(),
        image.image(),
    ));
    let support = image.support_atoms();
    for support in [
        support.coordinate_group(),
        support.coordinate_name(),
        support.coordinate_version(),
        support.dependencies(),
        support.static_storages(),
        support.immortal_objects(),
        support.initialization_units(),
        support.type_registrations(),
        support.safepoints(),
        support.callables(),
    ] {
        atoms.push(GlobalAtomMaterializationV1::new(
            support.atom(),
            support.global(),
        ));
    }
    Ok(atoms)
}

#[cfg(test)]
mod tests;
