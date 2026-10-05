use std::fmt;

use scoop_identity::{CapabilityId, ConeIdentity};
use scoop_wire::{Encoder, HashError, WireEncode, WireError, domain_separated_cbor_hash};

use crate::{
    CapabilityContractRegistry, CompatibilityRecord, DecodedMetadataSection, DependencyRecord,
    FingerprintSink, HirFingerprint, LirFingerprint, MemberPurposeSet, MetadataLocation,
    MetadataSection, MirFingerprint, SemanticFingerprintRecord,
};

const HIR_SEMANTIC_DOMAIN: &str = "scoop-hir-semantic-v1";
const MIR_SEMANTIC_DOMAIN: &str = "scoop-mir-semantic-v1";
const LIR_SEMANTIC_DOMAIN: &str = "scoop-lir-semantic-v1";

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SupportEdgeRoleId(CapabilityId);

impl SupportEdgeRoleId {
    pub fn all_direct() -> Self {
        Self(
            CapabilityId::new("org.scoop-lang.support-edge", "all-direct", 1)
                .expect("built-in support edge role is valid"),
        )
    }

    pub const fn capability(&self) -> &CapabilityId {
        &self.0
    }
}

impl WireEncode for SupportEdgeRoleId {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        self.0.encode(encoder)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FoundationLayer {
    Hir,
    Mir,
    Lir,
}

impl fmt::Display for FoundationLayer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Hir => "HIR",
            Self::Mir => "MIR",
            Self::Lir => "LIR",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticFingerprintError {
    WrongSectionLocation {
        layer: FoundationLayer,
        index: usize,
        actual: MetadataLocation,
    },
    UnsupportedRequiredCapability {
        layer: FoundationLayer,
        index: usize,
        capability: CapabilityId,
    },
    DuplicateContribution {
        layer: FoundationLayer,
        capability: CapabilityId,
    },
    ContributionTableAllocation {
        layer: FoundationLayer,
        requested_slots: usize,
    },
    DuplicateDependency {
        identity: ConeIdentity,
    },
    DependencyTableAllocation {
        requested_slots: usize,
    },
    Hash(HashError),
    Resource(WireError),
}

impl fmt::Display for SemanticFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongSectionLocation {
                layer,
                index,
                actual,
            } => write!(
                formatter,
                "{layer} semantic section {index} belongs to {actual}"
            ),
            Self::UnsupportedRequiredCapability {
                layer,
                index,
                capability,
            } => write!(
                formatter,
                "{layer} semantic section {index} requires unsupported capability {}/{}/{}",
                capability.namespace(),
                capability.name(),
                capability.major_version()
            ),
            Self::DuplicateContribution { layer, capability } => write!(
                formatter,
                "duplicate {layer} semantic contribution {}/{}/{}",
                capability.namespace(),
                capability.name(),
                capability.major_version()
            ),
            Self::ContributionTableAllocation {
                layer,
                requested_slots,
            } => write!(
                formatter,
                "failed to allocate {layer} semantic contribution table with {requested_slots} slots"
            ),
            Self::DuplicateDependency { identity } => {
                write!(formatter, "duplicate direct dependency {identity}")
            }
            Self::DependencyTableAllocation { requested_slots } => write!(
                formatter,
                "failed to allocate semantic fingerprint dependency table with {requested_slots} slots"
            ),
            Self::Hash(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticFingerprintError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash(error) => Some(error),
            Self::Resource(error) => Some(error),
            Self::WrongSectionLocation { .. }
            | Self::UnsupportedRequiredCapability { .. }
            | Self::DuplicateContribution { .. }
            | Self::ContributionTableAllocation { .. }
            | Self::DuplicateDependency { .. }
            | Self::DependencyTableAllocation { .. } => None,
        }
    }
}

impl SemanticFingerprintRecord {
    pub fn from_metadata_sections(
        compatibility: &CompatibilityRecord,
        direct_dependencies: &[DependencyRecord],
        hir: &[MetadataSection],
        mir: &[MetadataSection],
        lir: &[MetadataSection],
    ) -> Result<Self, SemanticFingerprintError> {
        let dependencies = OrderedDependencies::sorted(direct_dependencies)?;
        metadata_fingerprints(compatibility, hir, mir, lir, &dependencies, None)
    }

