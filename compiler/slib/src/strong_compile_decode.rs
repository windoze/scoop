//! Compile-view section inventory and atomic payload decoding for the
//! single-Cone strong profile.

use std::fmt;

use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CoreBootstrapInterfaceValidationError,
    CoreCallableDefinitionV1, CoreHirCallableCapabilityV1, CoreHirInterfaceBranchV1,
    CoreHirTypeCapabilityV1, CoreShapeSupportSourceProjectionError, CoreTypeDefinitionV1,
    DecodedCoreBootstrapInterfaceSectionV1, DecodedHirFoundation, HirFoundationValidationError,
    HirOutputContractV1, ImportedHirFoundation, OdrFreeHirFoundation, OdrFreeHirFoundationError,
};
use scoop_identity::{
    ConeCoordinate, ConeIdentity, IdentityValidationError, SemanticIdentitySession,
    ValidatedIdentityGraph,
};
use scoop_lir::{
    CoreLirBridgeBranchV1, DecodedLirFoundation, DecodedStrongProductionSectionV1,
    EntryProductionSourceV1, ImportedLirFoundation, LirFoundationValidationError,
    OdrFreeLirFoundation, OdrFreeLirFoundationError, StrongExternalLirBridgeReconstructionError,
    StrongExternalLirBridgeSurfaceV1, StrongProductionSectionV1,
    StrongProductionSectionValidationError,
};
use scoop_mir::{
    CoreBootstrapBridgeSectionV1, CoreMirBridgeBranchV1, DecodedCoreBootstrapBridgeSectionV1,
    DecodedMirFoundation, EntryMirBridgeBranchV1, ImportedMirFoundation,
    MirFoundationValidationError, MirProductionValidationError, OdrFreeMirFoundation,
    OdrFreeMirFoundationError,
};
use scoop_wire::DecodeUsage;

use crate::{
    ArtifactCapabilityProfile, ArtifactFingerprint, CompileSectionDecodeError, ConeKind,
    MetadataLocation, NativeBoundaryCompileError, ValidatedGraphArtifact,
    compile_decode::{
        CompileCommitError, NativeBoundaryFoundationView, SingleConeStrongProfile,
        ValidatedCompileArtifact, commit_identity_graph, validate_foundation_identity_graph,
        validate_native_boundary_parts,
    },
    compile_sections::{decode_compile_metadata_envelopes, decode_compile_section},
    hir_core_bootstrap_interface_capability, hir_identity_foundation_capability,
    lir_identity_foundation_capability, lir_strong_production_capability,
    mir_core_bootstrap_bridge_capability, mir_identity_foundation_capability,
};

/// Every Compile payload required by one strong-profile graph artifact,
/// decoded as a single transaction. Persistent identities and cross-section
/// structure have not yet been validated or committed to a semantic session.
#[derive(Debug)]
pub struct DecodedSingleConeCompileSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    hir_foundation: DecodedHirFoundation,
    hir_production: DecodedCoreBootstrapInterfaceSectionV1,
    mir_foundation: DecodedMirFoundation,
    mir_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_foundation: DecodedLirFoundation,
    lir_production: DecodedStrongProductionSectionV1,
}

/// Strong-profile Compile sections whose complete HIR-to-LIR identity graph
/// passed one transaction. Foundation structure, ODR policy, and production
/// relations remain unproven.
pub struct IdentityCheckedSingleConeCompileSections<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    hir_foundation: DecodedHirFoundation,
    hir_production: DecodedCoreBootstrapInterfaceSectionV1,
    mir_foundation: DecodedMirFoundation,
    mir_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_foundation: DecodedLirFoundation,
    lir_production: DecodedStrongProductionSectionV1,
}

/// Strong-profile foundations whose local structure and `RejectAll` ODR
/// policy both passed. Production sections remain decoded references and are
/// validated only by the following cross-section phase.
pub struct OdrCheckedSingleConeCompileFoundations<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    hir_foundation: OdrFreeHirFoundation,
    hir_production: DecodedCoreBootstrapInterfaceSectionV1,
    mir_foundation: OdrFreeMirFoundation,
    mir_production: DecodedCoreBootstrapBridgeSectionV1,
    lir_foundation: OdrFreeLirFoundation,
    lir_production: DecodedStrongProductionSectionV1,
}

/// ODR-free foundations whose HIR and MIR production sections are locally
/// validated. Cross-layer HIR/MIR authority relations and the LIR production
/// section remain separate obligations.
pub struct LocallyValidatedSingleConeCompileProduction<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    hir_foundation: OdrFreeHirFoundation,
    hir_production: CoreBootstrapInterfaceSectionV1,
    mir_foundation: OdrFreeMirFoundation,
    mir_production: CoreBootstrapBridgeSectionV1,
    lir_foundation: OdrFreeLirFoundation,
    lir_production: DecodedStrongProductionSectionV1,
}

/// HIR and MIR production sections proven to describe the same manifest
/// output and, for core, the same complete public and compiler-protocol
/// callable bridge surface.
/// LIR production validation remains the next proof obligation.
pub struct ValidatedSingleConeCompileSemanticFront<'input> {
    local: LocallyValidatedSingleConeCompileProduction<'input>,
}

