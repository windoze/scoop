//! Deterministic archive assembly for the cross-Cone layout strong profile.

use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceSectionV1,
    CrossConeTypeSemanticsSectionV1, OdrFreeHirFoundation,
};
use scoop_identity::ConeIdentity;
use scoop_lir::{
    ConeLirFoundation, CrossConeLayoutAbiSectionV1, CrossConeLirBridgeSectionV1,
    ValidatedLirTargetSelection,
};
use scoop_mir::{
    CoreBootstrapBridgeSectionV1, CrossConeMirBridgeSectionV1, CrossConeMirTypeBridgeSectionV1,
    OdrFreeMirFoundation,
};
use scoop_wire::encode;

use super::{StrongArtifactSectionV1, verify_layout_link_objects};
use crate::{
    ArtifactCapabilityProfile, ArtifactFingerprint, BootstrapManifest, CanonicalSlibArchive,
    CompatibilityRecord, ConeRecord, CrossConeLayoutProductionManifestV1,
    CrossConeLinkSemanticImportSetV1, DependencyRecord, LinkIdentityClosureSectionV1,
    ManifestSection, MemberPurposeSet, ProducerRecord, SemanticFingerprintRecord, SlibMember,
    VerifiedCodeFingerprintV2, VerifiedCrossConeLayoutCodeFingerprintV1,
    manifest_single_cone_production_capability,
};

mod error;
pub use error::CrossConeLayoutStrongArtifactWriteError;
mod sections;
use sections::{LayoutMetadataInput, assemble_metadata};

/// Complete typed input for one `CrossConeLayoutStrong` archive.
///
/// Strong V2, Link closures, object coverage, fingerprints, and the production
/// manifest come from the same completed object production.
pub struct CrossConeLayoutStrongArtifactInputV1<'ir> {
    producer: ProducerRecord,
    cone: ConeRecord,
    direct_dependencies: Vec<DependencyRecord>,
    hir_foundation: &'ir OdrFreeHirFoundation,
    hir_production: &'ir CoreBootstrapInterfaceSectionV1,
    hir_cross_cone: CrossConeHirInterfaceSectionV1,
    hir_type_semantics: &'ir CrossConeTypeSemanticsSectionV1,
    mir_foundation: &'ir OdrFreeMirFoundation,
    mir_production: &'ir CoreBootstrapBridgeSectionV1,
    mir_cross_cone: &'ir CrossConeMirBridgeSectionV1,
    mir_type_bridge: &'ir CrossConeMirTypeBridgeSectionV1<'ir>,
    lir_foundation: &'ir ConeLirFoundation,
    lir_cross_cone: &'ir CrossConeLirBridgeSectionV1,
    lir_layout_abi: &'ir CrossConeLayoutAbiSectionV1<'ir>,
    layout_code: VerifiedCrossConeLayoutCodeFingerprintV1<'ir>,
    link_objects: Vec<SlibMember>,
}

impl<'ir> CrossConeLayoutStrongArtifactInputV1<'ir> {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        producer: ProducerRecord,
        cone: ConeRecord,
        direct_dependencies: Vec<DependencyRecord>,
        hir_foundation: &'ir OdrFreeHirFoundation,
        hir_production: &'ir CoreBootstrapInterfaceSectionV1,
        hir_cross_cone: CrossConeHirInterfaceSectionV1,
        hir_type_semantics: &'ir CrossConeTypeSemanticsSectionV1,
        mir_foundation: &'ir OdrFreeMirFoundation,
        mir_production: &'ir CoreBootstrapBridgeSectionV1,
        mir_cross_cone: &'ir CrossConeMirBridgeSectionV1,
        mir_type_bridge: &'ir CrossConeMirTypeBridgeSectionV1<'ir>,
        lir_foundation: &'ir ConeLirFoundation,
        lir_cross_cone: &'ir CrossConeLirBridgeSectionV1,
        lir_layout_abi: &'ir CrossConeLayoutAbiSectionV1<'ir>,
        layout_code: VerifiedCrossConeLayoutCodeFingerprintV1<'ir>,
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
        }
    }
}

