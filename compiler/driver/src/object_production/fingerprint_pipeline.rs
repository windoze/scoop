//! Link-closure verification and terminal strong-object fingerprint pipeline.

use super::*;

/// All six registration-object leaf families fingerprinted from one exact
/// provisional Scoop object set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationObjectLeafFingerprintedProductionV1 {
    pub(super) production: PlannedBuiltinObjectProductionV1,
    pub(super) symbol_plan: PlannedStrongObjectSymbolSetV1,
    pub(super) safepoints: VerifiedStrongSafepointFingerprintSetV1,
    pub(super) callable_registration_objects:
        VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    pub(super) type_registration_objects: VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
    pub(super) immortal_object_registration_objects:
        VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    pub(super) static_storage_registration_objects:
        VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    pub(super) initialization_registration_objects:
        VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
}

impl RegistrationObjectLeafFingerprintedProductionV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) const fn from_verified_leaves(
        production: PlannedBuiltinObjectProductionV1,
        symbol_plan: PlannedStrongObjectSymbolSetV1,
        safepoints: VerifiedStrongSafepointFingerprintSetV1,
        callable_registration_objects: VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
        type_registration_objects: VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
        immortal_object_registration_objects: VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
        static_storage_registration_objects: VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
        initialization_registration_objects: VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
    ) -> Self {
        Self {
            production,
            symbol_plan,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
        }
    }

    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callable_registration_objects(
        &self,
    ) -> &VerifiedStrongCallableRegistrationObjectFingerprintSetV1 {
        &self.callable_registration_objects
    }

    pub const fn type_registration_objects(
        &self,
    ) -> &VerifiedStrongTypeRegistrationObjectFingerprintSetV1 {
        &self.type_registration_objects
    }

    pub const fn immortal_object_registration_objects(
        &self,
    ) -> &VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1 {
        &self.immortal_object_registration_objects
    }

    pub const fn static_storage_registration_objects(
        &self,
    ) -> &VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1 {
        &self.static_storage_registration_objects
    }

    pub const fn initialization_registration_objects(
        &self,
    ) -> &VerifiedStrongInitializationRegistrationObjectFingerprintSetV1 {
        &self.initialization_registration_objects
    }

    pub fn verify_link_symbol_requirements(
        self,
        core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
    ) -> Result<LinkSymbolVerifiedObjectProductionV1, BuiltinObjectProductionError> {
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
        let source = verify_source_external_requirements_v1(core, native_requirements.clone())
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
        let undefined_symbols = finalize_undefined_symbol_requirements_v1(current_cone, external)
            .map_err(BuiltinObjectProductionError::UndefinedSymbols)?;

        Ok(LinkSymbolVerifiedObjectProductionV1 {
            production,
            symbol_plan,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
        })
    }
}

/// Complete member-aware defined and undefined symbol closure for the exact
/// built-in object bytes whose registration leaves were fingerprinted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkSymbolVerifiedObjectProductionV1 {
    production: PlannedBuiltinObjectProductionV1,
    symbol_plan: PlannedStrongObjectSymbolSetV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
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

impl LinkSymbolVerifiedObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined_symbols
    }

    pub const fn undefined_symbols(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.undefined_symbols
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callable_registration_objects(
        &self,
    ) -> &VerifiedStrongCallableRegistrationObjectFingerprintSetV1 {
        &self.callable_registration_objects
    }

    pub const fn type_registration_objects(
        &self,
    ) -> &VerifiedStrongTypeRegistrationObjectFingerprintSetV1 {
        &self.type_registration_objects
    }

    pub const fn immortal_object_registration_objects(
        &self,
    ) -> &VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1 {
        &self.immortal_object_registration_objects
    }

    pub const fn static_storage_registration_objects(
        &self,
    ) -> &VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1 {
        &self.static_storage_registration_objects
    }

    pub const fn initialization_registration_objects(
        &self,
    ) -> &VerifiedStrongInitializationRegistrationObjectFingerprintSetV1 {
        &self.initialization_registration_objects
    }

    pub fn fingerprint_registration_dependencies(
        self,
    ) -> Result<RegistrationDependencyFingerprintedProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            defined_symbols,
            undefined_symbols,
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
            let callable_bodies = compute_strong_callable_body_object_fingerprints_v1(
                callable_registration_objects,
                stackmaps,
                undefined_symbols.clone(),
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

            let immortal_object_definitions =
                compute_strong_immortal_object_definition_fingerprints_v1(
                    immortal_object_registration_objects,
                    undefined_symbols.clone(),
                    &candidates,
                )
                .map_err(BuiltinObjectProductionError::ImmortalObjectDefinitionFingerprints)?;
            let immortal_objects =
                compute_strong_immortal_object_fingerprints_v1(immortal_object_definitions)
                    .map_err(BuiltinObjectProductionError::ImmortalObjectFingerprints)?;

            let static_storage_definitions =
                compute_strong_static_storage_definition_fingerprints_v1(
                    static_storage_registration_objects,
                    &candidates,
                )
                .map_err(BuiltinObjectProductionError::StaticStorageDefinitionFingerprints)?;
            let static_storage_shapes =
                compute_strong_static_storage_shape_fingerprints_v1(static_storage_definitions)
                    .map_err(BuiltinObjectProductionError::StaticStorageShapeFingerprints)?;
            let static_storages =
                compute_strong_static_storage_fingerprints_v1(static_storage_shapes)
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

        Ok(RegistrationDependencyFingerprintedProductionV1 {
            production,
            symbol_plan,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initializations,
        })
    }
}

/// Complete strong-registration fingerprints after every object and semantic
/// dependency leaf has been closed against the same object proof.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationDependencyFingerprintedProductionV1 {
    production: PlannedBuiltinObjectProductionV1,
    symbol_plan: PlannedStrongObjectSymbolSetV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: VerifiedStrongTypeFingerprintSetV1,
    immortal_objects: VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: VerifiedStrongInitializationFingerprintSetV1,
}

impl RegistrationDependencyFingerprintedProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined_symbols
    }

    pub const fn undefined_symbols(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.undefined_symbols
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callables(&self) -> &VerifiedStrongCallableFingerprintSetV1 {
        &self.callables
    }

    pub const fn types(&self) -> &VerifiedStrongTypeFingerprintSetV1 {
        &self.types
    }

    pub const fn immortal_objects(&self) -> &VerifiedStrongImmortalObjectFingerprintSetV1 {
        &self.immortal_objects
    }

    pub const fn static_storages(&self) -> &VerifiedStrongStaticStorageFingerprintSetV1 {
        &self.static_storages
    }

    pub const fn initializations(&self) -> &VerifiedStrongInitializationFingerprintSetV1 {
        &self.initializations
    }

    pub fn finalize_strong_objects(
        self,
    ) -> Result<FinalizedStrongObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan,
            defined_symbols,
            undefined_symbols,
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
                ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
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

        Ok(FinalizedStrongObjectProductionV1 {
            production,
            symbol_plan,
            defined_symbols,
            undefined_symbols,
            final_objects,
        })
    }
}

/// Final Scoop object bytes after every strong registration, runtime-image,
/// and entry digest write has been applied and revalidated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedStrongObjectProductionV1 {
    production: PlannedBuiltinObjectProductionV1,
    symbol_plan: PlannedStrongObjectSymbolSetV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_symbols: CanonicalUndefinedSymbolRequirementSetV1,
    final_objects: VerifiedEntryPatchSetV1,
}

impl FinalizedStrongObjectProductionV1 {
    pub const fn production(&self) -> &PlannedBuiltinObjectProductionV1 {
        &self.production
    }

    pub const fn symbol_plan(&self) -> &PlannedStrongObjectSymbolSetV1 {
        &self.symbol_plan
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined_symbols
    }

    pub const fn undefined_symbols(&self) -> &CanonicalUndefinedSymbolRequirementSetV1 {
        &self.undefined_symbols
    }

