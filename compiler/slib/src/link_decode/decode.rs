use super::*;

impl<'input> ValidatedGraphArtifact<'input> {
    pub fn decode_single_cone_link_sections(
        mut self,
    ) -> Result<DecodedSingleConeLinkSections<'input>, SingleConeLinkSectionDecodeError> {
        let profile = require_strong_profile(&self, ArtifactCapabilityProfile::SINGLE_CONE_STRONG)?;
        let production_manifest = decode_production_manifest(&mut self, profile)?;

        let hir_member_id = metadata_member_id(&self, MetadataLocation::Hir)?;
        let mir_member_id = metadata_member_id(&self, MetadataLocation::Mir)?;
        let lir_member_id = metadata_member_id(&self, MetadataLocation::Lir)?;
        let hir_payload = member_payload(&self, MetadataLocation::Hir, hir_member_id)?;
        let mir_payload = member_payload(&self, MetadataLocation::Mir, mir_member_id)?;
        let lir_payload = member_payload(&self, MetadataLocation::Lir, lir_member_id)?;

        let hir_envelope = decode_metadata_envelope(hir_payload, MetadataLocation::Hir)?;
        let mir_envelope = decode_metadata_envelope(mir_payload, MetadataLocation::Mir)?;
        let lir_envelope = decode_metadata_envelope(lir_payload, MetadataLocation::Lir)?;
        profile
            .validate_link_metadata_inventory(MetadataLocation::Hir, hir_envelope.sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
        profile
            .validate_link_metadata_inventory(MetadataLocation::Mir, mir_envelope.sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;
        profile
            .validate_link_metadata_inventory(MetadataLocation::Lir, lir_envelope.sections())
            .map_err(SingleConeLinkSectionDecodeError::Inventory)?;

        let hir_foundation_capability = hir_identity_foundation_capability();
        let hir_production_capability = hir_core_bootstrap_interface_capability();
        let mir_foundation_capability = mir_identity_foundation_capability();
        let mir_production_capability = mir_core_bootstrap_bridge_capability();
        let lir_foundation_capability = lir_identity_foundation_capability();
        let strong_production_capability = lir_strong_production_capability();
        let link_identity_closure_capability = lir_link_identity_closure_capability();
        let hir_foundation_payload =
            required_metadata_section(&hir_envelope, &hir_foundation_capability)?;
        let hir_production_payload =
            required_metadata_section(&hir_envelope, &hir_production_capability)?;
        let mir_foundation_payload =
            required_metadata_section(&mir_envelope, &mir_foundation_capability)?;
        let mir_production_payload =
            required_metadata_section(&mir_envelope, &mir_production_capability)?;
        let lir_foundation_payload =
            required_metadata_section(&lir_envelope, &lir_foundation_capability)?;
        let strong_payload =
            required_metadata_section(&lir_envelope, &strong_production_capability)?;
        let closure_payload =
            required_metadata_section(&lir_envelope, &link_identity_closure_capability)?;
        let hir_foundation = decode_inner(
            MetadataLocation::Hir,
            hir_foundation_capability,
            hir_foundation_payload,
        )?;
        let hir_production = decode_inner(
            MetadataLocation::Hir,
            hir_production_capability,
            hir_production_payload,
        )?;
        let mir_foundation = decode_inner(
            MetadataLocation::Mir,
            mir_foundation_capability,
            mir_foundation_payload,
        )?;
        let mir_production = decode_inner(
            MetadataLocation::Mir,
            mir_production_capability,
            mir_production_payload,
        )?;
        let lir_foundation = decode_inner(
            MetadataLocation::Lir,
            lir_foundation_capability,
            lir_foundation_payload,
        )?;
        let strong_production = decode_inner(
            MetadataLocation::Lir,
            strong_production_capability,
            strong_payload,
        )?;
        let link_identity_closure = decode_inner(
            MetadataLocation::Lir,
            link_identity_closure_capability,
            closure_payload,
        )?;
        validate_semantic_fingerprints(&mut self, &hir_envelope, &mir_envelope, &lir_envelope)?;

        Ok(DecodedSingleConeLinkSections {
            graph: self,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            strong_production,
            link_identity_closure,
            production_manifest,
        })
    }
}

impl<'input> DecodedSingleConeLinkSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn hir_foundation_wire(&self) -> &DecodedHirFoundation {
        &self.hir_foundation
    }

    pub const fn mir_foundation_wire(&self) -> &DecodedMirFoundation {
        &self.mir_foundation
    }

    pub const fn hir_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    pub const fn lir_foundation_wire(&self) -> &DecodedLirFoundation {
        &self.lir_foundation
    }

    pub const fn strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.strong_production
    }

    pub const fn link_identity_closure_wire(&self) -> &DecodedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    /// Resolves all persistent references in the single-Cone foundation.
    pub fn validate_identities(
        self,
    ) -> Result<IdentityCheckedSingleConeLinkSections<'input>, IdentityValidationError> {
        let Self {
            mut graph,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            strong_production,
            link_identity_closure,
            production_manifest,
        } = self;
        let identities = validate_foundation_identity_graph_with_authorities(
            &mut graph,
            &hir_foundation,
            &mir_foundation,
            &lir_foundation,
            std::iter::empty(),
        )?;
        Ok(IdentityCheckedSingleConeLinkSections {
            graph,
            identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            strong_production,
            link_identity_closure,
            production_manifest,
        })
    }
}

