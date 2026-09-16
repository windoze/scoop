//! Build-wide resource meter shared by graph discovery and artifact views.

use std::collections::HashSet;
use std::fmt;

use scoop_wire::{DecodeUsage, Digest256};

use crate::ArtifactFingerprint;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlibClosureDecodeLimitValuesV1 {
    pub cone_nodes: u64,
    pub dependency_edges: u64,
    pub graph_depth: u64,
    pub artifact_search_roots: u64,
    pub locator_candidates: u64,
    pub source_files: u64,
    pub source_bytes: u64,
    pub artifact_snapshot_bytes: u64,
    pub archive_members: u64,
    pub directory_carrier_bytes: u64,
    pub logical_heap_bytes: u64,
    pub decoded_nodes: u64,
    pub decoded_edges: u64,
    pub owned_bytes: u64,
    pub validation_work_units: u64,
    pub child_requests: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlibClosureDecodeLimitsV1(SlibClosureDecodeLimitValuesV1);

impl SlibClosureDecodeLimitsV1 {
    pub const M23_DEFAULT: Self = Self(SlibClosureDecodeLimitValuesV1 {
        cone_nodes: 4_096,
        dependency_edges: 65_536,
        graph_depth: 1_024,
        artifact_search_roots: 256,
        locator_candidates: 65_536,
        source_files: 1_048_576,
        source_bytes: 4_294_967_296,
        artifact_snapshot_bytes: 17_179_869_184,
        archive_members: 1_048_576,
        directory_carrier_bytes: 536_870_912,
        logical_heap_bytes: 8_589_934_592,
        decoded_nodes: 67_108_864,
        decoded_edges: 268_435_456,
        owned_bytes: 4_294_967_296,
        validation_work_units: 1_073_741_824,
        child_requests: 4_096,
    });

    pub fn new(
        values: SlibClosureDecodeLimitValuesV1,
    ) -> Result<Self, SlibClosureDecodeLimitsError> {
        for (resource, limit) in resources(values) {
            if limit == 0 {
                return Err(SlibClosureDecodeLimitsError::ZeroLimit { resource });
            }
        }
        Ok(Self(values))
    }

    pub const fn values(self) -> SlibClosureDecodeLimitValuesV1 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SlibClosureResourceKindV1 {
    ConeNodes,
    DependencyEdges,
    GraphDepth,
    ArtifactSearchRoots,
    LocatorCandidates,
    SourceFiles,
    SourceBytes,
    ArtifactSnapshotBytes,
    ArchiveMembers,
    DirectoryCarrierBytes,
    LogicalHeapBytes,
    DecodedNodes,
    DecodedEdges,
    OwnedBytes,
    ValidationWorkUnits,
    ChildRequests,
}

impl fmt::Display for SlibClosureResourceKindV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::ConeNodes => "Cone nodes",
            Self::DependencyEdges => "dependency edges",
            Self::GraphDepth => "graph depth",
            Self::ArtifactSearchRoots => "artifact search roots",
            Self::LocatorCandidates => "locator candidates",
            Self::SourceFiles => "source files",
            Self::SourceBytes => "source bytes",
            Self::ArtifactSnapshotBytes => "unique artifact snapshot bytes",
            Self::ArchiveMembers => "archive members",
            Self::DirectoryCarrierBytes => "manifest/member directory carrier bytes",
            Self::LogicalHeapBytes => "decoded logical heap bytes",
            Self::DecodedNodes => "decoded nodes",
            Self::DecodedEdges => "decoded edges",
            Self::OwnedBytes => "owned/copied decode bytes",
            Self::ValidationWorkUnits => "validation work units",
            Self::ChildRequests => "child requests",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SlibClosureDecodePurposeV1 {
    GraphSummary,
    Compile,
    Link,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SlibClosureDecodeUsageV1 {
    pub cone_nodes: u64,
    pub dependency_edges: u64,
    pub graph_depth: u64,
    pub artifact_search_roots: u64,
    pub locator_candidates: u64,
    pub source_files: u64,
    pub source_bytes: u64,
    pub artifact_snapshot_bytes: u64,
    pub archive_members: u64,
    pub directory_carrier_bytes: u64,
    pub logical_heap_bytes: u64,
    pub decoded_nodes: u64,
    pub decoded_edges: u64,
    pub owned_bytes: u64,
    pub validation_work_units: u64,
    pub child_requests: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ArtifactSnapshotChargeKey {
    artifact: ArtifactFingerprint,
    snapshot: Digest256,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ArtifactDecodeChargeKey {
    purpose: SlibClosureDecodePurposeV1,
    snapshot: ArtifactSnapshotChargeKey,
}

#[derive(Debug)]
pub struct SlibClosureDecodeMeterV1 {
    limits: SlibClosureDecodeLimitsV1,
    usage: SlibClosureDecodeUsageV1,
    physical_snapshots: HashSet<Digest256>,
    structural_artifacts: HashSet<ArtifactSnapshotChargeKey>,
    decoded_artifacts: HashSet<ArtifactDecodeChargeKey>,
}

impl SlibClosureDecodeMeterV1 {
    pub fn new(limits: SlibClosureDecodeLimitsV1) -> Self {
        Self {
            limits,
            usage: SlibClosureDecodeUsageV1::default(),
            physical_snapshots: HashSet::new(),
            structural_artifacts: HashSet::new(),
            decoded_artifacts: HashSet::new(),
        }
    }

    pub const fn limits(&self) -> SlibClosureDecodeLimitsV1 {
        self.limits
    }

    pub const fn usage(&self) -> SlibClosureDecodeUsageV1 {
        self.usage
    }

    pub fn charge_graph(
        &mut self,
        nodes: u64,
        edges: u64,
        depth: u64,
    ) -> Result<(), SlibClosureResourceErrorV1> {
        self.charge(SlibClosureResourceKindV1::ConeNodes, nodes)?;
        self.charge(SlibClosureResourceKindV1::DependencyEdges, edges)?;
        self.raise_max(SlibClosureResourceKindV1::GraphDepth, depth)
    }

    pub fn charge_search_roots(&mut self, count: u64) -> Result<(), SlibClosureResourceErrorV1> {
        self.charge(SlibClosureResourceKindV1::ArtifactSearchRoots, count)
    }

    pub fn charge_locator_candidates(
        &mut self,
        count: u64,
    ) -> Result<(), SlibClosureResourceErrorV1> {
        self.charge(SlibClosureResourceKindV1::LocatorCandidates, count)
    }

    pub fn charge_sources(
        &mut self,
        files: u64,
        bytes: u64,
    ) -> Result<(), SlibClosureResourceErrorV1> {
        self.charge(SlibClosureResourceKindV1::SourceFiles, files)?;
        self.charge(SlibClosureResourceKindV1::SourceBytes, bytes)
    }

    pub fn charge_child_request(&mut self) -> Result<(), SlibClosureResourceErrorV1> {
        self.charge(SlibClosureResourceKindV1::ChildRequests, 1)
    }

    /// Charges the deterministic M23 stable-Kahn cost before allocating the
    /// ready set. The formula is independent of comparison count, allocator
    /// behavior, and hash iteration order.
    pub fn charge_stable_kahn(
        &mut self,
        node_count: u64,
        edge_count: u64,
    ) -> Result<(), SlibClosureResourceErrorV1> {
        const READY_SET_ELEMENT_BYTES: u64 = 40;

        let ready_bytes = node_count.checked_mul(READY_SET_ELEMENT_BYTES).ok_or(
            SlibClosureResourceErrorV1::Overflow {
                resource: SlibClosureResourceKindV1::LogicalHeapBytes,
            },
        )?;
        let comparisons = node_count.checked_mul(ceil_log2(node_count.max(2))).ok_or(
            SlibClosureResourceErrorV1::Overflow {
                resource: SlibClosureResourceKindV1::ValidationWorkUnits,
            },
        )?;
        let work =
            comparisons
                .checked_add(edge_count)
                .ok_or(SlibClosureResourceErrorV1::Overflow {
                    resource: SlibClosureResourceKindV1::ValidationWorkUnits,
                })?;
        self.charge(SlibClosureResourceKindV1::LogicalHeapBytes, ready_bytes)?;
        self.charge(SlibClosureResourceKindV1::ValidationWorkUnits, work)
    }

    /// Reserves the deterministic upper bound for all per-source direct and
    /// support projections. Each source can visit every node and edge once;
    /// its persistent output, visited set, and pending set each contain at
    /// most one entry per graph node.
    pub fn charge_graph_projections(
        &mut self,
        source_count: u64,
        node_count: u64,
        edge_count: u64,
    ) -> Result<(), SlibClosureResourceErrorV1> {
        const PROJECTION_NODE_BYTES: u64 = 32 + 40 + 32;

        let node_slots =
            source_count
                .checked_mul(node_count)
                .ok_or(SlibClosureResourceErrorV1::Overflow {
                    resource: SlibClosureResourceKindV1::LogicalHeapBytes,
                })?;
        let heap_bytes = node_slots.checked_mul(PROJECTION_NODE_BYTES).ok_or(
            SlibClosureResourceErrorV1::Overflow {
                resource: SlibClosureResourceKindV1::LogicalHeapBytes,
            },
        )?;
        let traversal =
            node_count
                .checked_add(edge_count)
                .ok_or(SlibClosureResourceErrorV1::Overflow {
                    resource: SlibClosureResourceKindV1::ValidationWorkUnits,
                })?;
        let work =
            source_count
                .checked_mul(traversal)
                .ok_or(SlibClosureResourceErrorV1::Overflow {
                    resource: SlibClosureResourceKindV1::ValidationWorkUnits,
                })?;
        self.charge(SlibClosureResourceKindV1::LogicalHeapBytes, heap_bytes)?;
        self.charge(SlibClosureResourceKindV1::ValidationWorkUnits, work)
    }

    pub fn observe_artifact_snapshot(
        &mut self,
        summary: &crate::PrebuiltManifestSummaryV1,
        snapshot: Digest256,
    ) -> Result<(), SlibClosureResourceErrorV1> {
        self.observe_raw_artifact_snapshot(snapshot, summary.archive_length())?;
        let key = ArtifactSnapshotChargeKey {
            artifact: summary.artifact_fingerprint(),
            snapshot,
        };
        if self.structural_artifacts.contains(&key) {
            return Ok(());
        }
        self.charge(
            SlibClosureResourceKindV1::ArchiveMembers,
            summary.member_count(),
        )?;
        self.charge(
            SlibClosureResourceKindV1::DirectoryCarrierBytes,
            summary.manifest_length(),
        )?;
        self.structural_artifacts.insert(key);
        Ok(())
    }

    /// Charges immutable artifact bytes before any manifest or fingerprint can
    /// be trusted. Invalid artifacts therefore cannot evade the build-wide
    /// physical snapshot budget.
    pub fn observe_raw_artifact_snapshot(
        &mut self,
        snapshot: Digest256,
        byte_length: u64,
    ) -> Result<(), SlibClosureResourceErrorV1> {
        if self.physical_snapshots.contains(&snapshot) {
            return Ok(());
        }
        self.charge(
            SlibClosureResourceKindV1::ArtifactSnapshotBytes,
            byte_length,
        )?;
        self.physical_snapshots.insert(snapshot);
        Ok(())
    }

    pub fn charge_artifact_decode(
        &mut self,
        purpose: SlibClosureDecodePurposeV1,
        artifact: ArtifactFingerprint,
        snapshot: Digest256,
        usage: DecodeUsage,
    ) -> Result<(), SlibClosureResourceErrorV1> {
        let key = ArtifactDecodeChargeKey {
            purpose,
            snapshot: ArtifactSnapshotChargeKey { artifact, snapshot },
        };
        if self.decoded_artifacts.contains(&key) {
            return Ok(());
        }
        self.charge(
            SlibClosureResourceKindV1::LogicalHeapBytes,
            usage.logical_heap_bytes,
        )?;
        self.charge(SlibClosureResourceKindV1::DecodedNodes, usage.decoded_nodes)?;
        self.charge(SlibClosureResourceKindV1::DecodedEdges, usage.decoded_edges)?;
        self.charge(SlibClosureResourceKindV1::OwnedBytes, usage.owned_bytes)?;
        self.charge(
            SlibClosureResourceKindV1::ValidationWorkUnits,
            usage.validation_work_units,
        )?;
        self.decoded_artifacts.insert(key);
        Ok(())
    }

    fn raise_max(
        &mut self,
        resource: SlibClosureResourceKindV1,
        observed: u64,
    ) -> Result<(), SlibClosureResourceErrorV1> {
        let limit = limit(self.limits.0, resource);
        if observed > limit {
            return Err(SlibClosureResourceErrorV1::LimitExceeded {
                resource,
                limit,
                observed,
            });
        }
        let usage = usage_mut(&mut self.usage, resource);
        *usage = (*usage).max(observed);
        Ok(())
    }

    fn charge(
        &mut self,
        resource: SlibClosureResourceKindV1,
        amount: u64,
    ) -> Result<(), SlibClosureResourceErrorV1> {
        let current = *usage_mut(&mut self.usage, resource);
        let observed = current
            .checked_add(amount)
            .ok_or(SlibClosureResourceErrorV1::Overflow { resource })?;
        let limit = limit(self.limits.0, resource);
        if observed > limit {
            return Err(SlibClosureResourceErrorV1::LimitExceeded {
                resource,
                limit,
                observed,
            });
        }
        *usage_mut(&mut self.usage, resource) = observed;
        Ok(())
    }
}

fn ceil_log2(value: u64) -> u64 {
    u64::from(u64::BITS - (value - 1).leading_zeros())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlibClosureDecodeLimitsError {
    ZeroLimit { resource: SlibClosureResourceKindV1 },
}

impl fmt::Display for SlibClosureDecodeLimitsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLimit { resource } => write!(formatter, "{resource} limit must be nonzero"),
        }
    }
}

impl std::error::Error for SlibClosureDecodeLimitsError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlibClosureResourceErrorV1 {
    Overflow {
        resource: SlibClosureResourceKindV1,
    },
    LimitExceeded {
        resource: SlibClosureResourceKindV1,
        limit: u64,
        observed: u64,
    },
}

impl fmt::Display for SlibClosureResourceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Overflow { resource } => write!(formatter, "{resource} usage overflow"),
            Self::LimitExceeded {
                resource,
                limit,
                observed,
            } => write!(
                formatter,
                "{resource} exceeds closure limit {limit}: observed {observed}"
            ),
        }
    }
}

