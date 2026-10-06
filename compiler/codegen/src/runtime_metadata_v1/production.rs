use inkwell::context::Context;
use inkwell::module::Module as LlvmModule;
use inkwell::targets::TargetData;
use inkwell::values::GlobalValue;
use scoop_lir::{
    ConeIdentity, DigestPatchIntentId, ObjectDefinitionAtomId, ObjectDefinitionPlanId,
    PersistentSymbolRequest,
};

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
pub(crate) use callable::emit_callable_metadata_v1;
mod digest;
pub(crate) use digest::validate_patch_coverage;
use digest::{PatchParts, PatchSiteParts, record_patch};

/// Object-independent location of one provisional digest slot emitted by
/// codegen. The packager resolves the typed owner symbol and offset against
/// the verified object; it never rediscovers a slot from descriptor bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProvisionalStrongDigestPatchLocationV1 {
    intent: DigestPatchIntentId,
    definition: ObjectDefinitionPlanId,
    atom: ObjectDefinitionAtomId,
    owner: PersistentSymbolRequest,
    offset_within_owner: u64,
    width_bytes: u8,
}

impl ProvisionalStrongDigestPatchLocationV1 {
    pub const fn intent(self) -> DigestPatchIntentId {
        self.intent
    }

    pub const fn definition(self) -> ObjectDefinitionPlanId {
        self.definition
    }

    pub const fn atom(self) -> ObjectDefinitionAtomId {
        self.atom
    }

    pub const fn owner(self) -> PersistentSymbolRequest {
        self.owner
    }

    pub const fn offset_within_owner(self) -> u64 {
        self.offset_within_owner
    }

    pub const fn width_bytes(self) -> u8 {
        self.width_bytes
    }
}

/// Complete provisional runtime-metadata result for one strong Cone.
///
/// LLVM values deliberately do not escape this product. The only data needed
/// by object packaging is the canonical, typed patch-location set below.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmittedStrongRuntimeMetadataV1 {
    producer: ConeIdentity,
    patch_locations: Vec<ProvisionalStrongDigestPatchLocationV1>,
}

/// LLVM-local emission proof used to connect generated function bodies to
/// their canonical initialization registrations. Only the
/// packaging projection escapes codegen.
pub(crate) struct EmittedStrongRuntimeMetadataModuleV1<'ctx> {
    packaging: EmittedStrongRuntimeMetadataV1,
    initialization_units: EmittedStrongInitializationUnitRegistrationSetV1<'ctx>,
}

impl<'ctx> EmittedStrongRuntimeMetadataModuleV1<'ctx> {
    #[cfg(test)]
    pub(crate) const fn producer(&self) -> ConeIdentity {
        self.packaging.producer()
    }

    #[cfg(test)]
    pub(crate) fn patch_locations(&self) -> &[ProvisionalStrongDigestPatchLocationV1] {
        self.packaging.patch_locations()
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        EmittedStrongRuntimeMetadataV1,
        EmittedStrongInitializationUnitRegistrationSetV1<'ctx>,
    ) {
        (self.packaging, self.initialization_units)
    }
}

impl EmittedStrongRuntimeMetadataV1 {
    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn patch_locations(&self) -> &[ProvisionalStrongDigestPatchLocationV1] {
        &self.patch_locations
    }
}

/// Emit the complete runtime-metadata surface described by one closed strong
/// production section and return its exact provisional digest locations.
///
/// The caller owns the LLVM module under construction. On failure that module
/// must be discarded; no partially emitted module is a successful product.
pub(crate) fn emit_strong_runtime_metadata_v1<
    'ctx,
    D: scoop_lir::StrongDescriptorReference,
    C: Clone,
    I: Clone,
