//! Link validation extensions for the cross-Cone semantics strong profile.

use super::*;

pub(super) struct DecodedCrossConeLinkSections<'input> {
    pub(super) common: DecodedSingleConeLinkSections<'input>,
    pub(super) cross_cone_link_closure: DecodedCrossConeLinkClosureSectionV1,
}

/// Cross-Cone Link state whose legacy and dependency undefined uses have
/// been rebuilt as disjoint, complete partitions.
pub struct CrossConeLinkSymbolCheckedSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: SymbolProjectionCheckedLinkIdentityClosureSectionV1,
    cross_cone_link_closure: DecodedCrossConeLinkClosureSectionV1,
    scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_partitions: FinalizedUndefinedSymbolRequirementPartitionsV1,
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callable_registration_objects: VerifiedStrongCallableRegistrationObjectFingerprintSetV1,
    type_registration_objects: VerifiedStrongTypeRegistrationObjectFingerprintSetV1,
    immortal_object_registration_objects:
        VerifiedStrongImmortalObjectRegistrationObjectFingerprintSetV1,
    static_storage_registration_objects:
        VerifiedStrongStaticStorageRegistrationObjectFingerprintSetV1,
    initialization_registration_objects:
        VerifiedStrongInitializationRegistrationObjectFingerprintSetV1,
    production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Cross-Cone Link state after every registration dependency fingerprint has
/// consumed the same partitioned undefined-use authority.
pub struct CrossConeRegistrationDependencyFingerprintedSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: SymbolProjectionCheckedLinkIdentityClosureSectionV1,
    cross_cone_link_closure: DecodedCrossConeLinkClosureSectionV1,
    scoop_objects: VerifiedNormalizedProvisionalScoopLirObjectSetV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_partitions: FinalizedUndefinedSymbolRequirementPartitionsV1,
    safepoints: VerifiedStrongSafepointFingerprintSetV1,
    callables: VerifiedStrongCallableFingerprintSetV1,
    types: VerifiedStrongTypeFingerprintSetV1,
    immortal_objects: VerifiedStrongImmortalObjectFingerprintSetV1,
    static_storages: VerifiedStrongStaticStorageFingerprintSetV1,
    initializations: VerifiedStrongInitializationFingerprintSetV1,
    production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Cross-Cone Link state after all final object patches have been replayed.
pub struct FinalizedCrossConeStrongLinkObjectSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    link_identity_closure: SymbolProjectionCheckedLinkIdentityClosureSectionV1,
    cross_cone_link_closure: DecodedCrossConeLinkClosureSectionV1,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    undefined_partitions: FinalizedUndefinedSymbolRequirementPartitionsV1,
    final_objects: VerifiedEntryPatchSetV1,
    production_manifest: CBridgeCheckedSingleConeProductionManifestV1,
}

/// Fully validated Link view for one `CrossConeSemanticsStrongProfile`
/// artifact, including its independently reconstructed physical-use closure.
pub struct ValidatedCrossConeStrongLinkArtifact<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    production: ValidatedSingleConeStrongProduction,
    defined_symbols: CanonicalDefinedLinkSymbolOwnerSetV1,
    link_identity_closure: LinkIdentityClosureSectionV1,
    cross_cone_link_closure: CrossConeLinkClosureSectionV1,
    production_manifest: SingleConeProductionManifestV1,
}