impl std::error::Error for SlibClosureResourceErrorV1 {}

fn resources(values: SlibClosureDecodeLimitValuesV1) -> [(SlibClosureResourceKindV1, u64); 16] {
    use SlibClosureResourceKindV1 as R;
    [
        (R::ConeNodes, values.cone_nodes),
        (R::DependencyEdges, values.dependency_edges),
        (R::GraphDepth, values.graph_depth),
        (R::ArtifactSearchRoots, values.artifact_search_roots),
        (R::LocatorCandidates, values.locator_candidates),
        (R::SourceFiles, values.source_files),
        (R::SourceBytes, values.source_bytes),
        (R::ArtifactSnapshotBytes, values.artifact_snapshot_bytes),
        (R::ArchiveMembers, values.archive_members),
        (R::DirectoryCarrierBytes, values.directory_carrier_bytes),
        (R::LogicalHeapBytes, values.logical_heap_bytes),
        (R::DecodedNodes, values.decoded_nodes),
        (R::DecodedEdges, values.decoded_edges),
        (R::OwnedBytes, values.owned_bytes),
        (R::ValidationWorkUnits, values.validation_work_units),
        (R::ChildRequests, values.child_requests),
    ]
}

fn limit(values: SlibClosureDecodeLimitValuesV1, resource: SlibClosureResourceKindV1) -> u64 {
    resources(values)
        .into_iter()
        .find_map(|(kind, limit)| (kind == resource).then_some(limit))
        .expect("all closure resources have a limit")
}