>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    target_data: &TargetData,
    profile: ValidatedBackendProfile,
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    array_bounds_message: GlobalValue<'ctx>,
    array_size_overflow_message: GlobalValue<'ctx>,
) -> Result<EmittedStrongRuntimeMetadataModuleV1<'ctx>, CodegenError> {
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
        target_data,
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
    )?;

    let producer = production.image_plan().cone().identity();
    for actual in [
        types.producer(),
        immortal_objects.producer(),
        static_storages.producer(),
        initialization_units.producer(),
        production.image_plan().cone().identity(),
    ] {
        if actual != producer {
            return Err(CodegenError(format!(
                "strong runtime metadata producer mismatch: expected {producer}, found {actual}"
            )));
        }
    }

    let mut patches = Vec::new();
    for registration in types.registrations() {
        let registration = *registration;
        for patch in [
            registration.registration_definition_patch(),
            registration.descriptor_definition_patch(),
            registration.layout_fingerprint_patch(),
        ] {
            record_patch(production, &mut patches, patch.into_parts())?;
        }
    }
    for registration in immortal_objects.registrations() {
        record_patch(
            production,
            &mut patches,
            registration.registration_definition_patch().into_parts(),
        )?;
    }
    for registration in static_storages.registrations() {
        let registration = *registration;
        for patch in [
            registration.registration_definition_patch(),
            registration.scan_fingerprint_patch(),
            registration.layout_fingerprint_patch(),
        ] {
            record_patch(production, &mut patches, patch.into_parts())?;
        }
    }
    for registration in initialization_units.registrations() {
        let registration = *registration;
        record_patch(
            production,
            &mut patches,
            registration.registration_definition_patch().into_parts(),
        )?;
        if let Some(patch) = registration.gateway_definition_patch() {
            record_patch(production, &mut patches, patch.into_parts())?;
        }
    }
    if let EmittedEntryProductionV1::Executable(entry) = entry {
        record_patch(
            production,
            &mut patches,
            PatchParts::without_atom(entry.source_signature_patch()),
        )?;
        record_patch(
            production,
            &mut patches,
            PatchParts::without_atom(entry.gateway_definition_patch()),
        )?;
    }
    record_patch(production, &mut patches, image.patch().into_parts())?;

    patches.sort_unstable_by_key(|patch| patch.intent);
    emit_global_atom_boundaries_v1(
        llvm,
        target_data,
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
    Ok(EmittedStrongRuntimeMetadataModuleV1 {
        packaging: EmittedStrongRuntimeMetadataV1 {
            producer,
            patch_locations: patches,
        },
        initialization_units,
    })
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
        GlobalAtomMaterializationV1::new(
            registration.registration_definition_patch().atom(),
            registration.descriptor(),
        )
    }));

    require_parallel_coverage(
        "immortal-object registration",
        immortal_objects.registrations().len(),
        production
            .registration_production()
            .immortal_objects()
            .registrations()
            .len(),
    )?;
    for (emitted, plan) in immortal_objects.registrations().iter().zip(
        production
            .registration_production()
            .immortal_objects()
            .registrations(),
    ) {
        if emitted.object() != plan.object() {
            return Err(CodegenError(
                "immortal-object emission order diverges from its closed plan".to_string(),
            ));
        }
        atoms.push(GlobalAtomMaterializationV1::new(
            plan.registration_primary_atom(),
            emitted.descriptor(),
        ));
        atoms.push(GlobalAtomMaterializationV1::new(
            plan.object_primary_atom(),
            emitted.object_value(),
        ));
    }

    require_parallel_coverage(
        "static-storage registration",
        static_storages.registrations().len(),
        production
            .registration_production()
            .static_storages()
            .registrations()
            .len(),
    )?;
    for (emitted, plan) in static_storages.registrations().iter().zip(
        production
            .registration_production()
            .static_storages()
            .registrations(),
    ) {
        if emitted.storage() != plan.semantic().storage() {
            return Err(CodegenError(
                "static-storage emission order diverges from its closed plan".to_string(),
            ));
        }
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

    require_parallel_coverage(
        "initialization registration",
        initialization_units.registrations().len(),
        production
            .registration_production()
            .initialization_units()
            .registrations()
            .len(),
    )?;
    for (emitted, plan) in initialization_units.registrations().iter().zip(
        production
            .registration_production()
            .initialization_units()
            .registrations(),
    ) {
        if emitted.unit() != plan.semantic().unit() {
            return Err(CodegenError(
                "initialization emission order diverges from its closed plan".to_string(),
            ));
        }
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
        let plan = match production.entry_plan() {
            scoop_lir::EntryProductionPlanV1::Executable(plan) => plan,
            scoop_lir::EntryProductionPlanV1::Library => {
                return Err(CodegenError(
                    "executable entry emission has a library production plan".to_string(),
                ));
            }
        };
        let definition = production
            .canonical_definitions()
            .plan(plan.root_descriptor_definition())
            .ok_or_else(|| {
                CodegenError(
                    "root entry definition is absent from canonical symbol authority".to_string(),
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

fn require_parallel_coverage(
    kind: &str,
    emitted: usize,
    planned: usize,
) -> Result<(), CodegenError> {
    if emitted == planned {
        Ok(())
    } else {
        Err(CodegenError(format!(
            "{kind} emission coverage mismatch: planned {planned}, emitted {emitted}"
        )))
    }
}

#[cfg(test)]
mod tests;