/// Canonical layout-profile bytes and their publication identities.
#[derive(Debug, Eq, PartialEq)]
pub struct AssembledCrossConeLayoutStrongArtifactV1 {
    archive: CanonicalSlibArchive,
    summary: crate::CrossConeArtifactSummary,
}

impl AssembledCrossConeLayoutStrongArtifactV1 {
    pub fn write(
        input: CrossConeLayoutStrongArtifactInputV1<'_>,
    ) -> Result<Self, CrossConeLayoutStrongArtifactWriteError> {
        let CrossConeLayoutStrongArtifactInputV1 {
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
        } = input;
        lir_foundation
            .require_strong()
            .map_err(CrossConeLayoutStrongArtifactWriteError::LirProfile)?;
        let identity = cone.identity();
        let link_object_count = link_objects.len();
        let (code, callable_link_closure, layout_link_closure) = layout_code.into_parts();

        validate_producers(
            identity,
            &code,
            mir_cross_cone,
            mir_type_bridge,
            lir_foundation,
            lir_cross_cone,
            lir_layout_abi,
            &callable_link_closure,
            &layout_link_closure,
        )?;
        verify_layout_link_objects(&code, &link_objects)?;
        validate_link_projections(
            &code,
            lir_cross_cone,
            lir_layout_abi,
            &callable_link_closure,
            &layout_link_closure,
        )?;

        let target_selection = code.undefined_symbols().selection();
        if lir_layout_abi.target_profile() != target_selection.target() {
            return Err(CrossConeLayoutStrongArtifactWriteError::TargetMismatch);
        }
        let compatibility = CompatibilityRecord::new(
            target_selection,
            ArtifactCapabilityProfile::CROSS_CONE_LAYOUT_STRONG,
        )
        .map_err(CrossConeLayoutStrongArtifactWriteError::Compatibility)?;
        let link_identity_closure = LinkIdentityClosureSectionV1::from_verified_layout_code(&code)
            .map_err(CrossConeLayoutStrongArtifactWriteError::LinkIdentityClosure)?;

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
            callable_link_closure: &callable_link_closure,
            layout_link_closure: &layout_link_closure,
        })?;

        let production_manifest = CrossConeLayoutProductionManifestV1::from_verified_code(code);
        let foundation_fingerprints = SemanticFingerprintRecord::from_metadata_sections(
            &compatibility,
            &direct_dependencies,
            &metadata.hir.sections,
            &metadata.mir.sections,
            &metadata.lir.sections,
        )
        .map_err(CrossConeLayoutStrongArtifactWriteError::SemanticFingerprints)?;
        let semantic_fingerprints = SemanticFingerprintRecord::from_layout_production_manifest(
            foundation_fingerprints.hir(),
            foundation_fingerprints.mir(),
            foundation_fingerprints.lir(),
            &production_manifest,
        );
        let manifest_section = ManifestSection::new(
            manifest_single_cone_production_capability(),
            MemberPurposeSet::LINK,
            encode(&production_manifest).map_err(|source| {
                CrossConeLayoutStrongArtifactWriteError::Encoding {
                    section: StrongArtifactSectionV1::ProductionManifest,
                    source,
                }
            })?,
        )
        .map_err(CrossConeLayoutStrongArtifactWriteError::ManifestSection)?;

        let mut members = Vec::with_capacity(3 + link_objects.len());
        members.push(metadata.hir.into_member(identity)?);
        members.push(metadata.mir.into_member(identity)?);
        members.push(metadata.lir.into_member(identity)?);
        members.extend(link_objects);
        let manifest = BootstrapManifest::new(
            producer,
            compatibility,
            cone,
            direct_dependencies,
            &members,
            semantic_fingerprints,
            vec![manifest_section],
        )
        .map_err(CrossConeLayoutStrongArtifactWriteError::Manifest)?;
        let summary = crate::CrossConeArtifactSummary::from_layout(
            &manifest,
            production_manifest.code_proof().production().projection(),
            link_object_count,
            target_selection,
        );
        let archive = CanonicalSlibArchive::write_bootstrap(&manifest, members)
            .map_err(CrossConeLayoutStrongArtifactWriteError::Archive)?;
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
}