fn usage_mut(
    usage: &mut SlibClosureDecodeUsageV1,
    resource: SlibClosureResourceKindV1,
) -> &mut u64 {
    match resource {
        SlibClosureResourceKindV1::ConeNodes => &mut usage.cone_nodes,
        SlibClosureResourceKindV1::DependencyEdges => &mut usage.dependency_edges,
        SlibClosureResourceKindV1::GraphDepth => &mut usage.graph_depth,
        SlibClosureResourceKindV1::ArtifactSearchRoots => &mut usage.artifact_search_roots,
        SlibClosureResourceKindV1::LocatorCandidates => &mut usage.locator_candidates,
        SlibClosureResourceKindV1::SourceFiles => &mut usage.source_files,
        SlibClosureResourceKindV1::SourceBytes => &mut usage.source_bytes,
        SlibClosureResourceKindV1::ArtifactSnapshotBytes => &mut usage.artifact_snapshot_bytes,
        SlibClosureResourceKindV1::ArchiveMembers => &mut usage.archive_members,
        SlibClosureResourceKindV1::DirectoryCarrierBytes => &mut usage.directory_carrier_bytes,
        SlibClosureResourceKindV1::LogicalHeapBytes => &mut usage.logical_heap_bytes,
        SlibClosureResourceKindV1::DecodedNodes => &mut usage.decoded_nodes,
        SlibClosureResourceKindV1::DecodedEdges => &mut usage.decoded_edges,
        SlibClosureResourceKindV1::OwnedBytes => &mut usage.owned_bytes,
        SlibClosureResourceKindV1::ValidationWorkUnits => &mut usage.validation_work_units,
        SlibClosureResourceKindV1::ChildRequests => &mut usage.child_requests,
    }
}