/// Every strong Compile payload structurally proven against the same ODR-free
/// HIR, MIR, and LIR front. External bridge equality is checked against a
/// typed selection projection; trusted-core authority remains a later proof
/// obligation. Entry and core shape sources are not caller-supplied.
pub struct StructurallyValidatedSingleConeCompileProduction<'input> {
    graph: ValidatedGraphArtifact<'input>,
    identities: ValidatedIdentityGraph,
    hir_foundation: OdrFreeHirFoundation,
    hir_production: CoreBootstrapInterfaceSectionV1,
    mir_foundation: OdrFreeMirFoundation,
    mir_production: CoreBootstrapBridgeSectionV1,
    lir_foundation: OdrFreeLirFoundation,
    lir_production: StrongProductionSectionV1,
}

/// Structurally valid strong Compile production whose source and target
/// native-boundary closures were replayed against the same canonical
/// foundations.
pub struct NativeBoundaryValidatedSingleConeCompileProduction<'input> {
    structural: StructurallyValidatedSingleConeCompileProduction<'input>,
}

pub(crate) struct OdrFreeStrongFoundationSet {
    pub(crate) hir: OdrFreeHirFoundation,
    pub(crate) mir: OdrFreeMirFoundation,
    pub(crate) lir: OdrFreeLirFoundation,
}

pub(crate) struct DecodedStrongProfileProductionSet {
    pub(crate) hir: DecodedCoreBootstrapInterfaceSectionV1,
    pub(crate) mir: DecodedCoreBootstrapBridgeSectionV1,
    pub(crate) lir: DecodedStrongProductionSectionV1,
}

/// The validated HIR, MIR, and LIR production surfaces retained by a
/// structurally valid `SingleConeStrongProfile` proof.
pub struct ValidatedSingleConeStrongProduction {
    hir: CoreBootstrapInterfaceSectionV1,
    mir: CoreBootstrapBridgeSectionV1,
    lir: StrongProductionSectionV1,
}

/// Validate and atomically import the complete Compile view of one
/// `SingleConeStrongProfile` artifact.
///
/// This is the only public whole-artifact Compile entry. The intermediate
/// states remain available for focused verifier tests, but callers cannot
/// obtain the final proof without replaying every required phase.
pub fn validate_single_cone_strong_compile_artifact<'input>(
    graph: ValidatedGraphArtifact<'input>,
    expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
    session: &mut SemanticIdentitySession,
) -> Result<
    ValidatedCompileArtifact<'input, SingleConeStrongProfile>,
    StrongCompileArtifactValidationError,
> {
    graph
        .decode_single_cone_compile_sections()
        .map_err(|error| StrongCompileArtifactValidationError::Decode(Box::new(error)))?
        .validate_identities()
        .map_err(|error| StrongCompileArtifactValidationError::Identities(Box::new(error)))?
        .validate_foundation_structure()
        .map_err(|error| StrongCompileArtifactValidationError::Foundations(Box::new(error)))?
        .validate_local_production()
        .map_err(|error| StrongCompileArtifactValidationError::LocalProduction(Box::new(error)))?
        .validate_cross_layer()
        .map_err(|error| StrongCompileArtifactValidationError::Relations(Box::new(error)))?
        .validate_lir_production(expected_external_bridges)
        .map_err(|error| StrongCompileArtifactValidationError::LirProduction(Box::new(error)))?
        .validate_native_boundary()
        .map_err(|error| StrongCompileArtifactValidationError::NativeBoundary(Box::new(error)))?
        .commit(session)
        .map_err(|error| StrongCompileArtifactValidationError::Commit(Box::new(error)))
}

/// Validates a previously published artifact without compiler-side IR by
/// reconstructing its typed external bridge surface from the artifact's
/// already validated identity graph before replaying the complete Compile
/// view.
pub fn validate_self_describing_single_cone_strong_compile_artifact<'input>(
    graph: ValidatedGraphArtifact<'input>,
    session: &mut SemanticIdentitySession,
) -> Result<
    ValidatedCompileArtifact<'input, SingleConeStrongProfile>,
    StrongCompileArtifactValidationError,
> {
    let mut front = graph
        .decode_single_cone_compile_sections()
        .map_err(|error| StrongCompileArtifactValidationError::Decode(Box::new(error)))?
        .validate_identities()
        .map_err(|error| StrongCompileArtifactValidationError::Identities(Box::new(error)))?
        .validate_foundation_structure()
        .map_err(|error| StrongCompileArtifactValidationError::Foundations(Box::new(error)))?
        .validate_local_production()
        .map_err(|error| StrongCompileArtifactValidationError::LocalProduction(Box::new(error)))?
        .validate_cross_layer()
        .map_err(|error| StrongCompileArtifactValidationError::Relations(Box::new(error)))?;
    let external_bridges = front
        .reconstruct_external_bridges()
        .map_err(|error| StrongCompileArtifactValidationError::ExternalBridges(Box::new(error)))?;
    front
        .validate_lir_production(&external_bridges)
        .map_err(|error| StrongCompileArtifactValidationError::LirProduction(Box::new(error)))?
        .validate_native_boundary()
        .map_err(|error| StrongCompileArtifactValidationError::NativeBoundary(Box::new(error)))?
        .commit(session)
        .map_err(|error| StrongCompileArtifactValidationError::Commit(Box::new(error)))
}

