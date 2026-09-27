//! Deterministic archive assembly for the single-Cone strong profile.

use std::fmt;

use scoop_hir::{CoreBootstrapInterfaceSectionV1, OdrFreeHirFoundation};
use scoop_identity::ConeIdentity;
use scoop_lir::{ConeLirFoundation, ValidatedLirTargetSelection};
use scoop_mir::{CoreBootstrapBridgeSectionV1, OdrFreeMirFoundation};
use scoop_wire::{HashError, encode};

use crate::{
    ArtifactCapabilityProfile, ArtifactFingerprint, BootstrapManifest, BootstrapManifestError,
    CanonicalSlibArchive, CompatibilityRecord, ConeRecord, DependencyRecord,
    LinkIdentityClosureBuildError, LinkIdentityClosureSectionV1, ManifestSection,
    ManifestSectionError, MemberPurposeSet, MetadataEnvelopeError, MetadataLocation,
    MetadataSectionError, ProducerRecord, SemanticFingerprintError, SemanticFingerprintRecord,
    SingleConeProductionManifestV1, SlibMember, SlibMemberId, SlibMemberRecordError,
    SlibWriteError, hir_core_bootstrap_interface_capability, hir_identity_foundation_capability,
    lir_identity_foundation_capability, lir_link_identity_closure_capability,
    lir_strong_production_capability, manifest_single_cone_production_capability,
    mir_core_bootstrap_bridge_capability, mir_identity_foundation_capability,
};

mod assembly;
use assembly::{
    LayerAssembly, StrongArtifactAssemblyError, build_section, verify_layout_link_objects,
    verify_link_objects,
};

mod layout;
pub use layout::*;

/// Complete, closed input to the only `SingleConeStrong` archive writer.
///
/// Every required metadata and manifest contribution is mandatory. The writer
/// derives the Link identity closure and all outer envelopes itself, so callers
/// cannot publish a partial or differently ordered strong profile.
pub struct SingleConeStrongArtifactInputV1<'ir> {
    producer: ProducerRecord,
    cone: ConeRecord,
    direct_dependencies: Vec<DependencyRecord>,
    hir_foundation: &'ir OdrFreeHirFoundation,
    hir_production: &'ir CoreBootstrapInterfaceSectionV1,
    mir_foundation: &'ir OdrFreeMirFoundation,
    mir_production: &'ir CoreBootstrapBridgeSectionV1,
    lir_foundation: &'ir ConeLirFoundation,
    production_manifest: SingleConeProductionManifestV1,
    link_objects: Vec<SlibMember>,
}

impl<'ir> SingleConeStrongArtifactInputV1<'ir> {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        producer: ProducerRecord,
        cone: ConeRecord,
        direct_dependencies: Vec<DependencyRecord>,
        hir_foundation: &'ir OdrFreeHirFoundation,
        hir_production: &'ir CoreBootstrapInterfaceSectionV1,
        mir_foundation: &'ir OdrFreeMirFoundation,
        mir_production: &'ir CoreBootstrapBridgeSectionV1,
        lir_foundation: &'ir ConeLirFoundation,
        production_manifest: SingleConeProductionManifestV1,
        link_objects: Vec<SlibMember>,
    ) -> Self {
        Self {
            producer,
            cone,
            direct_dependencies,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            production_manifest,
            link_objects,
        }
    }
}

/// Canonical final bytes together with the identities needed by publication.
#[derive(Debug, Eq, PartialEq)]
pub struct AssembledSingleConeStrongArtifactV1 {
    archive: CanonicalSlibArchive,
    artifact_fingerprint: ArtifactFingerprint,
    target_selection: ValidatedLirTargetSelection,
}