    pub(crate) fn from_decoded_compile_metadata_sections(
        compatibility: &CompatibilityRecord,
        direct_dependencies: &[DependencyRecord],
        hir: &[DecodedMetadataSection<'_>],
        mir: &[DecodedMetadataSection<'_>],
        lir: &[DecodedMetadataSection<'_>],
    ) -> Result<Self, SemanticFingerprintError> {
        let dependencies = OrderedDependencies::canonical(direct_dependencies);
        metadata_fingerprints(
            compatibility,
            hir,
            mir,
            lir,
            &dependencies,
            Some(MemberPurposeSet::COMPILE),
        )
    }
}

fn metadata_fingerprints<S: SemanticSection>(
    compatibility: &CompatibilityRecord,
    hir: &[S],
    mir: &[S],
    lir: &[S],
    dependencies: &OrderedDependencies<'_>,

    required_view: Option<MemberPurposeSet>,
) -> Result<SemanticFingerprintRecord, SemanticFingerprintError> {
    let hir = calculate_layer_fingerprint(
        FoundationLayer::Hir,
        compatibility,
        hir,
        dependencies,
        required_view,
    )?;
    let mir = calculate_layer_fingerprint(
        FoundationLayer::Mir,
        compatibility,
        mir,
        dependencies,
        required_view,
    )?;
    let lir = calculate_layer_fingerprint(
        FoundationLayer::Lir,
        compatibility,
        lir,
        dependencies,
        required_view,
    )?;
    Ok(SemanticFingerprintRecord::from_foundation_digests(
        HirFingerprint::from_array(*hir.as_array()),
        MirFingerprint::from_array(*mir.as_array()),
        LirFingerprint::from_array(*lir.as_array()),
    ))
}

trait SemanticSection {
    fn location(&self) -> MetadataLocation;
    fn capability(&self) -> &CapabilityId;
    fn required_for(&self) -> MemberPurposeSet;
    fn payload(&self) -> &[u8];
}

impl SemanticSection for MetadataSection {
    fn location(&self) -> MetadataLocation {
        self.location()
    }

    fn capability(&self) -> &CapabilityId {
        self.capability()
    }

    fn required_for(&self) -> MemberPurposeSet {
        self.required_for()
    }

    fn payload(&self) -> &[u8] {
        self.payload()
    }
}

impl SemanticSection for DecodedMetadataSection<'_> {
    fn location(&self) -> MetadataLocation {
        self.location()
    }

    fn capability(&self) -> &CapabilityId {
        self.capability()
    }

    fn required_for(&self) -> MemberPurposeSet {
        self.required_for()
    }

    fn payload(&self) -> &[u8] {
        self.payload()
    }
}

fn calculate_layer_fingerprint<S: SemanticSection>(
    layer: FoundationLayer,
    compatibility: &CompatibilityRecord,
    sections: &[S],
    dependencies: &OrderedDependencies<'_>,

    required_view: Option<MemberPurposeSet>,
) -> Result<scoop_wire::Digest256, SemanticFingerprintError> {
    let contributions = collect_contributions(layer, sections, required_view)?;
    let input = LayerFingerprintInput {
        context: LayerFingerprintContext {
            layer,
            compatibility,
        },
        contributions,
        support_edges: SupportEdges {
            layer,
            dependencies,
        },
    };

    domain_separated_cbor_hash(layer.domain(), &input).map_err(SemanticFingerprintError::Hash)
}

fn collect_contributions<'section, S: SemanticSection>(
    layer: FoundationLayer,
    sections: &'section [S],

    required_view: Option<MemberPurposeSet>,
) -> Result<Vec<CanonicalSemanticContribution<'section>>, SemanticFingerprintError> {
    let mut contributions = Vec::new();
    contributions
        .try_reserve_exact(sections.len())
        .map_err(|_| SemanticFingerprintError::ContributionTableAllocation {
            layer,
            requested_slots: sections.len(),
        })?;
    for (index, section) in sections.iter().enumerate() {
        if section.location() != layer.location() {
            return Err(SemanticFingerprintError::WrongSectionLocation {
                layer,
                index,
                actual: section.location(),
            });
        }
        let Some(contract) = CapabilityContractRegistry::contract(section.capability()) else {
            if required_view.map_or(
                section.required_for() != MemberPurposeSet::NONE,
                |purpose| section.required_for().contains(purpose),
            ) {
                return Err(SemanticFingerprintError::UnsupportedRequiredCapability {
                    layer,
                    index,
                    capability: section.capability().clone(),
                });
            }
            continue;
        };
        if contract.sinks().contains(layer.sink()) {
            contributions.push(CanonicalSemanticContribution {
                location: section.location(),
                capability: section.capability(),
                sink: layer.sink(),
                projection: section.payload(),
            });
        }
    }
    contributions.sort_unstable_by(|left, right| {
        left.location
            .cmp(&right.location)
            .then_with(|| left.capability.cmp(right.capability))
            .then_with(|| left.sink.cmp(&right.sink))
    });
    if let Some(pair) = contributions.windows(2).find(|pair| {
        pair[0].location == pair[1].location
            && pair[0].capability == pair[1].capability
            && pair[0].sink == pair[1].sink
    }) {
        return Err(SemanticFingerprintError::DuplicateContribution {
            layer,
            capability: pair[0].capability.clone(),
        });
    }
    Ok(contributions)
}