impl ValidatedSingleConeStrongProduction {
    pub const fn hir(&self) -> &CoreBootstrapInterfaceSectionV1 {
        &self.hir
    }

    pub const fn mir(&self) -> &CoreBootstrapBridgeSectionV1 {
        &self.mir
    }

    pub const fn lir(&self) -> &StrongProductionSectionV1 {
        &self.lir
    }
}

impl<'input> ValidatedGraphArtifact<'input> {
    pub fn decode_single_cone_compile_sections(
        mut self,
    ) -> Result<DecodedSingleConeCompileSections<'input>, SingleConeCompileSectionDecodeError> {
        let metadata = decode_compile_metadata_envelopes(
            &mut self,
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        )?;

        let hir_foundation_capability = hir_identity_foundation_capability();
        let hir_production_capability = hir_core_bootstrap_interface_capability();
        let mir_foundation_capability = mir_identity_foundation_capability();
        let mir_production_capability = mir_core_bootstrap_bridge_capability();
        let lir_foundation_capability = lir_identity_foundation_capability();
        let lir_production_capability = lir_strong_production_capability();
        let hir_foundation = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Hir,
            hir_foundation_capability,
        )?;
        let hir_production = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Hir,
            hir_production_capability,
        )?;
        let mir_foundation = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Mir,
            mir_foundation_capability,
        )?;
        let mir_production = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Mir,
            mir_production_capability,
        )?;
        let lir_foundation = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Lir,
            lir_foundation_capability,
        )?;
        let lir_production = decode_compile_section(
            &mut self,
            &metadata,
            MetadataLocation::Lir,
            lir_production_capability,
        )?;

        metadata.validate_semantic_fingerprints(&mut self)?;

        Ok(DecodedSingleConeCompileSections {
            graph: self,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        })
    }
}

impl<'input> DecodedSingleConeCompileSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.graph.decode_usage()
    }

    pub const fn hir_foundation_wire(&self) -> &DecodedHirFoundation {
        &self.hir_foundation
    }

    pub const fn hir_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_foundation_wire(&self) -> &DecodedMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    pub const fn lir_foundation_wire(&self) -> &DecodedLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_production
    }

    /// Registers and resolves every foundation identity before exposing any
    /// trusted persistent id to production-section validation.
    pub fn validate_identities(
        self,
    ) -> Result<IdentityCheckedSingleConeCompileSections<'input>, IdentityValidationError> {
        let Self {
            mut graph,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        } = self;
        let identities = validate_foundation_identity_graph(
            &mut graph,
            &hir_foundation,
            &mir_foundation,
            &lir_foundation,
        )?;

        Ok(IdentityCheckedSingleConeCompileSections {
            graph,
            identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        })
    }
}

impl<'input> IdentityCheckedSingleConeCompileSections<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.graph.decode_usage()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub fn declared_identity_count(&self) -> usize {
        self.identities.declared_identity_count()
    }

    pub const fn hir_foundation_wire(&self) -> &DecodedHirFoundation {
        &self.hir_foundation
    }

    pub const fn hir_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_foundation_wire(&self) -> &DecodedMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    pub const fn lir_foundation_wire(&self) -> &DecodedLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_production
    }

    /// Validates all three foundation structures before replaying the strong
    /// profile's ODR rejection over each canonical layer.
    pub fn validate_foundation_structure(
        self,
    ) -> Result<OdrCheckedSingleConeCompileFoundations<'input>, StrongProfileFoundationError> {
        let Self {
            mut graph,
            mut identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        } = self;
        let foundations = validate_strong_profile_foundations(
            &mut graph,
            &mut identities,
            hir_foundation,
            mir_foundation,
            lir_foundation,
        )?;

        Ok(OdrCheckedSingleConeCompileFoundations {
            graph,
            identities,
            hir_foundation: foundations.hir,
            hir_production,
            mir_foundation: foundations.mir,
            mir_production,
            lir_foundation: foundations.lir,
            lir_production,
        })
    }
}

impl<'input> OdrCheckedSingleConeCompileFoundations<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.graph.decode_usage()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub fn declared_identity_count(&self) -> usize {
        self.identities.declared_identity_count()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.hir_foundation
    }

    pub const fn hir_production_wire(&self) -> &DecodedCoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_production_wire(&self) -> &DecodedCoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_production
    }

    pub fn validate_local_production(
        self,
    ) -> Result<
        LocallyValidatedSingleConeCompileProduction<'input>,
        StrongProfileLocalProductionError,
    > {
        let Self {
            graph,
            mut identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        } = self;
        let artifact = graph.identity();
        let production = validate_strong_profile_local_production(
            artifact,
            &mut identities,
            &hir_foundation,
            hir_production,
            &mir_foundation,
            mir_production,
        )?;
        Ok(LocallyValidatedSingleConeCompileProduction {
            graph,
            identities,
            hir_foundation,
            hir_production: production.hir,
            mir_foundation,
            mir_production: production.mir,
            lir_foundation,
            lir_production,
        })
    }
}