impl AssembledSingleConeStrongArtifactV1 {
    pub fn write(
        input: SingleConeStrongArtifactInputV1<'_>,
    ) -> Result<Self, SingleConeStrongArtifactWriteError> {
        let SingleConeStrongArtifactInputV1 {
            producer,
            cone,
            direct_dependencies,
            hir_foundation,
            hir_production,
            mir_foundation,
            mir_production,
            lir_foundation,
            production_manifest,
            link_objects,
        } = input;
        lir_foundation
            .require_strong()
            .map_err(SingleConeStrongArtifactWriteError::LirProfile)?;
        let identity = cone.identity();
        let code = production_manifest.code_proof();
        if code.producer() != identity {
            return Err(SingleConeStrongArtifactWriteError::CodeProducerMismatch {
                expected: identity,
                actual: code.producer(),
            });
        }
        if lir_foundation.producer() != identity {
            return Err(
                SingleConeStrongArtifactWriteError::LirFoundationProducerMismatch {
                    expected: identity,
                    actual: lir_foundation.producer(),
                },
            );
        }
        verify_link_objects(code, &link_objects)?;

        let target_selection = code.undefined_symbols().selection();
        let compatibility = CompatibilityRecord::new(
            target_selection,
            ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        )
        .map_err(SingleConeStrongArtifactWriteError::Compatibility)?;
        let closure = LinkIdentityClosureSectionV1::from_verified_code(code)
            .map_err(SingleConeStrongArtifactWriteError::LinkClosure)?;

        let hir = LayerAssembly::new(
            MetadataLocation::Hir,
            vec![
                build_section(
                    StrongArtifactSectionV1::HirFoundation,
                    MetadataLocation::Hir,
                    hir_identity_foundation_capability(),
                    MemberPurposeSet::COMPILE,
                    hir_foundation,
                )?,
                build_section(
                    StrongArtifactSectionV1::HirProduction,
                    MetadataLocation::Hir,
                    hir_core_bootstrap_interface_capability(),
                    MemberPurposeSet::COMPILE,
                    hir_production,
                )?,
            ],
        )?;
        let mir = LayerAssembly::new(
            MetadataLocation::Mir,
            vec![
                build_section(
                    StrongArtifactSectionV1::MirFoundation,
                    MetadataLocation::Mir,
                    mir_identity_foundation_capability(),
                    MemberPurposeSet::COMPILE,
                    mir_foundation,
                )?,
                build_section(
                    StrongArtifactSectionV1::MirProduction,
                    MetadataLocation::Mir,
                    mir_core_bootstrap_bridge_capability(),
                    MemberPurposeSet::COMPILE,
                    mir_production,
                )?,
            ],
        )?;
        let lir = LayerAssembly::new(
            MetadataLocation::Lir,
            vec![
                build_section(
                    StrongArtifactSectionV1::LirFoundation,
                    MetadataLocation::Lir,
                    lir_identity_foundation_capability(),
                    MemberPurposeSet::COMPILE,
                    lir_foundation,
                )?,
                build_section(
                    StrongArtifactSectionV1::LirProduction,
                    MetadataLocation::Lir,
                    lir_strong_production_capability(),
                    MemberPurposeSet::COMPILE_AND_LINK,
                    code.production().strong_production(),
                )?,
                build_section(
                    StrongArtifactSectionV1::LinkIdentityClosure,
                    MetadataLocation::Lir,
                    lir_link_identity_closure_capability(),
                    MemberPurposeSet::LINK,
                    &closure,
                )?,
            ],
        )?;

        let foundation_fingerprints = SemanticFingerprintRecord::from_metadata_sections(
            &compatibility,
            &direct_dependencies,
            &hir.sections,
            &mir.sections,
            &lir.sections,
        )
        .map_err(SingleConeStrongArtifactWriteError::SemanticFingerprints)?;
        let semantic_fingerprints = SemanticFingerprintRecord::from_production_manifest(
            foundation_fingerprints.hir(),
            foundation_fingerprints.mir(),
            foundation_fingerprints.lir(),
            &production_manifest,
        );
        let manifest_section = ManifestSection::new(
            manifest_single_cone_production_capability(),
            MemberPurposeSet::LINK,
            encode(&production_manifest).map_err(|source| {
                SingleConeStrongArtifactWriteError::Encoding {
                    section: StrongArtifactSectionV1::ProductionManifest,
                    source,
                }
            })?,
        )
        .map_err(SingleConeStrongArtifactWriteError::ManifestSection)?;

        let mut members = Vec::with_capacity(3 + link_objects.len());
        members.push(hir.into_member(identity)?);
        members.push(mir.into_member(identity)?);
        members.push(lir.into_member(identity)?);
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
        .map_err(SingleConeStrongArtifactWriteError::Manifest)?;
        let artifact_fingerprint = manifest.artifact_fingerprint();
        let archive = CanonicalSlibArchive::write_bootstrap(&manifest, members)
            .map_err(SingleConeStrongArtifactWriteError::Archive)?;
        Ok(Self {
            archive,
            artifact_fingerprint,
            target_selection,
        })
    }

