use std::fmt;

use scoop_identity::{CapabilityId, ConeIdentity};
use scoop_wire::{Encoder, HashError, WireEncode, domain_separated_cbor_hash};

use crate::{
    CompatibilityRecord, DependencyRecord, HirFingerprint, LirFingerprint, MetadataLocation,
    MetadataSection, MirFingerprint, SemanticFingerprintRecord, hir_identity_foundation_capability,
    lir_identity_foundation_capability, mir_identity_foundation_capability,
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
    WrongFoundationSection { layer: FoundationLayer },
    DuplicateDependency { identity: ConeIdentity },
    DependencyTableAllocation { requested_slots: usize },
    Hash(HashError),
}

impl fmt::Display for SemanticFingerprintError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongFoundationSection { layer } => {
                write!(
                    formatter,
                    "expected the {layer} identity-foundation section"
                )
            }
            Self::DuplicateDependency { identity } => {
                write!(formatter, "duplicate direct dependency {identity}")
            }
            Self::DependencyTableAllocation { requested_slots } => write!(
                formatter,
                "failed to allocate semantic fingerprint dependency table with {requested_slots} slots"
            ),
            Self::Hash(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for SemanticFingerprintError {}

impl SemanticFingerprintRecord {
    pub fn identity_foundation(
        compatibility: &CompatibilityRecord,
        direct_dependencies: &[DependencyRecord],
        hir: &MetadataSection,
        mir: &MetadataSection,
        lir: &MetadataSection,
    ) -> Result<Self, SemanticFingerprintError> {
        validate_foundation_section(FoundationLayer::Hir, hir)?;
        validate_foundation_section(FoundationLayer::Mir, mir)?;
        validate_foundation_section(FoundationLayer::Lir, lir)?;
        let dependencies = SortedDependencies::new(direct_dependencies)?;

        let hir =
            calculate_layer_fingerprint(FoundationLayer::Hir, compatibility, hir, &dependencies)?;
        let mir =
            calculate_layer_fingerprint(FoundationLayer::Mir, compatibility, mir, &dependencies)?;
        let lir =
            calculate_layer_fingerprint(FoundationLayer::Lir, compatibility, lir, &dependencies)?;
        Ok(Self::from_foundation_digests(
            HirFingerprint::from_array(*hir.as_array()),
            MirFingerprint::from_array(*mir.as_array()),
            LirFingerprint::from_array(*lir.as_array()),
        ))
    }
}

fn validate_foundation_section(
    layer: FoundationLayer,
    section: &MetadataSection,
) -> Result<(), SemanticFingerprintError> {
    let expected = match layer {
        FoundationLayer::Hir => hir_identity_foundation_capability(),
        FoundationLayer::Mir => mir_identity_foundation_capability(),
        FoundationLayer::Lir => lir_identity_foundation_capability(),
    };
    if section.location() != layer.location() || section.capability() != &expected {
        return Err(SemanticFingerprintError::WrongFoundationSection { layer });
    }
    Ok(())
}

fn calculate_layer_fingerprint(
    layer: FoundationLayer,
    compatibility: &CompatibilityRecord,
    section: &MetadataSection,
    dependencies: &SortedDependencies<'_>,
) -> Result<scoop_wire::Digest256, SemanticFingerprintError> {
    let input = LayerFingerprintInput {
        context: LayerFingerprintContext {
            layer,
            compatibility,
        },
        contribution: CanonicalSemanticContribution {
            location: section.location(),
            capability: section.capability(),
            sink: layer.sink(),
            projection: section.payload(),
        },
        support_edges: SupportEdges {
            layer,
            dependencies,
        },
    };
    domain_separated_cbor_hash(layer.domain(), &input).map_err(SemanticFingerprintError::Hash)
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

struct SortedDependencies<'dependency> {
    entries: Vec<&'dependency DependencyRecord>,
}

impl<'dependency> SortedDependencies<'dependency> {
    fn new(
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
        Ok(Self { entries })
    }
}

#[derive(Clone, Copy)]
enum FingerprintSink {
    Hir,
    Mir,
    Lir,
}

impl WireEncode for FingerprintSink {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Hir => 1,
            Self::Mir => 2,
            Self::Lir => 3,
        })
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
    dependencies: &'dependency SortedDependencies<'dependency>,
}

