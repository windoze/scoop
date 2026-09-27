use super::*;

impl<'input> RegistrationLeafFingerprintedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn object_projections(&self) -> &ObjectProjectionCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn scoop_objects(&self) -> &VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
        &self.scoop_objects
    }

    pub const fn safepoints(&self) -> &VerifiedStrongSafepointFingerprintSetV1 {
        &self.safepoints
    }

    pub const fn callable_registrations(&self) -> &VerifiedStrongCallableRegistrationSetV1 {
        &self.callable_registrations
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

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_link_symbol_requirements(
        self,
        dependency_owners: &[CanonicalDefinedLinkSymbolOwnerSetV1],
        c_bridge_profile: &CBridgeToolchainProfileV1,
    ) -> Result<LinkSymbolCheckedSingleConeLinkSections<'input>, StrongLinkSymbolRequirementError>
    {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            safepoints,
            callable_registrations,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
            production_manifest,
        } = self;
        let selection = graph.target_selection();
        let patch_sites = callable_registrations.patch_sites().clone();
        let strong_closure = patch_sites.builtins().strong_relocations().clone();
        let defined_symbols =
            CanonicalDefinedLinkSymbolOwnerSetV1::from_verified_strong_closure(&strong_closure)
                .map_err(StrongLinkSymbolRequirementError::DefinedSymbols)?;
        let bridge_plan = production.lir().generated_bridge_plan().clone();
        let current_cone = crate::verify_current_cone_undefined_requirements_v1(
            strong_closure.clone(),
            bridge_plan.clone(),
        )
        .map_err(StrongLinkSymbolRequirementError::CurrentCone)?;
        let native_requirements = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
            selection.target(),
            &foundations.lir,
        )
        .map_err(StrongLinkSymbolRequirementError::NativeSurface)?;
        let dependencies = crate::verify_dependency_strong_requirements_v1(
            selection.target(),
            strong_closure,
            dependency_owners,
        )
        .map_err(StrongLinkSymbolRequirementError::CrossCone)?;
        let source = crate::verify_source_external_requirements_v1(
            dependencies,
            native_requirements.clone(),
        )
        .map_err(StrongLinkSymbolRequirementError::SourceExternal)?;
        let runtime_and_eh = crate::verify_runtime_and_eh_requirements_v1(source, selection)
            .map_err(StrongLinkSymbolRequirementError::RuntimeAndEh)?;
        let bridge_semantics = crate::verify_generated_c_bridge_semantics_v1(
            patch_sites,
            bridge_plan,
            native_requirements,
            c_bridge_profile,
        )
        .map_err(StrongLinkSymbolRequirementError::GeneratedBridgeSemantics)?;
        let external =
            crate::verify_c_bridge_target_support_requirements_v1(runtime_and_eh, bridge_semantics)
                .map_err(StrongLinkSymbolRequirementError::CBridgeTargetSupport)?;
        let external = crate::seal_builtin_object_external_requirements_v1(external)
            .map_err(StrongLinkSymbolRequirementError::UnclassifiedExternal)?;
        let undefined_symbols =
            crate::finalize_undefined_symbol_requirements_v1(current_cone, external)
                .map_err(StrongLinkSymbolRequirementError::UndefinedSymbols)?;
        let link_identity_closure = link_identity_closure
            .validate_symbol_projections(&defined_symbols, &undefined_symbols)
            .map_err(StrongLinkSymbolRequirementError::ClosureProjection)?;

        Ok(LinkSymbolCheckedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callable_registrations,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
            production_manifest,
        })
    }
}