impl<'input> LocallyValidatedSingleConeCompileProduction<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.graph.decode_usage()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.hir_foundation
    }

    pub const fn hir_production(&self) -> &CoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_production(&self) -> &CoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        &self.lir_production
    }

    pub fn validate_cross_layer(
        self,
    ) -> Result<ValidatedSingleConeCompileSemanticFront<'input>, StrongProfileRelationError> {
        validate_strong_profile_relations(
            self.graph.kind(),
            &self.hir_production,
            &self.mir_production,
        )?;
        Ok(ValidatedSingleConeCompileSemanticFront { local: self })
    }
}

impl<'input> ValidatedSingleConeCompileSemanticFront<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.local.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.local.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.local.artifact_fingerprint()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.local.decode_usage()
    }

    pub fn identity_count(&self) -> usize {
        self.local.identity_count()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        self.local.hir_foundation()
    }

    pub const fn hir_production(&self) -> &CoreBootstrapInterfaceSectionV1 {
        self.local.hir_production()
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        self.local.mir_foundation()
    }

    pub const fn mir_production(&self) -> &CoreBootstrapBridgeSectionV1 {
        self.local.mir_production()
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        self.local.lir_foundation()
    }

    pub const fn lir_production_wire(&self) -> &DecodedStrongProductionSectionV1 {
        self.local.lir_production_wire()
    }

    pub fn reconstruct_external_bridges(
        &mut self,
    ) -> Result<StrongExternalLirBridgeSurfaceV1, StrongExternalLirBridgeReconstructionError> {
        let producer = self.local.identity();
        self.local
            .lir_production
            .reconstruct_external_bridges(producer, &mut self.local.identities)
    }

    /// Closes the LIR production proof without accepting independently
    /// supplied entry or shape-source lists.
    pub fn validate_lir_production(
        self,
        expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
    ) -> Result<
        StructurallyValidatedSingleConeCompileProduction<'input>,
        StrongProfileLirProductionError,
    > {
        let LocallyValidatedSingleConeCompileProduction {
            graph,
            mut identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        } = self.local;
        let lir_production = validate_strong_profile_lir_production(
            &graph,
            &mut identities,
            StrongProfileSemanticFront {
                hir_foundation: &hir_foundation,
                hir_production: &hir_production,
                mir_production: &mir_production,
                lir_foundation: &lir_foundation,
            },
            lir_production,
            expected_external_bridges,
        )?;
        Ok(StructurallyValidatedSingleConeCompileProduction {
            graph,
            identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        })
    }
}

impl<'input> StructurallyValidatedSingleConeCompileProduction<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.graph.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.graph.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.graph.artifact_fingerprint()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.graph.decode_usage()
    }

    pub fn identity_count(&self) -> usize {
        self.identities.identity_count()
    }

    pub const fn hir_foundation(&self) -> &OdrFreeHirFoundation {
        &self.hir_foundation
    }

    pub const fn hir_production(&self) -> &CoreBootstrapInterfaceSectionV1 {
        &self.hir_production
    }

    pub const fn mir_foundation(&self) -> &OdrFreeMirFoundation {
        &self.mir_foundation
    }

    pub const fn mir_production(&self) -> &CoreBootstrapBridgeSectionV1 {
        &self.mir_production
    }

    pub const fn lir_foundation(&self) -> &OdrFreeLirFoundation {
        &self.lir_foundation
    }

    pub const fn lir_production(&self) -> &StrongProductionSectionV1 {
        &self.lir_production
    }

    pub fn validate_native_boundary(
        mut self,
    ) -> Result<
        NativeBoundaryValidatedSingleConeCompileProduction<'input>,
        NativeBoundaryCompileError,
    > {
        let view = NativeBoundaryFoundationView::from_odr_free(
            &self.hir_foundation,
            &self.mir_foundation,
            &self.lir_foundation,
        );
        validate_native_boundary_parts(&mut self.graph, &self.identities, &view)?;
        Ok(NativeBoundaryValidatedSingleConeCompileProduction { structural: self })
    }
}

impl<'input> NativeBoundaryValidatedSingleConeCompileProduction<'input> {
    pub const fn coordinate(&self) -> &ConeCoordinate {
        self.structural.coordinate()
    }

    pub const fn identity(&self) -> ConeIdentity {
        self.structural.identity()
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.structural.artifact_fingerprint()
    }

    pub const fn decode_usage(&self) -> DecodeUsage {
        self.structural.decode_usage()
    }

