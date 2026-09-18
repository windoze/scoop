//! Deterministic archive assembly for the cross-Cone semantics strong profile.

use std::fmt;

use scoop_hir::{
    CoreBootstrapInterfaceSectionV1, CrossConeHirInterfaceIndexError,
    CrossConeHirInterfaceSectionV1, OdrFreeHirFoundation,
};
use scoop_identity::ConeIdentity;
use scoop_lir::{CrossConeLirBridgeSectionV1, OdrFreeLirFoundation, ValidatedLirTargetSelection};
use scoop_mir::{CoreBootstrapBridgeSectionV1, CrossConeMirBridgeSectionV1, OdrFreeMirFoundation};
use scoop_wire::{HashError, encode};

use super::{
    LayerAssembly, StrongArtifactAssemblyError, StrongArtifactSectionV1, build_section,
    verify_link_objects,
};
use crate::{
    ArtifactCapabilityProfile, ArtifactFingerprint, BootstrapManifest, BootstrapManifestError,
    CanonicalSlibArchive, CompatibilityRecord, ConeRecord, CrossConeLinkSemanticImportBuildError,
    CrossConeLinkSemanticImportSetV1, DependencyRecord, LinkIdentityClosureBuildError,
    LinkIdentityClosureSectionV1, ManifestSection, ManifestSectionError, MemberPurposeSet,
    MetadataEnvelopeError, MetadataLocation, MetadataSectionError, ProducerRecord,
    SemanticFingerprintError, SemanticFingerprintRecord, SingleConeProductionManifestV1,
    SlibMember, SlibMemberId, SlibMemberRecordError, SlibWriteError,
    VerifiedCrossConeCodeFingerprintV1, hir_core_bootstrap_interface_capability,
    hir_cross_cone_interface_capability, hir_identity_foundation_capability,
    lir_cross_cone_link_closure_capability, lir_cross_cone_param_free_bridge_capability,
    lir_identity_foundation_capability, lir_link_identity_closure_capability,
    lir_strong_production_capability, manifest_single_cone_production_capability,
    mir_core_bootstrap_bridge_capability, mir_cross_cone_param_free_bridge_capability,
    mir_identity_foundation_capability,
};

/// Complete typed input for one `CrossConeSemanticsStrong` archive.
///
/// The writer consumes the cross-Cone code proof and derives both Link-only
/// closures and the production manifest from it. Callers therefore cannot
/// pair a code fingerprint with a closure produced from another use set.
pub struct CrossConeStrongArtifactInputV1<'ir> {
    producer: ProducerRecord,
    cone: ConeRecord,
    direct_dependencies: Vec<DependencyRecord>,
    hir_foundation: &'ir OdrFreeHirFoundation,
    hir_production: &'ir CoreBootstrapInterfaceSectionV1,
    hir_cross_cone: CrossConeHirInterfaceSectionV1,
    mir_foundation: &'ir OdrFreeMirFoundation,
    mir_production: &'ir CoreBootstrapBridgeSectionV1,
    mir_cross_cone: &'ir CrossConeMirBridgeSectionV1,
    lir_foundation: &'ir OdrFreeLirFoundation,
    lir_cross_cone: &'ir CrossConeLirBridgeSectionV1,
    cross_cone_code: VerifiedCrossConeCodeFingerprintV1,
    link_objects: Vec<SlibMember>,
}

impl<'ir> CrossConeStrongArtifactInputV1<'ir> {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        producer: ProducerRecord,
        cone: ConeRecord,
        direct_dependencies: Vec<DependencyRecord>,
        hir_foundation: &'ir OdrFreeHirFoundation,
        hir_production: &'ir CoreBootstrapInterfaceSectionV1,
        hir_cross_cone: CrossConeHirInterfaceSectionV1,
        mir_foundation: &'ir OdrFreeMirFoundation,
        mir_production: &'ir CoreBootstrapBridgeSectionV1,
        mir_cross_cone: &'ir CrossConeMirBridgeSectionV1,
        lir_foundation: &'ir OdrFreeLirFoundation,
        lir_cross_cone: &'ir CrossConeLirBridgeSectionV1,
        cross_cone_code: VerifiedCrossConeCodeFingerprintV1,
        link_objects: Vec<SlibMember>,
    ) -> Self {
        Self {
            producer,
            cone,
            direct_dependencies,
            hir_foundation,
            hir_production,
            hir_cross_cone,
            mir_foundation,
            mir_production,
            mir_cross_cone,
            lir_foundation,
            lir_cross_cone,
            cross_cone_code,
            link_objects,
        }
    }
}

