//! Deterministic archive assembly for the cross-Cone layout strong profile.

use crate::{
    ArtifactCapabilityProfile, ArtifactFingerprint, BootstrapManifest, CanonicalSlibArchive,
    CompatibilityRecord, ConeRecord, CrossConeLayoutCodeDataV1, DependencyRecord,
    FingerprintAvailability, LinkIdentityClosureSectionV1, ProducerRecord,
    SemanticFingerprintRecord, SlibMember,
};
use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1,
    CrossConeTypeSemanticsSectionV1,
};
use scoop_lir::{
    ConeLirFoundation, CrossConeLayoutAbiSectionV1, CrossConeLirBridgeSectionV1,
    ValidatedLirTargetSelection,
};
use scoop_mir::{
    CoreBootstrapBridgeSectionV1, CrossConeMirBridgeSectionV1, CrossConeMirTypeBridgeSectionV1,
};

mod error;
pub use error::CrossConeLayoutArtifactWriteError;
mod sections;
use sections::{LayoutMetadataInput, assemble_metadata};
mod odr_reuse;

/// Complete typed input for one `CrossConeLayoutStrong` archive.
///
/// Strong V2, Link closures, object coverage, fingerprints, and the production
/// manifest come from the same completed object production.
pub struct CrossConeLayoutArtifactInputV1<'ir> {
    producer: ProducerRecord,
    cone: ConeRecord,
    direct_dependencies: Vec<DependencyRecord>,
    hir_foundation: &'ir scoop_hir::CanonicalHirFoundation,
    hir_production: &'ir CoreBootstrapInterfaceSectionV1,
    hir_cross_cone: CrossConeHirInterfaceSectionV1,
    hir_type_semantics: &'ir CrossConeTypeSemanticsSectionV1,
    mir_foundation: &'ir scoop_mir::CanonicalMirFoundation,
    mir_production: &'ir CoreBootstrapBridgeSectionV1,
    mir_cross_cone: &'ir CrossConeMirBridgeSectionV1,
    mir_type_bridge: &'ir CrossConeMirTypeBridgeSectionV1<'ir>,
    lir_foundation: &'ir ConeLirFoundation,
    lir_cross_cone: &'ir CrossConeLirBridgeSectionV1,
    lir_layout_abi: &'ir CrossConeLayoutAbiSectionV1<'ir>,
    layout_code: CrossConeLayoutCodeDataV1,
    link_objects: Vec<SlibMember>,
    test_directory: Option<crate::TestDirectory>,
    dependencies: Option<&'ir crate::ValidatedCrossConeArtifactClosure>,
}

impl<'ir> CrossConeLayoutArtifactInputV1<'ir> {
    pub fn with_test_directory(mut self, directory: Option<crate::TestDirectory>) -> Self {
        self.test_directory = directory;
        self
    }

    pub fn with_dependencies(
        mut self,
        dependencies: Option<&'ir crate::ValidatedCrossConeArtifactClosure>,
    ) -> Self {
        self.dependencies = dependencies;
        self
    }

    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        producer: ProducerRecord,
        cone: ConeRecord,
        direct_dependencies: Vec<DependencyRecord>,
        hir_foundation: &'ir scoop_hir::CanonicalHirFoundation,
        hir_production: &'ir CoreBootstrapInterfaceSectionV1,
        hir_cross_cone: CrossConeHirInterfaceSectionV1,
        hir_type_semantics: &'ir CrossConeTypeSemanticsSectionV1,
        mir_foundation: &'ir scoop_mir::CanonicalMirFoundation,
        mir_production: &'ir CoreBootstrapBridgeSectionV1,
        mir_cross_cone: &'ir CrossConeMirBridgeSectionV1,
        mir_type_bridge: &'ir CrossConeMirTypeBridgeSectionV1<'ir>,
        lir_foundation: &'ir ConeLirFoundation,
        lir_cross_cone: &'ir CrossConeLirBridgeSectionV1,
        lir_layout_abi: &'ir CrossConeLayoutAbiSectionV1<'ir>,
        layout_code: CrossConeLayoutCodeDataV1,
        link_objects: Vec<SlibMember>,
    ) -> Self {
        Self {
            producer,
            cone,
            direct_dependencies,
            hir_foundation,
            hir_production,
            hir_cross_cone,
            hir_type_semantics,
            mir_foundation,
            mir_production,
            mir_cross_cone,
            mir_type_bridge,
            lir_foundation,
            lir_cross_cone,
            lir_layout_abi,
            layout_code,
            link_objects,
            test_directory: None,
            dependencies: None,
        }
    }
}