impl<'input> IdentityCheckedSingleConeLinkSections<'input> {
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

    pub const fn strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.strong_production
    }

    pub const fn hir_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    pub const fn link_identity_closure_wire(&self) -> &DecodedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn validate_foundation_structure(
        self,
    ) -> Result<OdrCheckedSingleConeLinkFoundations<'input>, StrongProfileFoundationError> {
        let Self {
            mut graph,
            mut identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            strong_production,
            link_identity_closure,
            production_manifest,
        } = self;
        let foundations = validate_strong_profile_foundations(
            &mut graph,
            &mut identities,
            hir_foundation,
            mir_foundation,
            lir_foundation,
        )?;
        Ok(OdrCheckedSingleConeLinkFoundations {
            graph,
            identities,
            foundations,
            production: DecodedStrongProfileProductionSet {
                hir: hir_production,
                mir: mir_production,
                lir: strong_production,
            },
            link_identity_closure,
            production_manifest,
        })
    }
}

impl<'input> OdrCheckedSingleConeLinkFoundations<'input> {
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

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.foundations.hir
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.foundations.mir
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.foundations.lir
    }

    pub const fn strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.production.lir
    }

    pub const fn hir_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.production.hir
    }

    pub const fn mir_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.production.mir
    }

    pub const fn link_identity_closure_wire(&self) -> &DecodedLinkIdentityClosureSectionV1 {
        &self.link_identity_closure
    }

    pub const fn production_manifest_wire(&self) -> &DecodedSingleConeProductionManifestV1 {
        &self.production_manifest
    }

    pub fn reconstruct_external_bridges(
        &mut self,
    ) -> Result<StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeReconstructionError> {
        let producer = self.graph.identity();
        self.production
            .lir
            .reconstruct_external_bridges(producer, &mut self.identities)
    }

    pub fn validate_production(
        self,
        expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
    ) -> Result<ProductionValidatedSingleConeLinkSections<'input>, StrongProfileProductionError>
    {
        let Self {
            graph,
            mut identities,
            foundations,
            production,
            link_identity_closure,
            production_manifest,
        } = self;
        let production = validate_strong_profile_production(
            &graph,
            &mut identities,
            &foundations,
            production,
            expected_external_bridges,
        )?;
        Ok(ProductionValidatedSingleConeLinkSections {
            graph,
            identities: Rc::new(identities),
            foundations,
            production,
            link_identity_closure,
            production_manifest,
        })
    }
}
