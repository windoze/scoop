//! Cross-Cone link-closure and terminal object fingerprint pipeline.

use scoop_lir::CrossConeLirBridgeSectionV1;
use scoop_slib::{
    ArtifactCapabilityProfile, CanonicalDefinedLinkSymbolOwnerSetV1, ConeRecord, DependencyRecord,
    FinalizedUndefinedSymbolRequirementPartitionsV1, SlibMember,
    VerifiedCrossConeCodeFingerprintV1, VerifiedEntryPatchSetV1,
    VerifiedStrongCallableFingerprintSetV1, VerifiedStrongImmortalObjectFingerprintSetV1,
    VerifiedStrongInitializationFingerprintSetV1, VerifiedStrongSafepointFingerprintSetV1,
    VerifiedStrongStaticStorageFingerprintSetV1, VerifiedStrongTypeFingerprintSetV1,
    compute_cross_cone_code_fingerprint_v1,
    compute_cross_cone_strong_callable_body_object_fingerprints_v1,
    compute_cross_cone_strong_immortal_object_definition_fingerprints_v1,
    compute_runtime_image_fingerprint_v1, compute_strong_callable_fingerprints_v1,
    compute_strong_immortal_object_fingerprints_v1,
    compute_strong_initialization_definition_fingerprints_v1,
    compute_strong_initialization_fingerprints_v1,
    compute_strong_static_storage_definition_fingerprints_v1,
    compute_strong_static_storage_fingerprints_v1,
    compute_strong_static_storage_shape_fingerprints_v1,
    compute_strong_type_dependency_fingerprints_v1, compute_strong_type_fingerprints_v1,
    finalize_partitioned_undefined_symbol_requirements_v1, patch_entry_production_v1,
    patch_runtime_image_fingerprint_v1, patch_strong_registration_fingerprints_v1,
    seal_builtin_object_external_requirements_v1, verify_c_bridge_target_support_requirements_v1,
    verify_code_link_object_members_v1, verify_cone_image_v1, verify_core_strong_requirements_v1,
    verify_cross_cone_strong_requirements_v1, verify_current_cone_undefined_requirements_v1,
    verify_entry_production_v1, verify_generated_c_bridge_semantics_v1,
    verify_runtime_and_eh_requirements_v1, verify_single_cone_production_code_projection_v1,
    verify_source_external_requirements_after_cross_cone_v1,
};

use super::{fingerprint_pipeline::final_link_object_members, *};

/// Complete legacy/core/native and ordinary-dependency undefined-use closure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeLinkSymbolVerifiedObjectProductionV1 {
    production: PlannedBuiltinObjectProductionV1,
    symbol_plan: PlannedStrongObjectSymbolSetV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_requirements: FinalizedUndefinedSymbolRequirementPartitionsV1,
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callable_registration_objects: VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    type_registration_objects: VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
    immortal_object_registration_objects:
        VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    static_storage_registration_objects:
        VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    initialization_registration_objects:
        VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
}

impl RegistrationObjectLeafFingerprintedProductionV1 {
    pub fn verify_cross_cone_link_symbol_requirements(
        self,
        core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
        bridge: &CrossConeLirBridgeSectionV1,
    ) -> Result<CrossConeLinkSymbolVerifiedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
        } = self;
        let patch_sites = callable_registration_objects
            .registrations()
            .patch_sites()
            .clone();
        let strong_closure = patch_sites.builtins().strong_relocations().clone();
        let defined_symbols =
            CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&strong_closure)
                .map_err(BuiltinObjectProductionError::DefinedSymbols)?;
        let bridge_plan = production.production.generated_bridge_plan().clone();
        let current_cone = verify_current_cone_undefined_requirements_v1(
            strong_closure.clone(),
            bridge_plan.clone(),
        )
        .map_err(BuiltinObjectProductionError::CurrentConeRequirements)?;
        let native_requirements =
            scoop_lir::CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
                production.target(),
                &production.foundation,
            )
            .map_err(BuiltinObjectProductionError::NativeRequirementSurface)?;
        let core = verify_core_strong_requirements_v1(
            production.target(),
            strong_closure,
            production.production.external_bridges().clone(),
            core_owners.clone(),
        )
        .map_err(BuiltinObjectProductionError::CoreRequirements)?;
        let cross_cone = verify_cross_cone_strong_requirements_v1(core, bridge)
            .map_err(BuiltinObjectProductionError::CrossConeRequirements)?;
        let source = verify_source_external_requirements_after_cross_cone_v1(
            cross_cone,
            native_requirements.clone(),
        )
        .map_err(BuiltinObjectProductionError::SourceExternalRequirements)?;
        let runtime_and_eh =
            verify_runtime_and_eh_requirements_v1(source, production.target_selection)
                .map_err(BuiltinObjectProductionError::RuntimeAndEhRequirements)?;
        let bridge_semantics = verify_generated_c_bridge_semantics_v1(
            patch_sites,
            bridge_plan,
            native_requirements,
            &production.c_bridge_profile,
        )
        .map_err(BuiltinObjectProductionError::GeneratedBridgeSemantics)?;
        let external =
            verify_c_bridge_target_support_requirements_v1(runtime_and_eh, bridge_semantics)
                .map_err(BuiltinObjectProductionError::CBridgeTargetSupportRequirements)?;
        let external = seal_builtin_object_external_requirements_v1(external)
            .map_err(BuiltinObjectProductionError::UnclassifiedExternalRequirement)?;
        let undefined_requirements =
            finalize_partitioned_undefined_symbol_requirements_v1(current_cone, external)
                .map_err(BuiltinObjectProductionError::UndefinedSymbols)?;

        Ok(CrossConeLinkSymbolVerifiedObjectProductionV1 {
            production,
            symbol_plan,
            defined_symbols,
            undefined_requirements,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
        })
    }
}

