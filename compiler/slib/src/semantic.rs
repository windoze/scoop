use std::fmt;

use scoop_identity::{CapabilityId, ConeIdentity};
use scoop_wire::{Encoder, HashError, WireEncode, WireError, domain_separated_cbor_hash};

use crate::{
    CapabilityContractRegistry, CompatibilityRecord, DecodedMetadataSection, DependencyRecord,
    FingerprintSink, HirFingerprint, LirFingerprint, MemberPurposeSet, MetadataLocation,
    MetadataSection, MirFingerprint, SemanticFingerprintRecord,
};

mod encoding;
use encoding::{
    CanonicalSemanticContribution, LayerFingerprintContext, LayerFingerprintInput, SupportEdges,
};

const HIR_SEMANTIC_DOMAIN: &str = "scoop-hir-semantic-v1";
const MIR_SEMANTIC_DOMAIN: &str = "scoop-mir-semantic-v1";
const LIR_SEMANTIC_DOMAIN: &str = "scoop-lir-semantic-v2";

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

#[cfg(test)]
mod tests;