/// Canonical layout-profile bytes and their publication identities.
#[derive(Debug, Eq, PartialEq)]
pub struct AssembledCrossConeLayoutArtifactV1 {
    archive: CanonicalSlibArchive,
    summary: crate::CrossConeArtifactSummary,
}

impl AssembledCrossConeLayoutArtifactV1 {
    pub fn write(
        input: CrossConeLayoutArtifactInputV1<'_>,
    ) -> Result<Self, CrossConeLayoutArtifactWriteError> {
        let CrossConeLayoutArtifactInputV1 {
            producer,
            cone,
            direct_dependencies,
            hir_foundation,
            hir_production,
            hir_cross_cone,
            hir_type_semantics,
            mir_foundation,
            mir_production,
            mir_cross_cone,
            mir_type_bridge,
            lir_foundation,
            lir_cross_cone,
            lir_layout_abi,
            layout_code,
            mut link_objects,
            test_directory,
            dependencies,
        } = input;
        let identity = cone.identity();
        let (code, link_support) = layout_code.into_parts();

        let target_selection = code.undefined_symbols().selection();
        let compatibility = CompatibilityRecord::new(
            target_selection,
            ArtifactCapabilityProfile::CROSS_CONE_GENERIC,
        )
        .map_err(CrossConeLayoutArtifactWriteError::Compatibility)?;
        let mut link_identity_closure =
            LinkIdentityClosureSectionV1::from_verified_layout_code(&code);
        if let Some(dependencies) = dependencies {
            let removed = odr_reuse::select(
                &link_objects,
                &link_identity_closure,
                code.production().projection().odr_members(),
                dependencies,
            );
            link_objects.retain(|object| !removed.contains(&object.record().id()));
            link_identity_closure.remove_members(&removed);
        }

        let metadata = assemble_metadata(LayoutMetadataInput {
            hir_foundation,
            hir_production,
            hir_cross_cone,
            hir_type_semantics,
            mir_foundation,
            mir_production,
            mir_cross_cone,
            mir_type_bridge,
            lir_foundation,
            lir_cross_cone,
            lir_layout_abi,
            code: &code,
            link_identity_closure: &link_identity_closure,
            link_support: &link_support,
        })?;

        let foundation_fingerprints = SemanticFingerprintRecord::from_metadata_sections(
            &compatibility,
            &direct_dependencies,
            &metadata.hir.sections,
            &metadata.mir.sections,
            &metadata.lir.sections,
        )
        .and_then(|fingerprints| {
            fingerprints.with_odr_implementations(link_identity_closure.odr_members())
        })
        .map_err(CrossConeLayoutArtifactWriteError::SemanticFingerprints)?;
        let semantic_fingerprints = SemanticFingerprintRecord::from_digests(
            foundation_fingerprints.hir(),
            foundation_fingerprints.mir(),
            foundation_fingerprints.lir(),
            FingerprintAvailability::Available(code.fingerprint()),
            FingerprintAvailability::Available(
                code.production().projection().runtime_image_fingerprint(),
            ),
        );

        let mut members = Vec::with_capacity(3 + link_objects.len());
        members.push(metadata.hir.into_member(identity)?);
        members.push(metadata.mir.into_member(identity)?);
        members.push(metadata.lir.into_member(identity)?);
        members.extend(link_objects);
        let mut manifest_sections = Vec::new();
        if let Some(directory) = &test_directory {
            manifest_sections.push(
                directory
                    .section()
                    .map_err(CrossConeLayoutArtifactWriteError::TestDirectory)?,
            );
        }
        let manifest = BootstrapManifest::new(
            producer,
            compatibility,
            cone,
            direct_dependencies,
            &members,
            semantic_fingerprints,
            manifest_sections,
        )
        .map_err(CrossConeLayoutArtifactWriteError::Manifest)?;
        let summary = crate::CrossConeArtifactSummary::from_layout(
            &manifest,
            target_selection,
            test_directory,
        );
        let archive = CanonicalSlibArchive::write_bootstrap(&manifest, members)
            .map_err(CrossConeLayoutArtifactWriteError::Archive)?;
        Ok(Self { archive, summary })
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.summary.artifact_fingerprint()
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.summary.target_selection()
    }

    pub const fn summary(&self) -> &crate::CrossConeArtifactSummary {
        &self.summary
    }

    pub fn publish(
        &self,
        destination: &std::path::Path,
    ) -> Result<crate::PublishedCrossConeArtifact, crate::CrossConeArtifactPublishError> {
        crate::PublishedCrossConeArtifact::write(&self.archive, &self.summary, destination)
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.archive.as_bytes()
    }

    pub fn object_payloads(&self) -> impl Iterator<Item = (scoop_wire::Digest256, &[u8])> {
        self.archive.object_payloads()
    }
}

#[cfg(test)]
mod tests;
