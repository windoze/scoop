//! Compile-view HIR-front decoding for the cross-Cone semantics profile.

use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CoreBootstrapInterfaceValidationError,
    CrossConeHirInterfaceResolutionError, CrossConeHirInterfaceSectionV1,
    DecodedCoreBootstrapInterfaceSectionV1, DecodedCrossConeHirInterfaceSectionV1,
    DecodedHirFoundation, OdrFreeHirFoundation,
};
use scoop_identity::{
    ConeCoordinate, ConeIdentity, IdentityReferenceError, IdentityValidationError,
    ValidatedIdentityGraph,
};
use scoop_lir::{DecodedLirFoundation, DecodedStrongProductionSectionV1, OdrFreeLirFoundation};
use scoop_mir::{DecodedCoreBootstrapBridgeSectionV1, DecodedMirFoundation, OdrFreeMirFoundation};
use scoop_wire::DecodeUsage;

use crate::{
    ArtifactCapabilityProfile, ArtifactFingerprint, CompileSectionDecodeError, ConeKind,
    ConeSourceForm, DependencyRecord, MetadataLocation, SemanticFingerprintRecord,
    ValidatedGraphArtifact,
    compile_decode::validate_foundation_identity_graph_with_authorities,
    compile_sections::{decode_compile_metadata_envelopes, decode_compile_section},
    hir_core_bootstrap_interface_capability, hir_cross_cone_interface_capability,
    hir_identity_foundation_capability, lir_identity_foundation_capability,
    lir_strong_production_capability, mir_core_bootstrap_bridge_capability,
    mir_identity_foundation_capability,
    strong_compile_decode::{
        OdrFreeStrongFoundationSet, StrongProfileFoundationError,
        validate_strong_profile_foundations,
    },
};

/// Canonically decoded identity, legacy production, and general HIR payloads
/// from one exact `cross-cone-semantics-strong/1` Compile view.
///
/// The new MIR/LIR bridge payloads are intentionally not promoted by this
/// state. They remain obligations of the later bridge-validation phase, so
/// this type is neither a complete per-artifact Compile proof nor a semantic
/// closure proof.
#[derive(Debug)]
pub struct DecodedCrossConeHirFrontSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    hir_foundation: DecodedHirFoundation,
    hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    hir_interface: DecodedCrossConeHirInterfaceSectionV1,
    mir_foundation: DecodedMirFoundation,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_foundation: DecodedLirFoundation,
    lir_strong_production: DecodedStrongProductionSectionV1,
}

/// One cross-Cone provider whose HIR/MIR/LIR foundations are structurally
/// valid and ODR-free. General HIR and legacy production payloads remain
/// decoded references and carry no semantic-surface authority yet.
pub struct FoundationValidatedCrossConeHirFrontSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    hir_interface: DecodedCrossConeHirInterfaceSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_strong_production: DecodedStrongProductionSectionV1,
}

/// One cross-Cone provider whose general HIR section contains only typed
/// references resolved by the provider's validated identity authority.
/// Cross-table semantics and dependency routes remain unvalidated.
pub struct ResolvedCrossConeHirFrontSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core_production: DecodedCoreBootstrapInterfaceSectionV1,
    hir_interface: CrossConeHirInterfaceSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_strong_production: DecodedStrongProductionSectionV1,
}

/// One cross-Cone provider whose legacy HIR production surface and general
/// HIR identity references are both validated. General-interface ownership,
/// route, and external-reference semantics remain pending.
pub struct HirProductionValidatedCrossConeHirFrontSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    foundations: OdrFreeStrongFoundationSet,
    hir_core_production: CoreBootstrapInterfaceSectionV1,
    hir_interface: CrossConeHirInterfaceSectionV1,
    mir_core_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_strong_production: DecodedStrongProductionSectionV1,
}

impl<'input> ValidatedGraphArtifact<'input> {
    /// Opens the HIR-facing front of the M23-5 profile without granting any
    /// identity, surface, route, bridge, or session-import authority.
    pub fn decode_cross_cone_hir_front_sections(
        mut self,
    ) -> Result<DecodedCrossConeHirFrontSections<'input>, CrossConeHirFrontSectionDecodeError> {
        let metadata = decode_compile_metadata_envelopes(
            &mut self,
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
        )?;

        let hir_foundation = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Hir,
            hir_identity_foundation_capability(),
        )?;
        let hir_core_production = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Hir,
            hir_core_bootstrap_interface_capability(),
        )?;
        let hir_interface = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Hir,
            hir_cross_cone_interface_capability(),
        )?;
        let mir_foundation = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Mir,
            mir_identity_foundation_capability(),
        )?;
        let mir_core_production = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Mir,
            mir_core_bootstrap_bridge_capability(),
        )?;
        let lir_foundation = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Lir,
            lir_identity_foundation_capability(),
        )?;
        let lir_strong_production = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Lir,
            lir_strong_production_capability(),
        )?;

        metadata.validate_semantic_fingerprints(&mut self)?;

        Ok(DecodedCrossConeHirFrontSections {
            graph: self,
            hir_foundation,
            hir_core_production,
            hir_interface,
            mir_foundation,
            mir_core_production,
            lir_foundation,
            lir_strong_production,
        })
    }
}

impl<'input> DecodedCrossConeHirFrontSections<'input> {
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

