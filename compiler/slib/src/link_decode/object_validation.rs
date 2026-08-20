use super::*;

impl<'input> ProductionValidatedSingleConeLinkSections<'input> {
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

    pub fn declared_identity_count(&self) -> usize {
        self.identities.declared_identity_count()
    }

    pub fn hir_foundation(&self) -> &scoop_hir::CanonicalHirFoundation {
        &self.foundations.hir
    }

    pub fn mir_foundation(&self) -> &scoop_mir::CanonicalMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn link_identity_closure_wire(&self) -> &DecodedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_materializations(
        self,
    ) -> Result<MaterializationCheckedSingleConeLinkSections<'input>, StrongLinkMaterializationError>
    {
        let Self {
            mut graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            production_manifest,
        } = self;
        let partition = ProducerUnitPartitionV1::from_foundation(&foundations.lir)
            .map_err(StrongLinkMaterializationError::ProducerUnits)?;
        let link_identity_closure = link_identity_closure
            .validate_materializations(graph.target_selection().target(), &partition)
            .map_err(StrongLinkMaterializationError::Closure)?;
        let (scoop_objects, generated_bridge_objects) =
            object_directory::validate(&mut graph, link_identity_closure.member_plan())?;
        Ok(MaterializationCheckedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            generated_bridge_objects,
            production_manifest,
        })
    }
}

impl<'input> MaterializationCheckedSingleConeLinkSections<'input> {
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

    pub fn hir_foundation(&self) -> &scoop_hir::CanonicalHirFoundation {
        &self.foundations.hir
    }

    pub fn mir_foundation(&self) -> &scoop_mir::CanonicalMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn materializations(&self) -> &MaterializationCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub fn scoop_objects(&self) -> &[ScoopLirObjectCandidateV1<'_>] {
        &self.scoop_objects
    }

    pub fn generated_bridge_objects(&self) -> &[GeneratedCBridgeObjectCandidateV1<'_>] {
        &self.generated_bridge_objects
    }

    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_c_bridge_envelopes(
        self,
        profile: &CBridgeToolchainProfileV1,
    ) -> Result<CBridgeCheckedSingleConeLinkSections<'input>, StrongLinkCBridgeError> {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            generated_bridge_objects,
            production_manifest,
        } = self;
        let bridge_plan = production.lir().generated_bridge_plan();
        let production_manifest = production_manifest
            .validate_c_bridge_production(bridge_plan, profile)
            .map_err(StrongLinkCBridgeError::ManifestProduction)?;
        let c_bridge_production = crate::verify_c_bridge_production_envelopes_v1(
            bridge_plan.clone(),
            production_manifest.c_bridge_production().clone(),
            profile,
            link_identity_closure.member_plan(),
            &generated_bridge_objects,
        )
        .map_err(StrongLinkCBridgeError::ObjectEnvelopes)?;
        Ok(CBridgeCheckedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            generated_bridge_objects,
            c_bridge_production,
            production_manifest,
        })
    }
}

impl<'input> CBridgeCheckedSingleConeLinkSections<'input> {
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

    pub fn hir_foundation(&self) -> &scoop_hir::CanonicalHirFoundation {
        &self.foundations.hir
    }

    pub fn mir_foundation(&self) -> &scoop_mir::CanonicalMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn materializations(&self) -> &MaterializationCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub fn scoop_objects(&self) -> &[ScoopLirObjectCandidateV1<'_>] {
        &self.scoop_objects
    }

    pub fn generated_bridge_objects(&self) -> &[GeneratedCBridgeObjectCandidateV1<'_>] {
        &self.generated_bridge_objects
    }