impl<'input> LinkSymbolCheckedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn symbol_projections(&self) -> &SymbolProjectionCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn scoop_objects(&self) -> &VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
        &self.scoop_objects
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

    pub const fn callable_registrations(&self) -> &VerifiedStrongCallableRegistrationSetV1 {
        &self.callable_registrations
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

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn fingerprint_registration_dependencies(
        self,
    ) -> Result<
        RegistrationDependencyFingerprintedSingleConeLinkSections<'input>,
        StrongLinkRegistrationDependencyFingerprintError,
    > {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callable_registrations,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
            production_manifest,
        } = self;
        let scoop_candidates = scoop_objects.candidates();

        let stackmaps = safepoints.registrations().stackmaps().clone();
        let callable_bodies = crate::compute_strong_callable_body_object_fingerprints_v1(
            callable_registrations,
            stackmaps,
            undefined_symbols.clone(),
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationDependencyFingerprintError::CallableBodies)?;
        let callables = crate::compute_strong_callable_fingerprints_v1(
            callable_bodies,
            production.lir().canonical_callable_definitions(),
        )
        .map_err(StrongLinkRegistrationDependencyFingerprintError::Callables)?;

        let type_dependencies = crate::compute_strong_type_dependency_fingerprints_v1(
            type_registration_objects,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationDependencyFingerprintError::TypeDependencies)?;
        let types = crate::compute_strong_type_fingerprints_v1(type_dependencies)
            .map_err(StrongLinkRegistrationDependencyFingerprintError::Types)?;

        let immortal_object_definitions =
            crate::compute_strong_immortal_object_definition_fingerprints_v1(
                immortal_object_registration_objects,
                undefined_symbols.clone(),
                &scoop_candidates,
            )
            .map_err(StrongLinkRegistrationDependencyFingerprintError::ImmortalObjectDefinitions)?;
        let immortal_objects =
            crate::compute_strong_immortal_object_fingerprints_v1(immortal_object_definitions)
                .map_err(StrongLinkRegistrationDependencyFingerprintError::ImmortalObjects)?;

        let static_storage_definitions =
            crate::compute_strong_static_storage_definition_fingerprints_v1(
                static_storage_registration_objects,
                &scoop_candidates,
            )
            .map_err(StrongLinkRegistrationDependencyFingerprintError::StaticStorageDefinitions)?;
        let static_storage_shapes =
            crate::compute_strong_static_storage_shape_fingerprints_v1(static_storage_definitions)
                .map_err(StrongLinkRegistrationDependencyFingerprintError::StaticStorageShapes)?;
        let static_storages =
            crate::compute_strong_static_storage_fingerprints_v1(static_storage_shapes)
                .map_err(StrongLinkRegistrationDependencyFingerprintError::StaticStorages)?;

        let initialization_definitions =
            crate::compute_strong_initialization_definition_fingerprints_v1(
                initialization_registration_objects,
                &scoop_candidates,
            )
            .map_err(StrongLinkRegistrationDependencyFingerprintError::InitializationDefinitions)?;
        let initializations = crate::compute_strong_initialization_fingerprints_v1(
            initialization_definitions,
            callables.body_objects(),
        )
        .map_err(StrongLinkRegistrationDependencyFingerprintError::Initializations)?;

        Ok(RegistrationDependencyFingerprintedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initializations,
            production_manifest,
        })
    }
}

impl<'input> RegistrationDependencyFingerprintedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn symbol_projections(&self) -> &SymbolProjectionCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn scoop_objects(&self) -> &VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
        &self.scoop_objects
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

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn finalize_strong_objects(
        self,
    ) -> Result<FinalizedStrongLinkObjectSections<'input>, StrongLinkObjectFinalizationError> {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            defined_symbols,
            undefined_symbols,
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initializations,
            production_manifest,
        } = self;
        let scoop_candidates = scoop_objects.candidates();

        let patch_sites = safepoints.registrations().patch_sites().clone();
        let image = crate::verify_cone_image_v1(
            patch_sites.clone(),
            production.lir().image_plan().clone(),
            &scoop_candidates,
        )
        .map_err(StrongLinkObjectFinalizationError::ImageValidation)?;
        let entry = crate::verify_entry_production_v1(
            patch_sites,
            production.lir().entry_plan().clone(),
            &scoop_candidates,
        )
        .map_err(StrongLinkObjectFinalizationError::EntryValidation)?;
        let registrations = crate::patch_strong_registration_fingerprints_v1(
            safepoints,
            callables,
            types,
            immortal_objects,
            static_storages,
            initializations,
            &scoop_candidates,
        )
        .map_err(StrongLinkObjectFinalizationError::RegistrationPatch)?;
        let image_fingerprint = crate::compute_runtime_image_fingerprint_v1(
            image,
            registrations,
            graph.envelope.manifest().compatibility().clone(),
        )
        .map_err(StrongLinkObjectFinalizationError::ImageFingerprint)?;
        let runtime_images = crate::patch_runtime_image_fingerprint_v1(image_fingerprint)
            .map_err(StrongLinkObjectFinalizationError::ImagePatch)?;
        let final_objects = crate::patch_entry_production_v1(runtime_images, entry)
            .map_err(StrongLinkObjectFinalizationError::EntryPatch)?;
        verify_reconstructed_scoop_objects(&scoop_objects, &final_objects)
            .map_err(StrongLinkObjectFinalizationError::FinalObjectMismatch)?;

        Ok(FinalizedStrongLinkObjectSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            defined_symbols,
            undefined_symbols,
            final_objects,
            production_manifest,
        })
    }
}

impl<'input> FinalizedStrongLinkObjectSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn symbol_projections(&self) -> &SymbolProjectionCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
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

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_code_and_closure(
        self,
    ) -> Result<ValidatedSingleConeStrongLinkArtifact<'input>, StrongLinkFinalValidationError> {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            defined_symbols,
            undefined_symbols,
            final_objects,
            production_manifest,
        } = self;
        let manifest = graph.envelope.manifest();
        let native_requirements = CanonicalNativeExternalRequirementSurfaceV1::from_foundation(
            graph.target_selection().target(),
            &foundations.lir,
        )
        .map_err(StrongLinkFinalValidationError::NativeSurface)?;
        let link_objects =
            crate::verify_code_link_object_members_v1(final_objects, manifest.members())
                .map_err(StrongLinkFinalValidationError::LinkObjectMembers)?;
        let code_projection = crate::verify_single_cone_production_code_projection_v1(
            manifest.cone(),
            manifest.direct_dependencies(),
            foundations
                .hir
                .as_canonical()
                .source_count_for_cone(manifest.cone().identity()),
            production.lir().clone(),
            link_objects,
        )
        .map_err(StrongLinkFinalValidationError::ProductionProjection)?;
        let code = crate::compute_code_fingerprint_v1(
            code_projection,
            native_requirements,
            defined_symbols,
            undefined_symbols,
        )
        .map_err(StrongLinkFinalValidationError::CodeFingerprint)?;
        let runtime_image = code
            .production()
            .link_objects()
            .final_objects()
            .runtime_images()
            .fingerprint()
            .fingerprint();
        let semantic = manifest.semantic_fingerprints();
        if semantic.code() != crate::FingerprintAvailability::Available(code.fingerprint())
            || semantic.runtime_image() != crate::FingerprintAvailability::Available(runtime_image)
        {
            return Err(StrongLinkFinalValidationError::SemanticFingerprintMismatch);
        }
        let production_manifest = production_manifest
            .validate(&code)
            .map_err(StrongLinkFinalValidationError::ProductionManifest)?;
        let link_identity_closure = link_identity_closure
            .validate(&code)
            .map_err(StrongLinkFinalValidationError::LinkIdentityClosure)?;
        Ok(ValidatedSingleConeStrongLinkArtifact {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            production_manifest,
        })
    }
}

impl ValidatedSingleConeStrongLinkArtifact<'_> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn kind(&self) -> ConeKind {
        self.graph.kind()
    }

    pub const fn source_form(&self) -> ConeSourceForm {
        self.graph.source_form()
    }

    pub const fn compatibility(&self) -> &crate::CompatibilityRecord {
        self.graph.compatibility()
    }

    pub const fn target_selection(&self) -> scoop_lir::ValidatedLirTargetSelection {
        self.graph.target_selection()
    }

    pub fn direct_dependencies(&self) -> &[crate::DependencyRecord] {
        self.graph.direct_dependencies()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn semantic_fingerprints(&self) -> SemanticFingerprintRecord {
        self.graph.envelope.manifest().semantic_fingerprints()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn link_identity_closure(&self) -> &LinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn production_manifest(&self) -> &SingleConeProductionManifestV1 {
        &self.production_manifest
    }
}