#[allow(clippy::too_many_arguments)]
pub fn validate_cross_cone_strong_link_artifact<'input>(
    graph: ValidatedGraphArtifact<'input>,
    expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
    lir_cross_cone_bridge: &scoop_lir::CrossConeLirBridgeSectionV1,
    core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<ValidatedCrossConeStrongLinkArtifact<'input>, StrongLinkArtifactValidationError> {
    let DecodedCrossConeLinkSections {
        common,
        cross_cone_link_closure,
    } = graph
        .decode_cross_cone_link_sections()
        .map_err(|error| StrongLinkArtifactValidationError::Decode(Box::new(error)))?;
    common
        .validate_identities()
        .map_err(|error| StrongLinkArtifactValidationError::Identities(Box::new(error)))?
        .validate_foundation_structure()
        .map_err(|error| StrongLinkArtifactValidationError::Foundations(Box::new(error)))?
        .validate_production(expected_external_bridges)
        .map_err(|error| StrongLinkArtifactValidationError::Production(Box::new(error)))?
        .validate_materializations()
        .map_err(|error| StrongLinkArtifactValidationError::Materializations(Box::new(error)))?
        .validate_c_bridge_envelopes(c_bridge_profile)
        .map_err(|error| StrongLinkArtifactValidationError::CBridge(Box::new(error)))?
        .validate_builtin_objects()
        .map_err(|error| StrongLinkArtifactValidationError::BuiltinObjects(Box::new(error)))?
        .validate_digest_patch_sites()
        .map_err(|error| StrongLinkArtifactValidationError::DigestPatches(Box::new(error)))?
        .validate_registration_objects()
        .map_err(|error| StrongLinkArtifactValidationError::RegistrationObjects(Box::new(error)))?
        .fingerprint_registration_leaves()
        .map_err(|error| StrongLinkArtifactValidationError::RegistrationLeaves(Box::new(error)))?
        .validate_cross_cone_link_symbol_requirements(
            lir_cross_cone_bridge,
            cross_cone_link_closure,
            core_owners,
            c_bridge_profile,
        )
        .map_err(|error| StrongLinkArtifactValidationError::Symbols(Box::new(error)))?
        .fingerprint_registration_dependencies()
        .map_err(|error| {
            StrongLinkArtifactValidationError::RegistrationDependencies(Box::new(error))
        })?
        .finalize_strong_objects()
        .map_err(|error| StrongLinkArtifactValidationError::ObjectFinalization(Box::new(error)))?
        .validate_code_and_closures()
        .map_err(|error| StrongLinkArtifactValidationError::FinalProof(Box::new(error)))
}

pub fn validate_self_describing_cross_cone_strong_link_artifact<'input>(
    graph: ValidatedGraphArtifact<'input>,
    lir_cross_cone_bridge: &scoop_lir::CrossConeLirBridgeSectionV1,
    core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<ValidatedCrossConeStrongLinkArtifact<'input>, StrongLinkArtifactValidationError> {
    validate_self_describing_cross_cone_strong_link_artifact_with_authorities(
        graph,
        std::iter::empty(),
        lir_cross_cone_bridge,
        core_owners,
        c_bridge_profile,
    )
}

pub(crate) fn validate_self_describing_cross_cone_strong_link_artifact_with_authorities<
    'input,
    'authority,
>(
    graph: ValidatedGraphArtifact<'input>,
    external_authorities: impl IntoIterator<Item = &'authority ValidatedIdentityGraph>,
    lir_cross_cone_bridge: &scoop_lir::CrossConeLirBridgeSectionV1,
    core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
    c_bridge_profile: &CBridgeToolchainProfileV1,
) -> Result<ValidatedCrossConeStrongLinkArtifact<'input>, StrongLinkArtifactValidationError> {
    let DecodedCrossConeLinkSections {
        common,
        cross_cone_link_closure,
    } = graph
        .decode_cross_cone_link_sections()
        .map_err(|error| StrongLinkArtifactValidationError::Decode(Box::new(error)))?;
    let mut front = common
        .validate_identities_with_authorities(external_authorities)
        .map_err(|error| StrongLinkArtifactValidationError::Identities(Box::new(error)))?
        .validate_foundation_structure()
        .map_err(|error| StrongLinkArtifactValidationError::Foundations(Box::new(error)))?;
    let external_bridges = front
        .reconstruct_external_bridges()
        .map_err(|error| StrongLinkArtifactValidationError::ExternalBridges(Box::new(error)))?;
    front
        .validate_production(&external_bridges)
        .map_err(|error| StrongLinkArtifactValidationError::Production(Box::new(error)))?
        .validate_materializations()
        .map_err(|error| StrongLinkArtifactValidationError::Materializations(Box::new(error)))?
        .validate_c_bridge_envelopes(c_bridge_profile)
        .map_err(|error| StrongLinkArtifactValidationError::CBridge(Box::new(error)))?
        .validate_builtin_objects()
        .map_err(|error| StrongLinkArtifactValidationError::BuiltinObjects(Box::new(error)))?
        .validate_digest_patch_sites()
        .map_err(|error| StrongLinkArtifactValidationError::DigestPatches(Box::new(error)))?
        .validate_registration_objects()
        .map_err(|error| StrongLinkArtifactValidationError::RegistrationObjects(Box::new(error)))?
        .fingerprint_registration_leaves()
        .map_err(|error| StrongLinkArtifactValidationError::RegistrationLeaves(Box::new(error)))?
        .validate_cross_cone_link_symbol_requirements(
            lir_cross_cone_bridge,
            cross_cone_link_closure,
            core_owners,
            c_bridge_profile,
        )
        .map_err(|error| StrongLinkArtifactValidationError::Symbols(Box::new(error)))?
        .fingerprint_registration_dependencies()
        .map_err(|error| {
            StrongLinkArtifactValidationError::RegistrationDependencies(Box::new(error))
        })?
        .finalize_strong_objects()
        .map_err(|error| StrongLinkArtifactValidationError::ObjectFinalization(Box::new(error)))?
        .validate_code_and_closures()
        .map_err(|error| StrongLinkArtifactValidationError::FinalProof(Box::new(error)))
}