    pub const fn c_bridge_production(&self) -> &VerifiedCBridgeProductionEnvelopeSetV1 {
        &self.c_bridge_production
    }

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_builtin_objects(
        self,
    ) -> Result<BuiltinObjectCheckedSingleConeLinkSections<'input>, StrongLinkBuiltinObjectError>
    {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects: final_scoop_objects,
            generated_bridge_objects,
            c_bridge_production,
            production_manifest,
        } = self;
        let link_identity_closure = link_identity_closure
            .validate_digest_patch_inputs(production.lir().digest_finalization_plan())
            .map_err(StrongLinkBuiltinObjectError::ClosureInput)?;
        let scoop_objects = crate::normalize_final_scoop_lir_objects_v1(
            link_identity_closure.member_plan(),
            &final_scoop_objects,
            link_identity_closure.provisional_patch_sites(),
        )
        .map_err(StrongLinkBuiltinObjectError::Normalization)?;
        let scoop_candidates = scoop_objects.candidates();
        let symbol_plan = PlannedStrongObjectSymbolSetV1::new(
            graph.target_selection().target(),
            production.lir().canonical_definitions(),
            link_identity_closure.member_plan(),
        )
        .map_err(StrongLinkBuiltinObjectError::SymbolPlan)?;
        let builtin_objects = crate::verify_builtin_object_strong_relocations_v1(
            link_identity_closure.member_plan(),
            &symbol_plan,
            &scoop_candidates,
            c_bridge_production,
            &generated_bridge_objects,
        )
        .map_err(StrongLinkBuiltinObjectError::ObjectSet)?;
        Ok(BuiltinObjectCheckedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            builtin_objects,
            production_manifest,
        })
    }
}

impl<'input> BuiltinObjectCheckedSingleConeLinkSections<'input> {
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

    pub fn hir_foundation(&self) -> &scoop_hir::CanonicalHirFoundation {
        &self.foundations.hir
    }

    pub fn mir_foundation(&self) -> &scoop_mir::CanonicalMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &ConeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn digest_patch_inputs(
        &self,
    ) -> &crate::DigestPatchInputCheckedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn scoop_objects(&self) -> &VerifiedNormalizedProvisionalScoopLirObjectSetV1 {
        &self.scoop_objects
    }

    pub const fn builtin_objects(&self) -> &VerifiedBuiltinObjectStrongRelocationSetV1 {
        &self.builtin_objects
    }

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_digest_patch_sites(
        self,
    ) -> Result<DigestPatchCheckedSingleConeLinkSections<'input>, StrongLinkDigestPatchError> {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            builtin_objects,
            production_manifest,
        } = self;
        let digest_plan = production.lir().digest_finalization_plan();
        let scoop_candidates = scoop_objects.candidates();
        let digest_patch_sites = crate::verify_scoop_lir_digest_patch_sites_v1(
            builtin_objects,
            &foundations.lir,
            digest_plan.clone(),
            &scoop_candidates,
            link_identity_closure.provisional_patch_sites(),
        )
        .map_err(StrongLinkDigestPatchError::ObjectSites)?;
        let link_identity_closure = link_identity_closure
            .validate_object_projections(&digest_patch_sites)
            .map_err(StrongLinkDigestPatchError::ClosureProjection)?;
        Ok(DigestPatchCheckedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            digest_patch_sites,
            production_manifest,
        })
    }
}

impl<'input> DigestPatchCheckedSingleConeLinkSections<'input> {
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

    pub fn hir_foundation(&self) -> &scoop_hir::CanonicalHirFoundation {
        &self.foundations.hir
    }