    pub const fn artifact_fingerprint(&self) -> ArtifactFingerprint {
        self.artifact_fingerprint
    }

    pub const fn target_selection(&self) -> ValidatedLirTargetSelection {
        self.target_selection
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.archive.as_bytes()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongArtifactSectionV1 {
    HirFoundation,
    HirProduction,
    MirFoundation,
    MirProduction,
    LirFoundation,
    LirProduction,
    LinkIdentityClosure,
    HirCrossConeInterface,
    HirCrossConeTypeSemantics,
    MirCrossConeBridge,
    MirCrossConeTypeBridge,
    LirCrossConeBridge,
    LirCrossConeLayoutAbi,
    CrossConeLinkClosure,
    CrossConeLayoutLinkClosure,
    ProductionManifest,
}

#[derive(Debug)]
pub enum SingleConeStrongArtifactWriteError {
    LirProfile(scoop_lir::ConeLirFoundationError),
    CodeProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    LirFoundationProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    LinkObjectCount {
        expected: usize,
        actual: usize,
    },
    InvalidLinkObjectRole {
        index: usize,
        member: SlibMemberId,
    },
    LinkObjectMismatch {
        index: usize,
        expected: SlibMemberId,
        actual: SlibMemberId,
    },
    LinkObjectFingerprint(HashError),
    Compatibility(HashError),
    LinkClosure(LinkIdentityClosureBuildError),
    Encoding {
        section: StrongArtifactSectionV1,
        source: scoop_wire::cbor::EncodeError,
    },
    MetadataSection {
        section: StrongArtifactSectionV1,
        source: MetadataSectionError,
    },
    MetadataEnvelope {
        location: MetadataLocation,
        source: MetadataEnvelopeError,
    },
    MetadataEnvelopeEncoding {
        location: MetadataLocation,
        source: scoop_wire::cbor::EncodeError,
    },
    MetadataMember {
        location: MetadataLocation,
        source: SlibMemberRecordError,
    },
    SemanticFingerprints(SemanticFingerprintError),
    ManifestSection(ManifestSectionError),
    Manifest(BootstrapManifestError),
    Archive(SlibWriteError),
}

impl From<StrongArtifactAssemblyError> for SingleConeStrongArtifactWriteError {
    fn from(error: StrongArtifactAssemblyError) -> Self {
        match error {
            StrongArtifactAssemblyError::LinkObjectCount { expected, actual } => {
                Self::LinkObjectCount { expected, actual }
            }
            StrongArtifactAssemblyError::InvalidLinkObjectRole { index, member } => {
                Self::InvalidLinkObjectRole { index, member }
            }
            StrongArtifactAssemblyError::LinkObjectMismatch {
                index,
                expected,
                actual,
            } => Self::LinkObjectMismatch {
                index,
                expected,
                actual,
            },
            StrongArtifactAssemblyError::LinkObjectFingerprint(source) => {
                Self::LinkObjectFingerprint(source)
            }
            StrongArtifactAssemblyError::Encoding { section, source } => {
                Self::Encoding { section, source }
            }
            StrongArtifactAssemblyError::MetadataSection { section, source } => {
                Self::MetadataSection { section, source }
            }
            StrongArtifactAssemblyError::MetadataEnvelope { location, source } => {
                Self::MetadataEnvelope { location, source }
            }
            StrongArtifactAssemblyError::MetadataEnvelopeEncoding { location, source } => {
                Self::MetadataEnvelopeEncoding { location, source }
            }
            StrongArtifactAssemblyError::MetadataMember { location, source } => {
                Self::MetadataMember { location, source }
            }
        }
    }
}

impl fmt::Display for SingleConeStrongArtifactWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot assemble single-Cone strong artifact: {self:?}"
        )
    }
}

impl std::error::Error for SingleConeStrongArtifactWriteError {}