#[allow(clippy::too_many_arguments)]
fn validate_producers(
    expected: ConeIdentity,
    code: &VerifiedCodeFingerprintV2,
    mir_cross_cone: &CrossConeMirBridgeSectionV1,
    mir_type_bridge: &CrossConeMirTypeBridgeSectionV1<'_>,
    lir_foundation: &ConeLirFoundation,
    lir_cross_cone: &CrossConeLirBridgeSectionV1,
    lir_layout_abi: &CrossConeLayoutAbiSectionV1<'_>,
    callable_link_closure: &crate::CrossConeLinkClosureSectionV1,
    layout_link_closure: &crate::CrossConeLayoutLinkClosureSectionV1<'_>,
) -> Result<(), CrossConeLayoutStrongArtifactWriteError> {
    for (component, actual) in [
        ("Code", code.producer()),
        ("MIR bridge", mir_cross_cone.artifact()),
        ("MIR type bridge", mir_type_bridge.provider()),
        ("LIR foundation", lir_foundation.producer()),
        ("LIR bridge", lir_cross_cone.artifact()),
        ("LIR layout ABI", lir_layout_abi.provider()),
        ("callable Link closure", callable_link_closure.consumer()),
        ("layout Link closure", layout_link_closure.consumer()),
    ] {
        if actual != expected {
            return Err(
                CrossConeLayoutStrongArtifactWriteError::ComponentProducerMismatch {
                    component,
                    expected,
                    actual,
                },
            );
        }
    }
    Ok(())
}

fn validate_link_projections(
    code: &VerifiedCodeFingerprintV2,
    lir_cross_cone: &CrossConeLirBridgeSectionV1,
    lir_layout_abi: &CrossConeLayoutAbiSectionV1<'_>,
    callable_link_closure: &crate::CrossConeLinkClosureSectionV1,
    layout_link_closure: &crate::CrossConeLayoutLinkClosureSectionV1<'_>,
) -> Result<(), CrossConeLayoutStrongArtifactWriteError> {
    let callable_imports = CrossConeLinkSemanticImportSetV1::from_lir_bridge(lir_cross_cone)
        .map_err(CrossConeLayoutStrongArtifactWriteError::CallableSemanticImports)?;
    if &callable_imports != callable_link_closure.semantic_imports() {
        return Err(CrossConeLayoutStrongArtifactWriteError::CallableSemanticImportMismatch);
    }
    let layout_imports =
        encode(lir_layout_abi.selected().physical_imports()).map_err(|source| {
            CrossConeLayoutStrongArtifactWriteError::Encoding {
                section: StrongArtifactSectionV1::LirCrossConeLayoutAbi,
                source,
            }
        })?;
    let closure_imports = encode(layout_link_closure.semantic_imports()).map_err(|source| {
        CrossConeLayoutStrongArtifactWriteError::Encoding {
            section: StrongArtifactSectionV1::CrossConeLayoutLinkClosure,
            source,
        }
    })?;
    if layout_imports != closure_imports {
        return Err(CrossConeLayoutStrongArtifactWriteError::LayoutSemanticImportMismatch);
    }
    let expected_objects = code.production().link_objects().projection();
    if callable_link_closure
        .object_coverage()
        .verified_link_objects()
        != expected_objects
        || layout_link_closure
            .object_coverage()
            .verified_link_objects()
            != expected_objects
    {
        return Err(CrossConeLayoutStrongArtifactWriteError::LinkObjectProjectionMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