impl<'input> RegistrationLeafFingerprintedSingleConeLinkSections<'input> {
    #[allow(clippy::too_many_arguments)]
    pub fn validate_cross_cone_link_symbol_requirements(
        self,
        lir_cross_cone_bridge: &scoop_lir::CrossConeLirBridgeSectionV1,
        cross_cone_link_closure: DecodedCrossConeLinkClosureSectionV1,
        core_owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
        c_bridge_profile: &CBridgeToolchainProfileV1,
    ) -> Result<CrossConeLinkSymbolCheckedSections<'input>, StrongLinkSymbolRequirementError> {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            scoop_objects,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
            production_manifest,
        } = self;
        let selection = graph.target_selection();
        let patch_sites = callable_registration_objects
            .registrations()
            .patch_sites()
            .clone();
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
        let core = crate::verify_core_strong_requirements_v1(
            selection.target(),
            strong_closure,
            production.lir().external_bridges().clone(),
            core_owners.clone(),
        )
        .map_err(StrongLinkSymbolRequirementError::Core)?;
        let cross_cone =
            crate::verify_cross_cone_strong_requirements_v1(core, lir_cross_cone_bridge)
                .map_err(StrongLinkSymbolRequirementError::CrossCone)?;
        let source = crate::verify_source_external_requirements_after_cross_cone_v1(
            cross_cone,
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
        let undefined_partitions =
            crate::finalize_partitioned_undefined_symbol_requirements_v1(current_cone, external)
                .map_err(StrongLinkSymbolRequirementError::UndefinedSymbols)?;
        cross_cone_link_closure
            .validate_semantic_imports_against(undefined_partitions.cross_cone().semantic_imports())
            .map_err(StrongLinkSymbolRequirementError::CrossConeClosure)?;
        let link_identity_closure = link_identity_closure
            .validate_symbol_projections(&defined_symbols, undefined_partitions.legacy())
            .map_err(StrongLinkSymbolRequirementError::ClosureProjection)?;

        Ok(CrossConeLinkSymbolCheckedSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            cross_cone_link_closure,
            scoop_objects,
            defined_symbols,
            undefined_partitions,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
            production_manifest,
        })
    }
}

impl<'input> CrossConeLinkSymbolCheckedSections<'input> {
    pub const fn undefined_partitions(&self) -> &FinalizedUndefinedSymbolRequirementPartitionsV1 {
        &self.undefined_partitions
    }