    pub fn mir_foundation(&self) -> &scoop_mir::CanonicalMirFoundation {
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

    pub const fn digest_patch_sites(&self) -> &VerifiedScoopLirDigestPatchSiteSetV1 {
        &self.digest_patch_sites
    }

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_registration_objects(
        self,
    ) -> Result<
        RegistrationObjectCheckedSingleConeLinkSections<'input>,
        StrongLinkRegistrationObjectError,
    > {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            digest_patch_sites,
            production_manifest,
        } = self;
        let registration_production = production.lir().registration_production();
        let safepoint_semantics = registration_production.safepoint_semantics();
        let safepoint_plan = registration_production.safepoints().clone();
        let callable_plan = registration_production.callables().clone();
        let type_plan = registration_production.types().clone();
        let immortal_object_plan = registration_production.immortal_objects().clone();
        let static_storage_plan = registration_production.static_storages().clone();
        let initialization_plan = registration_production.initialization_units().clone();
        let scoop_candidates = scoop_objects.candidates();

        let stackmaps = crate::verify_scoop_lir_stackmaps_v1(
            digest_patch_sites.builtins().clone(),
            safepoint_semantics,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::Stackmaps)?;
        let safepoint_registrations = crate::verify_strong_safepoint_registrations_v1(
            stackmaps,
            digest_patch_sites.clone(),
            safepoint_plan,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::Safepoints)?;
        let callable_registrations = crate::verify_strong_callable_registrations_v1(
            digest_patch_sites.clone(),
            callable_plan,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::Callables)?;
        let type_registrations = crate::verify_strong_type_registrations_v1(
            digest_patch_sites.clone(),
            type_plan,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::Types)?;
        let immortal_object_registrations = crate::verify_strong_immortal_object_registrations_v1(
            digest_patch_sites.clone(),
            immortal_object_plan,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::ImmortalObjects)?;
        let static_storage_registrations = crate::verify_strong_static_storage_registrations_v1(
            digest_patch_sites.clone(),
            static_storage_plan,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::StaticStorages)?;
        let initialization_registrations = crate::verify_strong_initialization_registrations_v1(
            digest_patch_sites,
            initialization_plan,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationObjectError::InitializationUnits)?;

        Ok(RegistrationObjectCheckedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            safepoint_registrations,
            callable_registrations,
            type_registrations,
            immortal_object_registrations,
            static_storage_registrations,
            initialization_registrations,
            production_manifest,
        })
    }
}

impl<'input> RegistrationObjectCheckedSingleConeLinkSections<'input> {
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

    pub fn hir_foundation(&self) -> &scoop_hir::CanonicalHirFoundation {
        &self.foundations.hir
    }

    pub fn mir_foundation(&self) -> &scoop_mir::CanonicalMirFoundation {
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

    pub const fn stackmaps(&self) -> &VerifiedScoopLirStackmapSetV1 {
        self.safepoint_registrations.stackmaps()
    }

    pub const fn safepoint_registrations(&self) -> &VerifiedStrongSafepointRegistrationSetV1 {
        &self.safepoint_registrations
    }

    pub const fn callable_registrations(&self) -> &VerifiedStrongCallableRegistrationSetV1 {
        &self.callable_registrations
    }

    pub const fn type_registrations(&self) -> &VerifiedStrongTypeRegistrationSetV1 {
        &self.type_registrations
    }

    pub const fn immortal_object_registrations(
        &self,
    ) -> &VerifiedStrongImmortalObjectRegistrationSetV1 {
        &self.immortal_object_registrations
    }

    pub const fn static_storage_registrations(
        &self,
    ) -> &VerifiedStrongStaticStorageRegistrationSetV1 {
        &self.static_storage_registrations
    }

    pub const fn initialization_registrations(
        &self,
    ) -> &VerifiedStrongInitializationRegistrationSetV1 {
        &self.initialization_registrations
    }

    pub const fn production_manifest(&self) -> &CBridgeCheckedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn fingerprint_registration_leaves(
        self,
    ) -> Result<
        RegistrationLeafFingerprintedSingleConeLinkSections<'input>,
        StrongLinkRegistrationLeafFingerprintError,
    > {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            safepoint_registrations,
            callable_registrations,
            type_registrations,
            immortal_object_registrations,
            static_storage_registrations,
            initialization_registrations,
            production_manifest,
        } = self;
        let safepoints = crate::compute_strong_safepoint_fingerprints_v1(safepoint_registrations)
            .map_err(StrongLinkRegistrationLeafFingerprintError::Safepoints)?;

        let immortal_object_registration_objects = immortal_object_registrations;
        let static_storage_registration_objects = static_storage_registrations;
        let initialization_registration_objects = initialization_registrations;

        Ok(RegistrationLeafFingerprintedSingleConeLinkSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            safepoints,
            callable_registrations,
            type_registrations,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
            production_manifest,
        })
    }
}