    pub const fn structural(&self) -> &StructurallyValidatedSingleConeCompileProduction<'_> {
        &self.structural
    }

    /// Atomically imports the strong profile's ODR-free foundations and
    /// retains all three validated production surfaces in the final proof.
    pub fn commit(
        self,
        session: &mut SemanticIdentitySession,
    ) -> Result<ValidatedCompileArtifact<'input, SingleConeStrongProfile>, CompileCommitError> {
        let StructurallyValidatedSingleConeCompileProduction {
            mut graph,
            identities,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            lir_production,
        } = self.structural;
        let imported = commit_identity_graph(&mut graph, &identities, session)?;
        let (hir_identities, mir_identities, lir_identities) = imported.into_parts();
        let production = ValidatedSingleConeStrongProduction {
            hir: hir_production,
            mir: mir_production,
            lir: lir_production,
        };
        Ok(ValidatedCompileArtifact::from_parts(
            graph,
            ImportedHirFoundation::from_odr_free(hir_foundation, hir_identities),
            ImportedMirFoundation::from_odr_free(mir_foundation, mir_identities),
            ImportedLirFoundation::from_odr_free(lir_foundation, lir_identities),
            production,
        ))
    }
}

struct StrongProfileLocalProductionSet {
    hir: CoreBootstrapInterfaceSectionV1,
    mir: CoreBootstrapBridgeSectionV1,
}

struct StrongProfileSemanticFront<'a> {
    hir_foundation: &'a OdrFreeHirFoundation,
    hir_production: &'a CoreBootstrapInterfaceSectionV1,
    mir_production: &'a CoreBootstrapBridgeSectionV1,
    lir_foundation: &'a OdrFreeLirFoundation,
}

pub(crate) fn validate_strong_profile_production(
    graph: &ValidatedGraphArtifact<'_>,
    identities: &mut ValidatedIdentityGraph,
    foundations: &OdrFreeStrongFoundationSet,
    production: DecodedStrongProfileProductionSet,
    expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
) -> Result<ValidatedSingleConeStrongProduction, StrongProfileProductionError> {
    let local = validate_strong_profile_local_production(
        graph.identity(),
        identities,
        &foundations.hir,
        production.hir,
        &foundations.mir,
        production.mir,
    )
    .map_err(StrongProfileProductionError::Local)?;
    validate_strong_profile_relations(graph.kind(), &local.hir, &local.mir)
        .map_err(StrongProfileProductionError::Relation)?;
    let lir = validate_strong_profile_lir_production(
        graph,
        identities,
        StrongProfileSemanticFront {
            hir_foundation: &foundations.hir,
            hir_production: &local.hir,
            mir_production: &local.mir,
            lir_foundation: &foundations.lir,
        },
        production.lir,
        expected_external_bridges,
    )
    .map_err(StrongProfileProductionError::Lir)?;
    Ok(ValidatedSingleConeStrongProduction {
        hir: local.hir,
        mir: local.mir,
        lir,
    })
}

fn validate_strong_profile_local_production(
    artifact: ConeIdentity,
    identities: &mut ValidatedIdentityGraph,
    hir_foundation: &OdrFreeHirFoundation,
    hir: DecodedCoreBootstrapInterfaceSectionV1,
    mir_foundation: &OdrFreeMirFoundation,
    mir: DecodedCoreBootstrapBridgeSectionV1,
) -> Result<StrongProfileLocalProductionSet, StrongProfileLocalProductionError> {
    let hir = hir
        .validate_against_strong_foundation(artifact, hir_foundation)
        .map_err(StrongProfileLocalProductionError::Hir)?;
    let mir = mir
        .validate_against_strong_foundation(artifact, identities, mir_foundation)
        .map_err(StrongProfileLocalProductionError::Mir)?;
    Ok(StrongProfileLocalProductionSet { hir, mir })
}

fn validate_strong_profile_relations(
    kind: ConeKind,
    hir: &CoreBootstrapInterfaceSectionV1,
    mir: &CoreBootstrapBridgeSectionV1,
) -> Result<(), StrongProfileRelationError> {
    validate_output_relation(kind, hir.output_contract(), mir.entry_bridge())?;
    validate_core_relation(
        hir.core_interface(),
        mir.core_bridge(),
        mir.strong_callable_bridges(),
    )
}

fn validate_strong_profile_lir_production(
    graph: &ValidatedGraphArtifact<'_>,
    identities: &mut ValidatedIdentityGraph,
    front: StrongProfileSemanticFront<'_>,
    lir: DecodedStrongProductionSectionV1,
    expected_external_bridges: &StrongExternalLirBridgeSurfaceV1,
) -> Result<StrongProductionSectionV1, StrongProfileLirProductionError> {
    let entry_source = match front.mir_production.entry_bridge() {
        EntryMirBridgeBranchV1::Library => EntryProductionSourceV1::Library,
        EntryMirBridgeBranchV1::Executable(bridge) => {
            EntryProductionSourceV1::executable(bridge.source().clone())
        }
    };
    let core_shape_sources = match front.hir_production.core_interface() {
        CoreHirInterfaceBranchV1::NotCore => Vec::new(),
        CoreHirInterfaceBranchV1::Core(interface) => interface
            .param_free_shape_support_sources(front.hir_foundation)
            .map_err(StrongProfileLirProductionError::ShapeSources)?,
    };
    let lir = lir
        .validate(
            graph.coordinate().clone(),
            graph.target_selection().target(),
            front.lir_foundation,
            expected_external_bridges,
            entry_source,
            &core_shape_sources,
            identities,
        )
        .map_err(StrongProfileLirProductionError::Production)?;
    validate_core_lir_relation(
        front.mir_production.core_bridge(),
        front.mir_production.strong_callable_bridges(),
        lir.core_lir_bridge(),
    )
    .map_err(StrongProfileLirProductionError::CoreRelation)?;
    Ok(lir)
}