    pub fn fingerprint_registration_dependencies(
        self,
    ) -> Result<
        CrossConeRegistrationDependencyFingerprintedSections<'input>,
        StrongLinkRegistrationDependencyFingerprintError,
    > {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            cross_cone_link_closure,
            scoop_objects,
            defined_symbols,
            undefined_partitions,
            safepoints,
            callable_registration_objects,
            type_registration_objects,
            immortal_object_registration_objects,
            static_storage_registration_objects,
            initialization_registration_objects,
            production_manifest,
        } = self;
        let scoop_candidates = scoop_objects.candidates();
        let stackmaps = safepoints.registrations().stackmaps().clone();
        let callable_bodies =
            crate::compute_cross_cone_strong_callable_body_object_fingerprints_v1(
                callable_registration_objects,
                stackmaps,
                undefined_partitions.clone(),
                &scoop_candidates,
            )
            .map_err(StrongLinkRegistrationDependencyFingerprintError::CallableBodies)?;
        let callables = crate::compute_strong_callable_fingerprints_v1(callable_bodies)
            .map_err(StrongLinkRegistrationDependencyFingerprintError::Callables)?;
        let type_dependencies = crate::compute_strong_type_dependency_fingerprints_v1(
            type_registration_objects,
            &scoop_candidates,
        )
        .map_err(StrongLinkRegistrationDependencyFingerprintError::TypeDependencies)?;
        let types = crate::compute_strong_type_fingerprints_v1(type_dependencies)
            .map_err(StrongLinkRegistrationDependencyFingerprintError::Types)?;
        let immortal_object_definitions =
            crate::compute_cross_cone_strong_immortal_object_definition_fingerprints_v1(
                immortal_object_registration_objects,
                undefined_partitions.clone(),
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

        Ok(CrossConeRegistrationDependencyFingerprintedSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            cross_cone_link_closure,
            scoop_objects,
            defined_symbols,
            undefined_partitions,
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

impl<'input> CrossConeRegistrationDependencyFingerprintedSections<'input> {
    pub fn finalize_strong_objects(
        self,
    ) -> Result<FinalizedCrossConeStrongLinkObjectSections<'input>, StrongLinkObjectFinalizationError>
    {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            cross_cone_link_closure,
            scoop_objects,
            defined_symbols,
            undefined_partitions,
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

        Ok(FinalizedCrossConeStrongLinkObjectSections {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            cross_cone_link_closure,
            defined_symbols,
            undefined_partitions,
            final_objects,
            production_manifest,
        })
    }
}

impl<'input> FinalizedCrossConeStrongLinkObjectSections<'input> {
    pub fn validate_code_and_closures(
        self,
    ) -> Result<ValidatedCrossConeStrongLinkArtifact<'input>, StrongLinkFinalValidationError> {
        let Self {
            graph,
            identities,
            foundations,
            production,
            link_identity_closure,
            cross_cone_link_closure,
            defined_symbols,
            undefined_partitions,
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
        let code_projection = crate::verify_cross_cone_production_code_projection_v1(
            manifest.cone(),
            manifest.direct_dependencies(),
            foundations.hir.as_canonical().counts().sources,
            production.lir().clone(),
            link_objects,
        )
        .map_err(StrongLinkFinalValidationError::ProductionProjection)?;
        let cross_cone_code = crate::compute_cross_cone_code_fingerprint_v1(
            code_projection,
            native_requirements,
            defined_symbols.clone(),
            undefined_partitions,
        )
        .map_err(StrongLinkFinalValidationError::CodeFingerprint)?;
        let (code, expected_cross_cone_link_closure) = cross_cone_code.into_parts();
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
        let cross_cone_link_closure = cross_cone_link_closure
            .validate_against(&expected_cross_cone_link_closure)
            .map_err(StrongLinkFinalValidationError::CrossConeLinkClosure)?;
        Ok(ValidatedCrossConeStrongLinkArtifact {
            graph,
            identities,
            foundations,
            production,
            defined_symbols,
            link_identity_closure,
            cross_cone_link_closure,
            production_manifest,
        })
    }
}

impl ValidatedCrossConeStrongLinkArtifact<'_> {
    pub(crate) const fn identity_graph(&self) -> &ValidatedIdentityGraph {
        &self.identities
    }

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

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.graph.decode_usage()
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

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn production(&self) -> &ValidatedSingleConeStrongProduction {
        &self.production
    }

    pub const fn defined_symbols(&self) -> &CanonicalDefinedLinkSymbolOwnerSetV1 {
        &self.defined_symbols
    }

    pub const fn link_identity_closure(&self) -> &LinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn cross_cone_link_closure(&self) -> &CrossConeLinkClosureSectionV1 {
        &self.cross_cone_link_closure
    }

    pub const fn production_manifest(&self) -> &SingleConeProductionManifestV1 {
        &self.production_manifest
    }
}