/// Registration fingerprints computed with the partitioned relocation proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeRegistrationDependencyFingerprintedProductionV1 {
    production: PlannedBuiltinObjectProductionV1,
    symbol_plan: PlannedStrongObjectSymbolSetV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_requirements: FinalizedUndefinedSymbolRequirementPartitionsV1,
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: VerifiedStrongTypeFingerprintSetV1,
    immortal_objects: VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: VerifiedStrongInitializationFingerprintSetV1,
}

impl CrossConeLinkSymbolVerifiedObjectProductionV1 {
    pub fn fingerprint_registration_dependencies(
        self,
    ) -> Result<
        CrossConeRegistrationDependencyFingerprintedProductionV1,
        BuiltinObjectProductionError,
    > {
        let Self {
            production,
            symbol_plan,
            defined_symbols,
            undefined_requirements,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
        } = self;
        let (callables, types, immortal_objects, static_storages, initializations) = {
            let candidates = production.scoop_lir_candidates();
            let stackmaps = safepoints.registrations().stackmaps().clone();
            let callable_bodies = compute_cross_cone_strong_callable_body_object_fingerprints_v1(
                callable_registration_objects,
                stackmaps,
                undefined_requirements.clone(),
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::CallableBodyFingerprints)?;
            let callables = compute_strong_callable_fingerprints_v1(callable_bodies)
                .map_err(BuiltinObjectProductionError::CallableFingerprints)?;
            let type_dependencies = compute_strong_type_dependency_fingerprints_v1(
                type_registration_objects,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::TypeDependencyFingerprints)?;
            let types = compute_strong_type_fingerprints_v1(type_dependencies)
                .map_err(BuiltinObjectProductionError::TypeFingerprints)?;
            let immortal_definitions =
                compute_cross_cone_strong_immortal_object_definition_fingerprints_v1(
                    immortal_object_registration_objects,
                    undefined_requirements.clone(),
                    &candidates,
                )
                .map_err(BuiltinObjectProductionError::ImmortalObjectDefinitionFingerprints)?;
            let immortal_objects =
                compute_strong_immortal_object_fingerprints_v1(immortal_definitions)
                    .map_err(BuiltinObjectProductionError::ImmortalObjectFingerprints)?;
            let static_definitions = compute_strong_static_storage_definition_fingerprints_v1(
                static_storage_registration_objects,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::StaticStorageDefinitionFingerprints)?;
            let static_shapes =
                compute_strong_static_storage_shape_fingerprints_v1(static_definitions)
                    .map_err(BuiltinObjectProductionError::StaticStorageShapeFingerprints)?;
            let static_storages = compute_strong_static_storage_fingerprints_v1(static_shapes)
                .map_err(BuiltinObjectProductionError::StaticStorageFingerprints)?;
            let initialization_definitions =
                compute_strong_initialization_definition_fingerprints_v1(
                    initialization_registration_objects,
                    &candidates,
                )
                .map_err(BuiltinObjectProductionError::InitializationDefinitionFingerprints)?;
            let initializations = compute_strong_initialization_fingerprints_v1(
                initialization_definitions,
                callables.body_objects(),
            )
            .map_err(BuiltinObjectProductionError::InitializationFingerprints)?;
            (
                callables,
                types,
                immortal_objects,
                static_storages,
                initializations,
            )
        };

        Ok(CrossConeRegistrationDependencyFingerprintedProductionV1 {
            production,
            symbol_plan,
            defined_symbols,
            undefined_requirements,
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initializations,
        })
    }
}