impl FoundationLayer {
    const fn location(self) -> MetadataLocation {
        match self {
            Self::Hir => MetadataLocation::Hir,
            Self::Mir => MetadataLocation::Mir,
            Self::Lir => MetadataLocation::Lir,
        }
    }

    const fn sink(self) -> FingerprintSink {
        match self {
            Self::Hir => FingerprintSink::Hir,
            Self::Mir => FingerprintSink::Mir,
            Self::Lir => FingerprintSink::Lir,
        }
    }

    const fn domain(self) -> &'static str {
        match self {
            Self::Hir => HIR_SEMANTIC_DOMAIN,
            Self::Mir => MIR_SEMANTIC_DOMAIN,
            Self::Lir => LIR_SEMANTIC_DOMAIN,
        }
    }
}

enum DependencyEntries<'dependency> {
    Canonical(&'dependency [DependencyRecord]),
    Sorted(Vec<&'dependency DependencyRecord>),
}

struct OrderedDependencies<'dependency> {
    entries: DependencyEntries<'dependency>,
}

impl<'dependency> OrderedDependencies<'dependency> {
    fn sorted(
        dependencies: &'dependency [DependencyRecord],
    ) -> Result<Self, SemanticFingerprintError> {
        let mut entries = Vec::new();
        entries.try_reserve_exact(dependencies.len()).map_err(|_| {
            SemanticFingerprintError::DependencyTableAllocation {
                requested_slots: dependencies.len(),
            }
        })?;
        entries.extend(dependencies);
        entries.sort_unstable_by_key(|dependency| dependency.identity());
        if let Some(pair) = entries
            .windows(2)
            .find(|pair| pair[0].identity() == pair[1].identity())
        {
            return Err(SemanticFingerprintError::DuplicateDependency {
                identity: pair[0].identity(),
            });
        }
        Ok(Self {
            entries: DependencyEntries::Sorted(entries),
        })
    }

    const fn canonical(dependencies: &'dependency [DependencyRecord]) -> Self {
        Self {
            entries: DependencyEntries::Canonical(dependencies),
        }
    }

    fn len(&self) -> usize {
        match &self.entries {
            DependencyEntries::Canonical(entries) => entries.len(),
            DependencyEntries::Sorted(entries) => entries.len(),
        }
    }
}

struct CanonicalSemanticContribution<'section> {
    location: MetadataLocation,
    capability: &'section CapabilityId,
    sink: FingerprintSink,
    projection: &'section [u8],
}