fn validate_core_lir_relation(
    mir: &CoreMirBridgeBranchV1,
    strong: &scoop_mir::StrongCallableBridgeSurfaceV1,
    lir: &CoreLirBridgeBranchV1,
) -> Result<(), StrongProfileCoreLirRelationError> {
    let (CoreMirBridgeBranchV1::Core(mir), CoreLirBridgeBranchV1::Core(lir)) = (mir, lir) else {
        return match (mir, lir) {
            (CoreMirBridgeBranchV1::NotCore, CoreLirBridgeBranchV1::NotCore) => Ok(()),
            _ => Err(StrongProfileCoreLirRelationError::BranchMismatch),
        };
    };
    if mir.callable_targets().len() != lir.callables().len() {
        return Err(StrongProfileCoreLirRelationError::Coverage {
            expected: mir.callable_targets().len(),
            actual: lir.callables().len(),
        });
    }
    for (index, (mir, lir)) in mir
        .callable_targets()
        .iter()
        .zip(lir.callables())
        .enumerate()
    {
        let implementation = mir.implementation();
        let expected_target = match implementation {
            scoop_identity::CallableOwner::Function(id) => {
                scoop_identity::StrongCallableDefinitionOwner::Function(id)
            }
            scoop_identity::CallableOwner::Constructor(id) => {
                scoop_identity::StrongCallableDefinitionOwner::Constructor(id)
            }
            scoop_identity::CallableOwner::Accessor(id) => {
                scoop_identity::StrongCallableDefinitionOwner::PropertyAccessor(id)
            }
            scoop_identity::CallableOwner::Generated(id) => {
                scoop_identity::StrongCallableDefinitionOwner::GeneratedCallable(id)
            }
            scoop_identity::CallableOwner::GenericTemplate(_)
            | scoop_identity::CallableOwner::Application(_) => {
                return Err(StrongProfileCoreLirRelationError::InvalidStrongOwner { index });
            }
        };
        if lir.binding() != mir.binding() || lir.target() != expected_target {
            return Err(StrongProfileCoreLirRelationError::CallableMismatch { index });
        }
        let Some(exact) = strong
            .bridges()
            .iter()
            .find(|bridge| bridge.implementation() == implementation)
        else {
            return Err(StrongProfileCoreLirRelationError::MissingExactSignature { index });
        };
        if lir.abi_signature().signature() != exact.signature() {
            return Err(StrongProfileCoreLirRelationError::ExactSignatureMismatch { index });
        }
    }
    let mir_cycle = mir.initialization_cycle_thrower();
    let cycle_implementation = mir_cycle.implementation();
    let cycle_target = match cycle_implementation {
        scoop_identity::CallableOwner::Function(id) => {
            scoop_identity::StrongCallableDefinitionOwner::Function(id)
        }
        _ => return Err(StrongProfileCoreLirRelationError::InvalidInitializationCycleOwner),
    };
    let lir_cycle = lir.initialization_cycle_thrower();
    if lir_cycle.target() != cycle_target {
        return Err(StrongProfileCoreLirRelationError::InitializationCycleMismatch);
    }
    let cycle_exact = strong
        .bridges()
        .iter()
        .find(|bridge| bridge.implementation() == cycle_implementation)
        .ok_or(StrongProfileCoreLirRelationError::MissingInitializationCycleSignature)?;
    if lir_cycle.abi_signature().signature() != cycle_exact.signature() {
        return Err(StrongProfileCoreLirRelationError::InitializationCycleSignatureMismatch);
    }
    Ok(())
}

fn validate_output_relation(
    kind: ConeKind,
    hir: &HirOutputContractV1,
    mir: &EntryMirBridgeBranchV1,
) -> Result<(), StrongProfileRelationError> {
    match (kind, hir, mir) {
        (ConeKind::Library, HirOutputContractV1::Library, EntryMirBridgeBranchV1::Library) => {
            Ok(())
        }
        (
            ConeKind::Executable,
            HirOutputContractV1::Executable(hir),
            EntryMirBridgeBranchV1::Executable(mir),
        ) if hir.as_ref() == mir.source() => Ok(()),
        (ConeKind::Library, _, _) | (ConeKind::Executable, _, _) => {
            Err(StrongProfileRelationError::OutputMismatch)
        }
    }
}