impl WireEncode for SupportEdges<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.dependencies.entries.len() as u64)?;
        let role = SupportEdgeRoleId::all_direct();
        for dependency in &self.dependencies.entries {
            encoder.map(3)?;
            encoder.field(1)?;
            dependency.identity().encode(encoder)?;
            encoder.field(2)?;
            role.encode(encoder)?;
            encoder.field(3)?;
            match self.layer {
                FoundationLayer::Hir => dependency.hir_fingerprint().encode(encoder)?,
                FoundationLayer::Mir => dependency.mir_fingerprint().encode(encoder)?,
                FoundationLayer::Lir => dependency.lir_fingerprint().encode(encoder)?,
            }
        }
        Ok(())
    }
}

struct LayerFingerprintInput<'input> {
    context: LayerFingerprintContext<'input>,
    contribution: CanonicalSemanticContribution<'input>,
    support_edges: SupportEdges<'input>,
}

impl WireEncode for LayerFingerprintInput<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.context.encode(encoder)?;
        encoder.field(2)?;
        encoder.array(1)?;
        self.contribution.encode(encoder)?;
        encoder.field(3)?;
        self.support_edges.encode(encoder)
    }
}

#[cfg(test)]
mod tests {
    use scoop_identity::ConeCoordinate;
    use scoop_lir::ValidatedLirTargetSelection;

    use super::*;
    use crate::{MemberPurposeSet, MetadataSection};

    fn compatibility() -> CompatibilityRecord {
        CompatibilityRecord::identity_foundation(
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
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
            MemberPurposeSet::COMPILE,
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

    #[test]
    fn foundation_fingerprints_have_fixed_vectors() {
        let fingerprints = SemanticFingerprintRecord::identity_foundation(
            &compatibility(),
            &[],
            &section(FoundationLayer::Hir, b"hir"),
            &section(FoundationLayer::Mir, b"mir"),
            &section(FoundationLayer::Lir, b"lir"),
        )
        .unwrap();

        assert_eq!(
            fingerprints.hir().to_string(),
            "1a7cb61cd83bc5d62f88280d22dd3d95b903c6cdad82377a9613dcc6df8872f9"
        );
        assert_eq!(
            fingerprints.mir().to_string(),
            "56bb9466f7112a21ff18d52257b67ae2946e160ea20984f22debcd5d4ca51857"
        );
        assert_eq!(
            fingerprints.lir().to_string(),
            "1146795f5298d4df3d1e887e8b181e2d9a1f6314ef96c560d88826cd76fc18dd"
        );
    }

    #[test]
    fn payload_and_dependency_fingerprints_affect_only_their_layer() {
        let original_dependency = dependency("left", 1, 2, 3);
        let changed_dependency = dependency("left", 9, 2, 3);
        let hir = section(FoundationLayer::Hir, b"hir");
        let mir = section(FoundationLayer::Mir, b"mir");
        let lir = section(FoundationLayer::Lir, b"lir");
        let original = SemanticFingerprintRecord::identity_foundation(
            &compatibility(),
            std::slice::from_ref(&original_dependency),
            &hir,
            &mir,
            &lir,
        )
        .unwrap();
        let dependency_changed = SemanticFingerprintRecord::identity_foundation(
            &compatibility(),
            std::slice::from_ref(&changed_dependency),
            &hir,
            &mir,
            &lir,
        )
        .unwrap();
        let payload_changed = SemanticFingerprintRecord::identity_foundation(
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
    fn dependency_order_is_canonical_and_duplicates_are_rejected() {
        let left = dependency("left", 1, 2, 3);
        let right = dependency("right", 4, 5, 6);
        let hir = section(FoundationLayer::Hir, b"hir");
        let mir = section(FoundationLayer::Mir, b"mir");
        let lir = section(FoundationLayer::Lir, b"lir");
        let forward = SemanticFingerprintRecord::identity_foundation(
            &compatibility(),
            &[left.clone(), right.clone()],
            &hir,
            &mir,
            &lir,
        )
        .unwrap();
        let reverse = SemanticFingerprintRecord::identity_foundation(
            &compatibility(),
            &[right, left.clone()],
            &hir,
            &mir,
            &lir,
        )
        .unwrap();

        assert_eq!(forward, reverse);
        assert!(matches!(
            SemanticFingerprintRecord::identity_foundation(
                &compatibility(),
                &[left.clone(), left],
                &hir,
                &mir,
                &lir,
            ),
            Err(SemanticFingerprintError::DuplicateDependency { .. })
        ));
    }

    #[test]
    fn foundation_calculator_rejects_a_section_from_another_layer() {
        let hir = section(FoundationLayer::Hir, b"hir");
        let mir = section(FoundationLayer::Mir, b"mir");
        assert_eq!(
            SemanticFingerprintRecord::identity_foundation(&compatibility(), &[], &hir, &mir, &mir,),
            Err(SemanticFingerprintError::WrongFoundationSection {
                layer: FoundationLayer::Lir,
            })
        );
    }
}