impl WireEncode for CanonicalSemanticContribution<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        encoder.unsigned(match self.location {
            MetadataLocation::Hir => 2,
            MetadataLocation::Mir => 3,
            MetadataLocation::Lir => 4,
        })?;
        encoder.field(2)?;
        self.capability.encode(encoder)?;
        encoder.field(3)?;
        self.sink.encode(encoder)?;
        encoder.field(4)?;
        encoder.bytes(self.projection)
    }
}

struct LayerFingerprintContext<'compatibility> {
    layer: FoundationLayer,
    compatibility: &'compatibility CompatibilityRecord,
}

impl WireEncode for LayerFingerprintContext<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self.layer {
            FoundationLayer::Hir => {
                encoder.map(2)?;
                encode_layer_tag(encoder, 1)?;
                encoder.field(1)?;
                self.compatibility.composite_identity_abi().encode(encoder)
            }
            FoundationLayer::Mir => {
                encoder.map(3)?;
                encode_layer_tag(encoder, 2)?;
                encoder.field(1)?;
                self.compatibility
                    .composite_identity_abi()
                    .encode(encoder)?;
                encoder.field(2)?;
                self.compatibility.mangling_schema().encode(encoder)
            }
            FoundationLayer::Lir => {
                encoder.map(6)?;
                encode_layer_tag(encoder, 3)?;
                encoder.field(1)?;
                self.compatibility
                    .composite_identity_abi()
                    .encode(encoder)?;
                encoder.field(2)?;
                self.compatibility.target_profile().encode(encoder)?;
                encoder.field(3)?;
                self.compatibility.target_fingerprint().encode(encoder)?;
                encoder.field(4)?;
                self.compatibility.backend_profile().encode(encoder)?;
                encoder.field(5)?;
                self.compatibility.backend_fingerprint().encode(encoder)
            }
        }
    }
}

fn encode_layer_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

struct SupportEdges<'dependency> {
    layer: FoundationLayer,
    dependencies: &'dependency OrderedDependencies<'dependency>,
}

impl WireEncode for SupportEdges<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.dependencies.len() as u64)?;
        let role = SupportEdgeRoleId::all_direct();
        match &self.dependencies.entries {
            DependencyEntries::Canonical(entries) => {
                for dependency in *entries {
                    encode_support_edge(encoder, self.layer, &role, dependency)?;
                }
            }
            DependencyEntries::Sorted(entries) => {
                for dependency in entries {
                    encode_support_edge(encoder, self.layer, &role, dependency)?;
                }
            }
        }
        Ok(())
    }
}

fn encode_support_edge(
    encoder: &mut Encoder,
    layer: FoundationLayer,
    role: &SupportEdgeRoleId,
    dependency: &DependencyRecord,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encoder.field(1)?;
    dependency.identity().encode(encoder)?;
    encoder.field(2)?;
    role.encode(encoder)?;
    encoder.field(3)?;
    match layer {
        FoundationLayer::Hir => dependency.hir_fingerprint().encode(encoder),
        FoundationLayer::Mir => dependency.mir_fingerprint().encode(encoder),
        FoundationLayer::Lir => dependency.lir_fingerprint().encode(encoder),
    }
}

struct LayerFingerprintInput<'input> {
    context: LayerFingerprintContext<'input>,
    contributions: Vec<CanonicalSemanticContribution<'input>>,
    support_edges: SupportEdges<'input>,
}