fn validate_core_relation(
    hir: &CoreHirInterfaceBranchV1,
    mir: &CoreMirBridgeBranchV1,
    strong: &scoop_mir::StrongCallableBridgeSurfaceV1,
) -> Result<(), StrongProfileRelationError> {
    let (CoreHirInterfaceBranchV1::Core(hir), CoreMirBridgeBranchV1::Core(mir)) = (hir, mir) else {
        return match (hir, mir) {
            (CoreHirInterfaceBranchV1::NotCore, CoreMirBridgeBranchV1::NotCore) => Ok(()),
            _ => Err(StrongProfileRelationError::CoreBranchMismatch),
        };
    };

    let mut expected = Vec::new();
    for (index, target) in hir.callable_targets().targets().iter().enumerate() {
        let CoreHirCallableCapabilityV1::ParamFreeCandidate(signature) = target.capability() else {
            continue;
        };
        let CoreCallableDefinitionV1::Function(definition) = target.definition() else {
            return Err(
                StrongProfileRelationError::InvalidCoreCallableCandidateDefinition { index },
            );
        };
        let implementation = scoop_identity::CallableOwner::Function(definition);
        let Some(strong) = strong
            .bridges()
            .iter()
            .find(|strong| strong.implementation() == implementation)
        else {
            continue;
        };
        if strong.signature() != signature {
            return Err(StrongProfileRelationError::CoreCallableSignatureMismatch { index });
        }
        expected.push((target, implementation));
    }

    if expected.len() != mir.callable_targets().len() {
        return Err(StrongProfileRelationError::CoreCallableCoverage {
            expected: expected.len(),
            actual: mir.callable_targets().len(),
        });
    }
    for (index, ((hir, implementation), mir)) in
        expected.iter().zip(mir.callable_targets()).enumerate()
    {
        let CoreCallableDefinitionV1::Function(definition) = hir.definition() else {
            return Err(
                StrongProfileRelationError::InvalidCoreCallableCandidateDefinition { index },
            );
        };
        if hir.binding() != mir.binding()
            || definition != mir.definition()
            || mir.implementation() != *implementation
        {
            return Err(StrongProfileRelationError::CoreCallableMismatch { index });
        }
    }

    let mut expected_shape_roots = hir
        .type_targets()
        .targets()
        .iter()
        .filter_map(|target| {
            let (
                CoreTypeDefinitionV1::Type(source),
                CoreHirTypeCapabilityV1::ParamFreeStrong(exact),
            ) = (target.definition(), target.capability())
            else {
                return None;
            };
            Some((source, exact))
        })
        .collect::<Vec<_>>();
    expected_shape_roots.sort_unstable_by_key(|(source, _)| *source);
    if expected_shape_roots.len() != mir.shape_support_roots().len() {
        return Err(StrongProfileRelationError::CoreShapeRootCoverage {
            expected: expected_shape_roots.len(),
            actual: mir.shape_support_roots().len(),
        });
    }
    for (index, ((expected_source, expected_exact), actual)) in expected_shape_roots
        .iter()
        .zip(mir.shape_support_roots())
        .enumerate()
    {
        if actual.source() != *expected_source || actual.exact() != *expected_exact {
            return Err(StrongProfileRelationError::CoreShapeRootMismatch { index });
        }
    }
    let cycle = hir.compiler_protocols().initialization_cycle_thrower();
    let scoop_hir::CoreProtocolCallableDefinitionV1::Function(cycle_definition) =
        cycle.definition()
    else {
        return Err(StrongProfileRelationError::InvalidInitializationCycleDefinition);
    };
    let actual_cycle = mir.initialization_cycle_thrower();
    let expected_implementation = scoop_identity::CallableOwner::Function(cycle_definition);
    if actual_cycle.definition() != cycle_definition
        || actual_cycle.implementation() != expected_implementation
    {
        return Err(StrongProfileRelationError::InitializationCycleMismatch);
    }
    let expected_signature = scoop_identity::ExactCallableSignature::new(
        scoop_identity::Effect::Ordinary,
        None,
        vec![hir.string_capability().exact_type()],
        scoop_mir::core_unit_exact_type(),
    );
    let actual_signature = strong
        .bridges()
        .iter()
        .find(|bridge| bridge.implementation() == expected_implementation)
        .ok_or(StrongProfileRelationError::MissingInitializationCycleSignature)?;
    if actual_signature.signature() != &expected_signature {
        return Err(StrongProfileRelationError::InitializationCycleSignatureMismatch);
    }
    Ok(())
}

pub(crate) fn validate_strong_profile_foundations(
    graph: &mut ValidatedGraphArtifact<'_>,
    identities: &mut ValidatedIdentityGraph,
    hir: DecodedHirFoundation,
    mir: DecodedMirFoundation,
    lir: DecodedLirFoundation,
) -> Result<OdrFreeStrongFoundationSet, StrongProfileFoundationError> {
    let coordinate = graph.coordinate().clone();
    let producer = graph.identity();
    let meter = graph.envelope.meter_mut();
    let hir = hir
        .validate(&coordinate, identities, meter)
        .map_err(StrongProfileFoundationError::HirStructure)?;
    let mir = mir
        .validate(identities, meter)
        .map_err(StrongProfileFoundationError::MirStructure)?;
    let lir = lir
        .validate(producer, identities, meter)
        .map_err(StrongProfileFoundationError::LirStructure)?;

    Ok(OdrFreeStrongFoundationSet {
        hir: OdrFreeHirFoundation::from_validated(hir)
            .map_err(StrongProfileFoundationError::HirOdr)?,
        mir: OdrFreeMirFoundation::from_validated(mir)
            .map_err(StrongProfileFoundationError::MirOdr)?,
        lir: OdrFreeLirFoundation::from_validated(lir)
            .map_err(StrongProfileFoundationError::LirOdr)?,
    })
}