    pub const fn final_objects(&self) -> &VerifiedEntryPatchSetV1 {
        &self.final_objects
    }

    pub fn fingerprint_code(
        self,
        cone: &ConeRecord,
        direct_dependencies: &[DependencyRecord],
        source_count: usize,
    ) -> Result<CodeFingerprintedObjectProductionV1, BuiltinObjectProductionError> {
        let Self {
            production,
            symbol_plan: _,
            defined_symbols,
            undefined_symbols,
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
        let code = compute_code_fingerprint_v1(
            code_projection,
            native_requirements,
            defined_symbols,
            undefined_symbols,
        )
        .map_err(BuiltinObjectProductionError::CodeFingerprint)?;
        let production_manifest = SingleConeProductionManifestV1::from_verified_code(code);

        Ok(CodeFingerprintedObjectProductionV1 {
            target_selection: production.target_selection,
            lir_foundation: production.foundation,
            c_bridge_profile: production.c_bridge_profile,
            members,
            production_manifest,
        })
    }
}

pub(super) fn final_link_object_members(
    production: &PlannedBuiltinObjectProductionV1,
    final_objects: &VerifiedEntryPatchSetV1,
) -> Result<Vec<SlibMember>, BuiltinObjectProductionError> {
    let producer = production.member_plan.producer();
    let mut members = Vec::with_capacity(
        final_objects.objects().len() + production.generated_c_bridge_members.len(),
    );
    for object in final_objects.objects() {
        let plan = production
            .member_plan
            .scoop_lir_members()
            .iter()
            .find(|plan| plan.member_id() == object.member())
            .ok_or(BuiltinObjectProductionError::MissingFinalMemberPlan(
                object.member(),
            ))?;
        let member = SlibMember::new(
            producer,
            plan.stable_key().clone(),
            plan.role().clone(),
            object.bytes().to_vec(),
        )
        .map_err(BuiltinObjectProductionError::FinalMember)?;
        require_final_member_id(&member, object.member())?;
        members.push(member);
    }
    for object in &production.generated_c_bridge_members {
        let member = SlibMember::new(
            producer,
            object.plan.stable_key().clone(),
            object.plan.role().clone(),
            object.bytes.clone(),
        )
        .map_err(BuiltinObjectProductionError::FinalMember)?;
        require_final_member_id(&member, object.plan.member_id())?;
        members.push(member);
    }
    members.sort_unstable_by_key(|member| member.record().id());
    Ok(members)
}

fn require_final_member_id(
    member: &SlibMember,
    expected: SlibMemberId,
) -> Result<(), BuiltinObjectProductionError> {
    let actual = member.record().id();
    if actual == expected {
        Ok(())
    } else {
        Err(BuiltinObjectProductionError::FinalMemberIdMismatch { expected, actual })
    }
}

/// Final LinkObject members plus the unique Code/production-manifest proof.
/// Provisional object bytes and their earlier state-machine proofs are no
/// longer retained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeFingerprintedObjectProductionV1 {
    target_selection: ValidatedLirTargetSelection,
    lir_foundation: OdrFreeLirFoundation,
    c_bridge_profile: CBridgeToolchainProfileV1,
    members: Vec<SlibMember>,
    production_manifest: SingleConeProductionManifestV1,
}

impl CodeFingerprintedObjectProductionV1 {
    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target_selection
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.lir_foundation
    }

    pub const fn c_bridge_profile(&self) -> &CBridgeToolchainProfileV1 {
        &self.c_bridge_profile
    }

    pub fn members(&self) -> &[SlibMember] {
        &self.members
    }

    pub const fn production_manifest(&self) -> &SingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub(crate) fn into_archive_parts(
        self,
    ) -> (
        ValidatedLirTargetSelection,
        OdrFreeLirFoundation,
        CBridgeToolchainProfileV1,
        Vec<SlibMember>,
        SingleConeProductionManifestV1,
    ) {
        (
            self.target_selection,
            self.lir_foundation,
            self.c_bridge_profile,
            self.members,
            self.production_manifest,
        )
    }
}