#[cfg(test)]
mod tests {
    use scoop_lir::ValidatedLirTargetSelection;
    use scoop_wire::{DecodeLimits, sha256};

    use super::*;

    #[test]
    fn default_limits_match_the_stage4_contract() {
        let values = SlibClosureDecodeLimitsV1::M23_DEFAULT.values();
        assert_eq!(values.cone_nodes, 4_096);
        assert_eq!(values.artifact_snapshot_bytes, 17_179_869_184);
        assert_eq!(values.validation_work_units, 1_073_741_824);
        assert_eq!(values.child_requests, 4_096);
    }

    #[test]
    fn stable_kahn_uses_the_fixed_logarithmic_cost() {
        let mut meter = SlibClosureDecodeMeterV1::new(SlibClosureDecodeLimitsV1::M23_DEFAULT);
        meter.charge_stable_kahn(5, 7).unwrap();
        assert_eq!(meter.usage().logical_heap_bytes, 5 * 40);
        assert_eq!(meter.usage().validation_work_units, 5 * 3 + 7);
    }

    #[test]
    fn graph_projection_reservation_uses_a_fixed_upper_bound() {
        let mut meter = SlibClosureDecodeMeterV1::new(SlibClosureDecodeLimitsV1::M23_DEFAULT);
        meter.charge_graph_projections(3, 5, 7).unwrap();
        assert_eq!(meter.usage().logical_heap_bytes, 3 * 5 * (32 + 40 + 32));
        assert_eq!(meter.usage().validation_work_units, 3 * (5 + 7));
    }