/// Canonical cross-Cone artifact bytes and their publication identities.
#[derive(Debug, Eq, PartialEq)]
pub struct AssembledCrossConeStrongArtifactV1 {
    archive: CanonicalSlibArchive,
    artifact_fingerprint: ArtifactFingerprint,
    target_selection: ValidatedLirTargetSelection,
}

impl AssembledCrossConeStrongArtifactV1 {
    pub fn write(
        input: CrossConeStrongArtifactInputV1<'_>,
    ) -> Result<Self, CrossConeStrongArtifactWriteError> {
        let CrossConeStrongArtifactInputV1 {
            producer,
            cone,
            direct_dependencies,
            hir_foundation,
            hir_production,
            mut hir_cross_cone,
            mir_foundation,
            mir_production,
            mir_cross_cone,
            lir_foundation,
            lir_cross_cone,
            cross_cone_code,
            link_objects,
        } = input;
        let identity = cone.identity();
        let (code, cross_cone_link_closure) = cross_cone_code.into_parts();
        validate_producers(
            identity,
            &code,
            mir_cross_cone,
            lir_foundation,
            lir_cross_cone,
            &cross_cone_link_closure,
        )?;
        verify_link_objects(&code, &link_objects)?;

        let semantic_imports = CrossConeLinkSemanticImportSetV1::from_lir_bridge(lir_cross_cone)
            .map_err(CrossConeStrongArtifactWriteError::SemanticImports)?;
        if &semantic_imports != cross_cone_link_closure.semantic_imports() {
            return Err(CrossConeStrongArtifactWriteError::SemanticImportMismatch);
        }
        if cross_cone_link_closure
            .object_coverage()
            .verified_link_objects()
            != code.production().link_objects().projection()
        {
            return Err(CrossConeStrongArtifactWriteError::LinkObjectProjectionMismatch);
        }

        let target_selection = code.undefined_symbols().selection();
        let compatibility = CompatibilityRecord::new(
            target_selection,
            ArtifactCapabilityProfile::CROSS_CONE_SEMANTICS_STRONG,
        )
        .map_err(CrossConeStrongArtifactWriteError::Compatibility)?;
        let link_identity_closure = LinkIdentityClosureSectionV1::from_verified_code(&code)
            .map_err(CrossConeStrongArtifactWriteError::LinkIdentityClosure)?;

        let hir_cross_cone = hir_cross_cone
            .index_for_wire()
            .map_err(CrossConeStrongArtifactWriteError::HirInterfaceIndex)?;
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
                build_section(
                    StrongArtifactSectionV1::HirCrossConeInterface,
                    MetadataLocation::Hir,
                    hir_cross_cone_interface_capability(),
                    MemberPurposeSet::COMPILE,
                    &hir_cross_cone,
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
                build_section(
                    StrongArtifactSectionV1::MirCrossConeBridge,
                    MetadataLocation::Mir,
                    mir_cross_cone_param_free_bridge_capability(),
                    MemberPurposeSet::COMPILE,
                    mir_cross_cone,
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
                    StrongArtifactSectionV1::LirCrossConeBridge,
                    MetadataLocation::Lir,
                    lir_cross_cone_param_free_bridge_capability(),
                    MemberPurposeSet::COMPILE,
                    lir_cross_cone,
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
                    &link_identity_closure,
                )?,
                build_section(
                    StrongArtifactSectionV1::CrossConeLinkClosure,
                    MetadataLocation::Lir,
                    lir_cross_cone_link_closure_capability(),
                    MemberPurposeSet::LINK,
                    &cross_cone_link_closure,
                )?,
            ],
        )?;