/// Final object bytes whose runtime image is bound to the cross-Cone profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeFinalizedStrongObjectProductionV1 {
    production: PlannedBuiltinObjectProductionV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_requirements: FinalizedUndefinedSymbolRequirementPartitionsV1,
    final_objects: VerifiedEntryPatchSetV1,
}

impl CrossConeRegistrationDependencyFingerprintedProductionV1 {
    pub fn finalize_strong_objects(
        self,
    ) -> Result<CrossConeFinalizedStrongObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan: _,
            defined_symbols,
            undefined_requirements,
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initializations,
        } = self;
        let final_objects = {
            let candidates = production.scoop_lir_candidates();
            let patch_sites = safepoints.registrations().patch_sites().clone();
            let image = verify_cone_image_v1(
                patch_sites.clone(),
                production.production.image_plan().clone(),
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::ConeImage)?;
            let entry = verify_entry_production_v1(
                patch_sites,
                production.production.entry_plan().clone(),
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::EntryProduction)?;
            let registrations = patch_strong_registration_fingerprints_v1(
                safepoints,
                callables,
                types,
                immortal_objects,
                static_storages,
                initializations,
                &candidates,
            )
            .map_err(BuiltinObjectProductionError::RegistrationPatch)?;
            let compatibility = scoop_slib::CompatibilityRecord::new(
                production.target_selection,
                ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
            )
            .map_err(BuiltinObjectProductionError::Compatibility)?;
            let image_fingerprint =
                compute_runtime_image_fingerprint_v1(image, registrations, compatibility)
                    .map_err(BuiltinObjectProductionError::RuntimeImageFingerprint)?;
            let runtime_images = patch_runtime_image_fingerprint_v1(image_fingerprint)
                .map_err(BuiltinObjectProductionError::RuntimeImagePatch)?;
            patch_entry_production_v1(runtime_images, entry)
                .map_err(BuiltinObjectProductionError::EntryPatch)?
        };

        Ok(CrossConeFinalizedStrongObjectProductionV1 {
            production,
            defined_symbols,
            undefined_requirements,
            final_objects,
        })
    }
}

/// Final members and cross-Cone Code proof ready for archive assembly.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CrossConeCodeFingerprintedObjectProductionV1 {
    target_selection: ValidatedLirTargetSelection,
    lir_foundation: OdrFreeLirFoundation,
    c_bridge_profile: CBridgeToolchainProfileV1,
    members: Vec<SlibMember>,
    code: VerifiedCrossConeCodeFingerprintV1,
}

impl CrossConeFinalizedStrongObjectProductionV1 {
    pub fn fingerprint_code(
        self,
        cone: &ConeRecord,
        direct_dependencies: &[DependencyRecord],
        source_count: usize,
    ) -> Result<CrossConeCodeFingerprintedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            defined_symbols,
            undefined_requirements,
            final_objects,
        } = self;
        let members = final_link_object_members(&production, &final_objects)?;
        let directory = members
            .iter()
            .map(|member| member.record().clone())
            .collect::<Vec<_>>();
        let link_objects = verify_code_link_object_members_v1(final_objects, &directory)
            .map_err(BuiltinObjectProductionError::CodeLinkObjects)?;
        let code_projection = verify_single_cone_production_code_projection_v1(
            cone,
            direct_dependencies,
            source_count,
            production.production.clone(),
            link_objects,
        )
        .map_err(BuiltinObjectProductionError::ProductionCodeProjection)?;
        let native_requirements =
            scoop_lir::CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
                production.target(),
                &production.foundation,
            )
            .map_err(BuiltinObjectProductionError::NativeRequirementSurface)?;
        let code = compute_cross_cone_code_fingerprint_v1(
            code_projection,
            native_requirements,
            defined_symbols,
            undefined_requirements,
        )
        .map_err(BuiltinObjectProductionError::CodeFingerprint)?;

        Ok(CrossConeCodeFingerprintedObjectProductionV1 {
            target_selection: production.target_selection,
            lir_foundation: production.foundation,
            c_bridge_profile: production.c_bridge_profile,
            members,
            code,
        })
    }
}

impl CrossConeCodeFingerprintedObjectProductionV1 {
    pub(crate) fn into_archive_parts(
        self,
    ) -> (
        ValidatedLirTargetSelection,
        OdrFreeLirFoundation,
        CBridgeToolchainProfileV1,
        Vec<SlibMember>,
        VerifiedCrossConeCodeFingerprintV1,
    ) {
        (
            self.target_selection,
            self.lir_foundation,
            self.c_bridge_profile,
            self.members,
            self.code,
        )
    }
}