    #[test]
    fn physical_bytes_deduplicate_but_compile_and_link_work_do_not() {
        let bytes = crate::link_decode::complete_strong_artifact_for_test(false);
        let summary = crate::probe_prebuilt_manifest_summary(
            &bytes,
            DecodeLimits::default(),
            ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1,
        )
        .unwrap();
        let snapshot = sha256(&bytes);
        let mut meter = SlibClosureDecodeMeterV1::new(SlibClosureDecodeLimitsV1::M23_DEFAULT);
        meter.observe_artifact_snapshot(&summary, snapshot).unwrap();
        meter.observe_artifact_snapshot(&summary, snapshot).unwrap();
        assert_eq!(meter.usage().artifact_snapshot_bytes, bytes.len() as u64);
        assert_eq!(meter.usage().archive_members, summary.member_count());

        meter
            .charge_artifact_decode(
                SlibClosureDecodePurposeV1::Compile,
                summary.artifact_fingerprint(),
                snapshot,
                summary.decode_usage(),
            )
            .unwrap();
        let once = meter.usage();
        meter
            .charge_artifact_decode(
                SlibClosureDecodePurposeV1::Compile,
                summary.artifact_fingerprint(),
                snapshot,
                summary.decode_usage(),
            )
            .unwrap();
        assert_eq!(meter.usage(), once);
        meter
            .charge_artifact_decode(
                SlibClosureDecodePurposeV1::Link,
                summary.artifact_fingerprint(),
                snapshot,
                summary.decode_usage(),
            )
            .unwrap();
        assert_eq!(
            meter.usage().validation_work_units,
            once.validation_work_units * 2
        );
    }

    #[test]
    fn invalid_raw_snapshots_are_charged_before_manifest_decode() {
        let snapshot = sha256(b"not an slib");
        let mut meter = SlibClosureDecodeMeterV1::new(SlibClosureDecodeLimitsV1::M23_DEFAULT);
        meter.observe_raw_artifact_snapshot(snapshot, 11).unwrap();
        meter.observe_raw_artifact_snapshot(snapshot, 11).unwrap();

        assert_eq!(meter.usage().artifact_snapshot_bytes, 11);
        assert_eq!(meter.usage().archive_members, 0);
    }

    #[test]
    fn checked_charges_report_the_exact_observed_value() {
        let mut values = SlibClosureDecodeLimitsV1::M23_DEFAULT.values();
        values.child_requests = 1;
        let limits = SlibClosureDecodeLimitsV1::new(values).unwrap();
        let mut meter = SlibClosureDecodeMeterV1::new(limits);
        meter.charge_child_request().unwrap();
        assert_eq!(
            meter.charge_child_request().unwrap_err(),
            SlibClosureResourceErrorV1::LimitExceeded {
                resource: SlibClosureResourceKindV1::ChildRequests,
                limit: 1,
                observed: 2,
            }
        );
    }
}