impl WireEncode for LayerFingerprintInput<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.context.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(self.contributions.len() as u64)?;
        for contribution in &self.contributions {
            contribution.encode(encoder)?;
        }
        encoder.field(3)?;
        self.support_edges.encode(encoder)
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::ConeCoordinate;
    use scoop_lir::ValidatedLirTargetSelection;

    use super::*;
    use crate::{
        MemberPurposeSet, MetadataSection, hir_identity_foundation_capability,
        lir_identity_foundation_capability, mir_identity_foundation_capability,
    };

    fn compatibility() -> CompatibilityRecord {
        CompatibilityRecord::new(
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
            crate::ArtifactCapabilityProfile::SINGLE_CONE_STRONG,
        )
        .unwrap()
    }

    fn section(layer: FoundationLayer, payload: &[u8]) -> MetadataSection {
        let capability = match layer {
            FoundationLayer::Hir => hir_identity_foundation_capability(),
            FoundationLayer::Mir => mir_identity_foundation_capability(),
            FoundationLayer::Lir => lir_identity_foundation_capability(),
        };
        MetadataSection::new(
            layer.location(),
            capability,
            MemberPurposeSet::COMPILE_AND_LINK,
            payload.to_vec(),
        )
        .unwrap()
    }

    fn dependency(name: &str, hir: u8, mir: u8, lir: u8) -> DependencyRecord {
        DependencyRecord::new(
            ConeCoordinate::new("example", name, "0.1.0").unwrap(),
            HirFingerprint::from_array([hir; 32]),
            MirFingerprint::from_array([mir; 32]),
            LirFingerprint::from_array([lir; 32]),
        )
        .unwrap()
    }

    fn semantic_fingerprints(
        compatibility: &CompatibilityRecord,
        dependencies: &[DependencyRecord],
        hir: &MetadataSection,
        mir: &MetadataSection,
        lir: &MetadataSection,
    ) -> Result<SemanticFingerprintRecord, SemanticFingerprintError> {
        SemanticFingerprintRecord::from_metadata_sections(
            compatibility,
            dependencies,
            std::slice::from_ref(hir),
            std::slice::from_ref(mir),
            std::slice::from_ref(lir),
        )
    }

    #[test]
    fn foundation_fingerprints_have_fixed_vectors() {
        let fingerprints = semantic_fingerprints(
            &compatibility(),
            &[],
            &section(FoundationLayer::Hir, b"hir"),
            &section(FoundationLayer::Mir, b"mir"),
            &section(FoundationLayer::Lir, b"lir"),
        )
        .unwrap();

        assert_eq!(
            [
                fingerprints.hir().to_string(),
                fingerprints.mir().to_string(),
                fingerprints.lir().to_string()
            ],
            [
                "ae8d51e29eb7d1eb8c0214d22e569162d962eee09633c30744e4c893b3d1faca",
                "0285031303f92624ec48efe37a9322cdd1604e5c19e2aab3874b8e598661c223",
                "b841376860f5aaa98ee7675e75468a9111a6d431102614114962297bbc0711c8",
            ]
        );
    }

    #[test]
    fn payload_and_dependency_fingerprints_affect_only_their_layer() {
        let original_dependency = dependency("left", 1, 2, 3);
        let changed_dependency = dependency("left", 9, 2, 3);
        let hir = section(FoundationLayer::Hir, b"hir");
        let mir = section(FoundationLayer::Mir, b"mir");
        let lir = section(FoundationLayer::Lir, b"lir");
        let original = semantic_fingerprints(
            &compatibility(),
            std::slice::from_ref(&original_dependency),
            &hir,
            &mir,
            &lir,
        )
        .unwrap();
        let dependency_changed = semantic_fingerprints(
            &compatibility(),
            std::slice::from_ref(&changed_dependency),
            &hir,
            &mir,
            &lir,
        )
        .unwrap();
        let payload_changed = semantic_fingerprints(
            &compatibility(),
            std::slice::from_ref(&original_dependency),
            &section(FoundationLayer::Hir, b"changed"),
            &mir,
            &lir,
        )
        .unwrap();

        assert_ne!(original.hir(), dependency_changed.hir());
        assert_eq!(original.mir(), dependency_changed.mir());
        assert_eq!(original.lir(), dependency_changed.lir());
        assert_ne!(original.hir(), payload_changed.hir());
        assert_eq!(original.mir(), payload_changed.mir());
        assert_eq!(original.lir(), payload_changed.lir());
    }

    #[test]
    fn multiple_registry_contributions_are_canonical_and_layer_local() {
        let foundation = section(FoundationLayer::Hir, b"foundation");
        let production = MetadataSection::new(
            MetadataLocation::Hir,
            crate::hir_core_bootstrap_interface_capability(),
            MemberPurposeSet::COMPILE,
            b"production".to_vec(),
        )
        .unwrap();
        let changed_production = MetadataSection::new(
            MetadataLocation::Hir,
            crate::hir_core_bootstrap_interface_capability(),
            MemberPurposeSet::COMPILE,
            b"changed-production".to_vec(),
        )
        .unwrap();
        let mir = section(FoundationLayer::Mir, b"mir");
        let lir = section(FoundationLayer::Lir, b"lir");

        let forward = SemanticFingerprintRecord::from_metadata_sections(
            &compatibility(),
            &[],
            &[foundation.clone(), production.clone()],
            std::slice::from_ref(&mir),
            std::slice::from_ref(&lir),
        )
        .unwrap();
        let reverse = SemanticFingerprintRecord::from_metadata_sections(
            &compatibility(),
            &[],
            &[production, foundation.clone()],
            std::slice::from_ref(&mir),
            std::slice::from_ref(&lir),
        )
        .unwrap();
        let changed = SemanticFingerprintRecord::from_metadata_sections(
            &compatibility(),
            &[],
            &[foundation.clone(), changed_production],
            std::slice::from_ref(&mir),
            std::slice::from_ref(&lir),
        )
        .unwrap();

        assert_eq!(forward, reverse);
        assert_ne!(forward.hir(), changed.hir());
        assert_eq!(forward.mir(), changed.mir());
        assert_eq!(forward.lir(), changed.lir());
        assert!(matches!(
            SemanticFingerprintRecord::from_metadata_sections(
                &compatibility(),
                &[],
                &[foundation.clone(), foundation],
                std::slice::from_ref(&mir),
                std::slice::from_ref(&lir),
            ),
            Err(SemanticFingerprintError::DuplicateContribution {
                layer: FoundationLayer::Hir,
                ..
            })
        ));
    }

    #[test]
    fn optional_unknown_sections_are_envelope_only() {
        let hir = section(FoundationLayer::Hir, b"hir");
        let optional = MetadataSection::new(
            MetadataLocation::Hir,
            CapabilityId::new("org.scoop-lang.test", "optional", 1).unwrap(),
            MemberPurposeSet::NONE,
            b"ignored".to_vec(),
        )
        .unwrap();
        let mir = section(FoundationLayer::Mir, b"mir");
        let lir = section(FoundationLayer::Lir, b"lir");
        let baseline = semantic_fingerprints(&compatibility(), &[], &hir, &mir, &lir).unwrap();
        let with_optional = SemanticFingerprintRecord::from_metadata_sections(
            &compatibility(),
            &[],
            &[hir, optional],
            std::slice::from_ref(&mir),
            std::slice::from_ref(&lir),
        )
        .unwrap();

        assert_eq!(with_optional, baseline);
    }

    #[test]
    fn dependency_order_is_canonical_and_duplicates_are_rejected() {
        let left = dependency("left", 1, 2, 3);
        let right = dependency("right", 4, 5, 6);
        let hir = section(FoundationLayer::Hir, b"hir");
        let mir = section(FoundationLayer::Mir, b"mir");
        let lir = section(FoundationLayer::Lir, b"lir");
        let forward = semantic_fingerprints(
            &compatibility(),
            &[left.clone(), right.clone()],
            &hir,
            &mir,
            &lir,
        )
        .unwrap();
        let reverse =
            semantic_fingerprints(&compatibility(), &[right, left.clone()], &hir, &mir, &lir)
                .unwrap();

        assert_eq!(forward, reverse);
        assert!(matches!(
            semantic_fingerprints(&compatibility(), &[left.clone(), left], &hir, &mir, &lir,),
            Err(SemanticFingerprintError::DuplicateDependency { .. })
        ));
    }

    #[test]
    fn foundation_calculator_rejects_a_section_from_another_layer() {
        let hir = section(FoundationLayer::Hir, b"hir");
        let mir = section(FoundationLayer::Mir, b"mir");
        assert_eq!(
            semantic_fingerprints(&compatibility(), &[], &hir, &mir, &mir,),
            Err(SemanticFingerprintError::WrongSectionLocation {
                layer: FoundationLayer::Lir,
                index: 0,
                actual: MetadataLocation::Mir,
            })
        );
    }
}