    pub const fn target_selection(&self) -> scoop_lir::ValidatedLirTargetSelection {
        self.graph.target_selection()
    }

    pub fn direct_dependencies(&self) -> &[DependencyRecord] {
        self.graph.direct_dependencies()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.graph.decode_usage()
    }

    pub const fn semantic_fingerprints(&self) -> SemanticFingerprintRecord {
        self.graph.envelope.manifest().semantic_fingerprints()
    }

    pub fn dependency_record(&self) -> DependencyRecord {
        let semantic = self.semantic_fingerprints();
        DependencyRecord::from_validated(
            self.coordinate().clone(),
            self.identity(),
            semantic.hir(),
            semantic.mir(),
            semantic.lir(),
        )
    }

    pub const fn hir_foundation_wire(&self) -> &DecodedHirFoundation {
        &self.hir_foundation
    }

    pub const fn hir_core_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_core_production
    }

    pub const fn hir_interface_wire(&self) -> &DecodedCrossConeHirInterfaceSectionV1 {
        &self.hir_interface
    }

    pub const fn mir_foundation_wire(&self) -> &DecodedMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_core_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn lir_foundation_wire(&self) -> &DecodedLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_strong_production
    }

    /// Validates this artifact's complete foundation identity delta against
    /// only the already validated authority of its own dependency closure.
    pub(crate) fn validate_foundation_identities<'authority>(
        &mut self,
        external_authorities: impl IntoIterator<Item = &'authority ValidatedIdentityGraph>,
    ) -> Result<ValidatedIdentityGraph, IdentityValidationError> {
        validate_foundation_identity_graph_with_authorities(
            &mut self.graph,
            &self.hir_foundation,
            &self.mir_foundation,
            &self.lir_foundation,
            external_authorities,
        )
    }

    pub(crate) fn validate_foundation_structure(
        self,
        mut identities: ValidatedIdentityGraph,
    ) -> Result<FoundationValidatedCrossConeHirFrontSections<'input>, StrongProfileFoundationError>
    {
        let Self {
            mut graph,
            hir_foundation,
            hir_core_production,
            hir_interface,
            mir_foundation,
            mir_core_production,
            lir_foundation,
            lir_strong_production,
        } = self;
        let foundations = validate_strong_profile_foundations(
            &mut graph,
            &mut identities,
            hir_foundation,
            mir_foundation,
            lir_foundation,
        )?;
        Ok(FoundationValidatedCrossConeHirFrontSections {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            lir_strong_production,
        })
    }
}

impl<'input> FoundationValidatedCrossConeHirFrontSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub fn declared_identity_count(&self) -> usize {
        self.identities.declared_identity_count()
    }

    pub const fn hir_core_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_core_production
    }

    pub const fn hir_interface_wire(&self) -> &DecodedCrossConeHirInterfaceSectionV1 {
        &self.hir_interface
    }

    pub const fn mir_core_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_strong_production
    }

    pub(crate) fn resolve_hir_interface(
        self,
    ) -> Result<
        ResolvedCrossConeHirFrontSections<'input>,
        CrossConeHirInterfaceResolutionError<IdentityReferenceError>,
    > {
        let Self {
            graph,
            mut identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            lir_strong_production,
        } = self;
        let hir_interface = hir_interface.resolve(&mut identities)?;
        Ok(ResolvedCrossConeHirFrontSections {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            lir_strong_production,
        })
    }
}

impl<'input> ResolvedCrossConeHirFrontSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub fn declared_identity_count(&self) -> usize {
        self.identities.declared_identity_count()
    }

    pub const fn hir_core_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_core_production
    }

    pub const fn hir_interface(&self) -> &CrossConeHirInterfaceSectionV1 {
        &self.hir_interface
    }

    pub const fn mir_core_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_strong_production
    }

    /// Replays the unchanged M23-3 HIR production contract against the same
    /// ODR-free foundation used to resolve the general interface. This grants
    /// the canonical direct-public surface needed by the M23-5 semantic pass.
    pub(crate) fn validate_hir_production(
        self,
    ) -> Result<
        HirProductionValidatedCrossConeHirFrontSections<'input>,
        CoreBootstrapInterfaceValidationError,
    > {
        let Self {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            lir_strong_production,
        } = self;
        let hir_core_production = hir_core_production
            .validate_against_strong_foundation(graph.identity(), &foundations.hir)?;
        Ok(HirProductionValidatedCrossConeHirFrontSections {
            graph,
            identities,
            foundations,
            hir_core_production,
            hir_interface,
            mir_core_production,
            lir_strong_production,
        })
    }
}

impl HirProductionValidatedCrossConeHirFrontSections<'_> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
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

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub fn declared_identity_count(&self) -> usize {
        self.identities.declared_identity_count()
    }

    pub const fn hir_core_production(&self) -> &CoreBootstrapInterfaceSectionV1 {
        &self.hir_core_production
    }

    pub const fn hir_interface(&self) -> &CrossConeHirInterfaceSectionV1 {
        &self.hir_interface
    }

    pub const fn mir_core_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_core_production
    }

    pub const fn lir_strong_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_strong_production
    }
}

pub type CrossConeHirFrontSectionDecodeError = CompileSectionDecodeError;

#[cfg(test)]
pub(crate) mod tests;