#[derive(Debug)]
pub enum StrongProfileFoundationError {
    HirStructure(HirFoundationValidationError),
    MirStructure(MirFoundationValidationError),
    LirStructure(LirFoundationValidationError),
    HirOdr(OdrFreeHirFoundationError),
    MirOdr(OdrFreeMirFoundationError),
    LirOdr(OdrFreeLirFoundationError),
}

impl fmt::Display for StrongProfileFoundationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong-profile foundation: {self:?}")
    }
}

impl std::error::Error for StrongProfileFoundationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::HirStructure(error) => error,
            Self::MirStructure(error) => error,
            Self::LirStructure(error) => error,
            Self::HirOdr(error) => error,
            Self::MirOdr(error) => error,
            Self::LirOdr(error) => error,
        })
    }
}

#[derive(Debug)]
pub enum StrongProfileLocalProductionError {
    Hir(CoreBootstrapInterfaceValidationError),
    Mir(MirProductionValidationError),
}

#[derive(Debug)]
pub enum StrongProfileProductionError {
    Local(StrongProfileLocalProductionError),
    Relation(StrongProfileRelationError),
    Lir(StrongProfileLirProductionError),
}

impl fmt::Display for StrongProfileProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong-profile production: {self:?}")
    }
}

impl std::error::Error for StrongProfileProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Local(error) => error,
            Self::Relation(error) => error,
            Self::Lir(error) => error,
        })
    }
}

impl fmt::Display for StrongProfileLocalProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong-profile local production: {self:?}"
        )
    }
}

impl std::error::Error for StrongProfileLocalProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Hir(error) => error,
            Self::Mir(error) => error,
        })
    }
}

#[derive(Debug)]
pub enum StrongProfileLirProductionError {
    ShapeSources(CoreShapeSupportSourceProjectionError),
    Production(StrongProductionSectionValidationError),
    CoreRelation(StrongProfileCoreLirRelationError),
}

impl fmt::Display for StrongProfileLirProductionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strong-profile LIR production: {self:?}")
    }
}

impl std::error::Error for StrongProfileLirProductionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::ShapeSources(error) => error,
            Self::Production(error) => error,
            Self::CoreRelation(error) => error,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongProfileCoreLirRelationError {
    BranchMismatch,
    Coverage { expected: usize, actual: usize },
    InvalidStrongOwner { index: usize },
    CallableMismatch { index: usize },
    MissingExactSignature { index: usize },
    ExactSignatureMismatch { index: usize },
    InvalidInitializationCycleOwner,
    InitializationCycleMismatch,
    MissingInitializationCycleSignature,
    InitializationCycleSignatureMismatch,
}

impl fmt::Display for StrongProfileCoreLirRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid MIR-to-LIR core bridge relation: {self:?}"
        )
    }
}

impl std::error::Error for StrongProfileCoreLirRelationError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongProfileRelationError {
    OutputMismatch,
    CoreBranchMismatch,
    CoreCallableCoverage { expected: usize, actual: usize },
    InvalidCoreCallableCandidateDefinition { index: usize },
    CoreCallableMismatch { index: usize },
    CoreCallableSignatureMismatch { index: usize },
    CoreShapeRootCoverage { expected: usize, actual: usize },
    CoreShapeRootMismatch { index: usize },
    InvalidInitializationCycleDefinition,
    InitializationCycleMismatch,
    MissingInitializationCycleSignature,
    InitializationCycleSignatureMismatch,
}

impl fmt::Display for StrongProfileRelationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong-profile cross-layer relation: {self:?}"
        )
    }
}

impl std::error::Error for StrongProfileRelationError {}

pub type SingleConeCompileSectionDecodeError = CompileSectionDecodeError;

#[derive(Debug)]
pub enum StrongCompileArtifactValidationError {
    Decode(Box<SingleConeCompileSectionDecodeError>),
    Identities(Box<IdentityValidationError>),
    Foundations(Box<StrongProfileFoundationError>),
    LocalProduction(Box<StrongProfileLocalProductionError>),
    Relations(Box<StrongProfileRelationError>),
    ExternalBridges(Box<StrongExternalLirBridgeReconstructionError>),
    LirProduction(Box<StrongProfileLirProductionError>),
    NativeBoundary(Box<NativeBoundaryCompileError>),
    Commit(Box<CompileCommitError>),
}

impl fmt::Display for StrongCompileArtifactValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid SingleConeStrong Compile artifact: {self:?}"
        )
    }
}

impl std::error::Error for StrongCompileArtifactValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Decode(error) => error.as_ref(),
            Self::Identities(error) => error.as_ref(),
            Self::Foundations(error) => error.as_ref(),
            Self::LocalProduction(error) => error.as_ref(),
            Self::Relations(error) => error.as_ref(),
            Self::ExternalBridges(error) => error.as_ref(),
            Self::LirProduction(error) => error.as_ref(),
            Self::NativeBoundary(error) => error.as_ref(),
            Self::Commit(error) => error.as_ref(),
        })
    }
}

#[cfg(test)]
pub(crate) mod tests;