        let production_manifest = SingleConeProductionManifestV1::from_verified_code(code);
        let foundation_fingerprints = SemanticFingerprintRecord::from_metadata_sections(
            &compatibility,
            &direct_dependencies,
            &hir.sections,
            &mir.sections,
            &lir.sections,
        )
        .map_err(CrossConeStrongArtifactWriteError::SemanticFingerprints)?;
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
                CrossConeStrongArtifactWriteError::Encoding {
                    section: StrongArtifactSectionV1::ProductionManifest,
                    source,
                }
            })?,
        )
        .map_err(CrossConeStrongArtifactWriteError::ManifestSection)?;

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
        .map_err(CrossConeStrongArtifactWriteError::Manifest)?;
        let artifact_fingerprint = manifest.artifact_fingerprint();
        let archive = CanonicalSlibArchive::write_bootstrap(&manifest, members)
            .map_err(CrossConeStrongArtifactWriteError::Archive)?;
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

fn validate_producers(
    expected: ConeIdentity,
    code: &crate::VerifiedCodeFingerprintV1,
    mir_cross_cone: &CrossConeMirBridgeSectionV1,
    lir_foundation: &OdrFreeLirFoundation,
    lir_cross_cone: &CrossConeLirBridgeSectionV1,
    cross_cone_link_closure: &crate::CrossConeLinkClosureSectionV1,
) -> Result<(), CrossConeStrongArtifactWriteError> {
    if code.producer() != expected {
        return Err(CrossConeStrongArtifactWriteError::CodeProducerMismatch {
            expected,
            actual: code.producer(),
        });
    }
    if lir_foundation.producer() != expected {
        return Err(
            CrossConeStrongArtifactWriteError::LirFoundationProducerMismatch {
                expected,
                actual: lir_foundation.producer(),
            },
        );
    }
    if mir_cross_cone.artifact() != expected {
        return Err(
            CrossConeStrongArtifactWriteError::MirBridgeProducerMismatch {
                expected,
                actual: mir_cross_cone.artifact(),
            },
        );
    }
    if lir_cross_cone.artifact() != expected {
        return Err(
            CrossConeStrongArtifactWriteError::LirBridgeProducerMismatch {
                expected,
                actual: lir_cross_cone.artifact(),
            },
        );
    }
    if cross_cone_link_closure.consumer() != expected {
        return Err(
            CrossConeStrongArtifactWriteError::LinkClosureProducerMismatch {
                expected,
                actual: cross_cone_link_closure.consumer(),
            },
        );
    }
    Ok(())
}

#[derive(Debug)]
pub enum CrossConeStrongArtifactWriteError {
    CodeProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    LirFoundationProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    MirBridgeProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    LirBridgeProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    LinkClosureProducerMismatch {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    SemanticImports(CrossConeLinkSemanticImportBuildError),
    SemanticImportMismatch,
    LinkObjectProjectionMismatch,
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
    LinkIdentityClosure(LinkIdentityClosureBuildError),
    HirInterfaceIndex(CrossConeHirInterfaceIndexError),
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

impl From<StrongArtifactAssemblyError> for CrossConeStrongArtifactWriteError {
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

impl fmt::Display for CrossConeStrongArtifactWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot assemble cross-Cone strong artifact: {self:?}"
        )
    }
}

impl std::error::Error for CrossConeStrongArtifactWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SemanticImports(source) => Some(source),
            Self::LinkObjectFingerprint(source) | Self::Compatibility(source) => Some(source),
            Self::LinkIdentityClosure(source) => Some(source),
            Self::HirInterfaceIndex(source) => Some(source),
            Self::Encoding { source, .. } => Some(source),
            Self::MetadataSection { source, .. } => Some(source),
            Self::MetadataEnvelope { source, .. } => Some(source),
            Self::MetadataEnvelopeEncoding { source, .. } => Some(source),
            Self::MetadataMember { source, .. } => Some(source),
            Self::SemanticFingerprints(source) => Some(source),
            Self::ManifestSection(source) => Some(source),
            Self::Manifest(source) => Some(source),
            Self::Archive(source) => Some(source),
            Self::CodeProducerMismatch { .. }
            | Self::LirFoundationProducerMismatch { .. }
            | Self::MirBridgeProducerMismatch { .. }
            | Self::LirBridgeProducerMismatch { .. }
            | Self::LinkClosureProducerMismatch { .. }
            | Self::SemanticImportMismatch
            | Self::LinkObjectProjectionMismatch
            | Self::LinkObjectCount { .. }
            | Self::InvalidLinkObjectRole { .. }
            | Self::LinkObjectMismatch { .. } => None,
        }
    }
}
