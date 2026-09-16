use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::Hash;

use crate::{WireError, WireErrorKind, WirePath};

pub const LOGICAL_NODE_BYTES: u64 = 64;
pub const COLLECTION_ELEMENT_BYTES: u64 = 32;
pub const GRAPH_EDGE_BYTES: u64 = 16;
pub const READY_SET_ELEMENT_BYTES: u64 = 40;
pub const PENDING_REMAP_ENTRY_BYTES: u64 = 96;

/// Resources covered by the deterministic logical decode cost model.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResourceKind {
    CborNesting,
    SemanticLeafBytes,
    SemanticTableEntries,
    SemanticRecursion,
    LogicalHeapBytes,
    DecodedNodes,
    DecodedEdges,
    OwnedBytes,
    ValidationWorkUnits,
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CborNesting => "CBOR nesting",
            Self::SemanticLeafBytes => "semantic leaf bytes",
            Self::SemanticTableEntries => "semantic table entries",
            Self::SemanticRecursion => "semantic recursion",
            Self::LogicalHeapBytes => "decoded logical heap bytes",
            Self::DecodedNodes => "decoded nodes",
            Self::DecodedEdges => "decoded edges",
            Self::OwnedBytes => "owned/copied bytes",
            Self::ValidationWorkUnits => "validation work units",
        })
    }
}

/// The shared M23-2 limits that apply inside known wire payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodeLimits {
    pub cbor_nesting: u64,
    pub semantic_table_entries: u64,
    pub semantic_leaf_bytes: u64,
    pub semantic_recursion: u64,
    pub logical_heap_bytes: u64,
    pub decoded_nodes: u64,
    pub decoded_edges: u64,
    pub owned_bytes: u64,
    pub validation_work_units: u64,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self::M23_DEFAULT
    }
}

impl DecodeLimits {
    pub const M23_DEFAULT: Self = Self {
        cbor_nesting: 128,
        semantic_table_entries: 16_777_216,
        semantic_leaf_bytes: 16_777_216,
        semantic_recursion: 1_024,
        logical_heap_bytes: 2_147_483_648,
        decoded_nodes: 16_777_216,
        decoded_edges: 67_108_864,
        owned_bytes: 1_073_741_824,
        validation_work_units: 268_435_456,
    };
}

/// Monotonic usage. No successful operation refunds budget.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DecodeUsage {
    pub logical_heap_bytes: u64,
    pub decoded_nodes: u64,
    pub decoded_edges: u64,
    pub owned_bytes: u64,
    pub validation_work_units: u64,
}

/// A deterministic, injectable meter shared by all decoders for an artifact.
#[derive(Debug, Eq, PartialEq)]
pub struct BudgetMeter {
    limits: DecodeLimits,
    usage: DecodeUsage,
}

impl BudgetMeter {
    pub fn new(limits: DecodeLimits) -> Self {
        Self {
            limits,
            usage: DecodeUsage::default(),
        }
    }

    pub fn limits(&self) -> DecodeLimits {
        self.limits
    }

    pub const fn usage(&self) -> DecodeUsage {
        self.usage
    }

    pub fn check_cbor_depth(&self, depth: u64, path: &WirePath) -> Result<(), WireError> {
        check_limit(
            ResourceKind::CborNesting,
            self.limits.cbor_nesting,
            depth,
            path,
        )
    }

    pub fn check_semantic_leaf(&self, length: u64, path: &WirePath) -> Result<(), WireError> {
        check_limit(
            ResourceKind::SemanticLeafBytes,
            self.limits.semantic_leaf_bytes,
            length,
            path,
        )
    }

    pub fn check_table_entries(&self, count: u64, path: &WirePath) -> Result<(), WireError> {
        check_limit(
            ResourceKind::SemanticTableEntries,
            self.limits.semantic_table_entries,
            count,
            path,
        )
    }

    pub fn check_semantic_depth(&self, depth: u64, path: &WirePath) -> Result<(), WireError> {
        check_limit(
            ResourceKind::SemanticRecursion,
            self.limits.semantic_recursion,
            depth,
            path,
        )
    }

    pub fn charge_nodes(&mut self, count: u64, path: &WirePath) -> Result<(), WireError> {
        charge(
            &mut self.usage.decoded_nodes,
            count,
            ResourceKind::DecodedNodes,
            self.limits.decoded_nodes,
            path,
        )?;
        self.charge_heap(product(count, LOGICAL_NODE_BYTES, path)?, path)
    }

    pub fn charge_collection_slots(
        &mut self,
        count: u64,
        path: &WirePath,
    ) -> Result<(), WireError> {
        self.check_table_entries(count, path)?;
        self.charge_heap(product(count, COLLECTION_ELEMENT_BYTES, path)?, path)
    }

    pub fn charge_edges(&mut self, count: u64, path: &WirePath) -> Result<(), WireError> {
        charge(
            &mut self.usage.decoded_edges,
            count,
            ResourceKind::DecodedEdges,
            self.limits.decoded_edges,
            path,
        )?;
        self.charge_heap(product(count, GRAPH_EDGE_BYTES, path)?, path)
    }

    pub fn charge_heap(&mut self, bytes: u64, path: &WirePath) -> Result<(), WireError> {
        charge(
            &mut self.usage.logical_heap_bytes,
            bytes,
            ResourceKind::LogicalHeapBytes,
            self.limits.logical_heap_bytes,
            path,
        )
    }

    pub fn charge_owned_bytes(&mut self, bytes: u64, path: &WirePath) -> Result<(), WireError> {
        charge(
            &mut self.usage.owned_bytes,
            bytes,
            ResourceKind::OwnedBytes,
            self.limits.owned_bytes,
            path,
        )?;
        self.charge_heap(bytes, path)
    }

    pub fn try_copy_str(&mut self, value: &str, path: &WirePath) -> Result<String, WireError> {
        let length = u64::try_from(value.len())
            .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        self.charge_owned_bytes(length, path)?;
        let mut copy = String::new();
        copy.try_reserve_exact(value.len()).map_err(|_| {
            WireError::new(
                WireErrorKind::ResourceAllocation {
                    requested_logical_bytes: length,
                    requested_slots: length,
                },
                path.clone(),
                None,
            )
        })?;
        copy.push_str(value);
        Ok(copy)
    }

    pub fn charge_work(&mut self, units: u64, path: &WirePath) -> Result<(), WireError> {
        charge(
            &mut self.usage.validation_work_units,
            units,
            ResourceKind::ValidationWorkUnits,
            self.limits.validation_work_units,
            path,
        )
    }

    pub fn charge_sha256(&mut self, stream_length: u64, path: &WirePath) -> Result<(), WireError> {
        let padded = stream_length.checked_add(72).ok_or_else(|| {
            limit_error(ResourceKind::ValidationWorkUnits, u64::MAX, u64::MAX, path)
        })?;
        self.charge_work(padded / 64, path)
    }

    pub fn charge_canonical_sequence(
        &mut self,
        length: u64,
        path: &WirePath,
    ) -> Result<(), WireError> {
        self.charge_work(length.saturating_sub(1), path)
    }

    pub fn charge_stable_kahn(
        &mut self,
        node_count: u64,
        edge_count: u64,
        path: &WirePath,
    ) -> Result<(), WireError> {
        let logarithm = ceil_log2(node_count.max(2));
        let comparisons = product(node_count, logarithm, path)?;
        let work = comparisons.checked_add(edge_count).ok_or_else(|| {
            limit_error(ResourceKind::ValidationWorkUnits, u64::MAX, u64::MAX, path)
        })?;
        self.charge_heap(product(node_count, READY_SET_ELEMENT_BYTES, path)?, path)?;
        self.charge_work(work, path)
    }

    pub fn charge_pending_remap(&mut self, count: u64, path: &WirePath) -> Result<(), WireError> {
        self.charge_heap(product(count, PENDING_REMAP_ENTRY_BYTES, path)?, path)
    }

    pub fn try_reserve_exact<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: u64,
        logical_slot_bytes: u64,
        path: &WirePath,
    ) -> Result<(), WireError> {
        self.charge_heap(product(additional, logical_slot_bytes, path)?, path)?;
        let additional = usize::try_from(additional)
            .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        values.try_reserve_exact(additional).map_err(|_| {
            let requested_logical_bytes = (additional as u64).saturating_mul(logical_slot_bytes);
            WireError::new(
                WireErrorKind::ResourceAllocation {
                    requested_logical_bytes,
                    requested_slots: additional as u64,
                },
                path.clone(),
                None,
            )
        })
    }

    /// Charges and fallibly reserves slots for an owned validation-time
    /// collection whose size came from an already decoded collection.
    pub fn try_reserve_collection_slots<T>(
        &mut self,
        values: &mut Vec<T>,
        additional: usize,
        path: &WirePath,
    ) -> Result<(), WireError> {
        let additional = u64::try_from(additional)
            .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        self.try_reserve_exact(values, additional, COLLECTION_ELEMENT_BYTES, path)
    }

    pub fn try_reserve_map_slots<K: Eq + Hash, V>(
        &mut self,
        values: &mut HashMap<K, V>,
        additional: usize,
        path: &WirePath,
    ) -> Result<(), WireError> {
        let additional = u64::try_from(additional)
            .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        self.charge_heap(product(additional, COLLECTION_ELEMENT_BYTES, path)?, path)?;
        let capacity = usize::try_from(additional)
            .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        values.try_reserve(capacity).map_err(|_| {
            WireError::new(
                WireErrorKind::ResourceAllocation {
                    requested_logical_bytes: additional.saturating_mul(COLLECTION_ELEMENT_BYTES),
                    requested_slots: additional,
                },
                path.clone(),
                None,
            )
        })
    }

    pub fn try_reserve_set_slots<T: Eq + Hash>(
        &mut self,
        values: &mut HashSet<T>,
        additional: usize,
        path: &WirePath,
    ) -> Result<(), WireError> {
        let additional = u64::try_from(additional)
            .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        self.charge_heap(product(additional, COLLECTION_ELEMENT_BYTES, path)?, path)?;
        let capacity = usize::try_from(additional)
            .map_err(|_| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        values.try_reserve(capacity).map_err(|_| {
            WireError::new(
                WireErrorKind::ResourceAllocation {
                    requested_logical_bytes: additional.saturating_mul(COLLECTION_ELEMENT_BYTES),
                    requested_slots: additional,
                },
                path.clone(),
                None,
            )
        })
    }
}

fn ceil_log2(value: u64) -> u64 {
    u64::from(u64::BITS - (value - 1).leading_zeros())
}

fn product(left: u64, right: u64, path: &WirePath) -> Result<u64, WireError> {
    left.checked_mul(right)
        .ok_or_else(|| limit_error(ResourceKind::LogicalHeapBytes, u64::MAX, u64::MAX, path))
}

fn charge(
    current: &mut u64,
    amount: u64,
    resource: ResourceKind,
    limit: u64,
    path: &WirePath,
) -> Result<(), WireError> {
    let observed = current
        .checked_add(amount)
        .ok_or_else(|| limit_error(resource, limit, u64::MAX, path))?;
    check_limit(resource, limit, observed, path)?;
    *current = observed;
    Ok(())
}

fn check_limit(
    resource: ResourceKind,
    limit: u64,
    observed: u64,
    path: &WirePath,
) -> Result<(), WireError> {
    if observed <= limit {
        Ok(())
    } else {
        Err(limit_error(resource, limit, observed, path))
    }
}

fn limit_error(resource: ResourceKind, limit: u64, observed: u64, path: &WirePath) -> WireError {
    WireError::new(
        WireErrorKind::LimitExceeded {
            resource,
            limit,
            observed,
        },
        path.clone(),
        None,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use super::{BudgetMeter, DecodeLimits, DecodeUsage, ResourceKind};
    use crate::{WireErrorKind, WirePath};

    fn tiny_limits(limit: u64) -> DecodeLimits {
        DecodeLimits {
            decoded_nodes: limit,
            logical_heap_bytes: limit * 64,
            validation_work_units: limit,
            ..DecodeLimits::default()
        }
    }

    #[test]
    fn inclusive_node_budget_covers_limit_minus_one_limit_and_limit_plus_one() {
        let path = WirePath::default();
        for accepted in [2, 3] {
            let mut meter = BudgetMeter::new(tiny_limits(3));
            meter.charge_nodes(accepted, &path).unwrap();
        }

        let mut meter = BudgetMeter::new(tiny_limits(3));
        let error = meter.charge_nodes(4, &path).unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::LimitExceeded {
                resource: ResourceKind::DecodedNodes,
                limit: 3,
                observed: 4,
            }
        );
        assert_eq!(meter.usage(), DecodeUsage::default());
    }

    #[test]
    fn collection_slot_reservation_has_inclusive_boundaries() {
        let path = WirePath::root().field(7);
        let required = super::COLLECTION_ELEMENT_BYTES;
        for (limit, accepted) in [
            (required - 1, false),
            (required, true),
            (required + 1, true),
        ] {
            let mut meter = BudgetMeter::new(DecodeLimits {
                logical_heap_bytes: limit,
                ..DecodeLimits::default()
            });
            let mut values = Vec::<u8>::new();
            let result = meter.try_reserve_collection_slots(&mut values, 1, &path);
            assert_eq!(result.is_ok(), accepted);
            if !accepted {
                assert_eq!(
                    result.unwrap_err().kind(),
                    &WireErrorKind::LimitExceeded {
                        resource: ResourceKind::LogicalHeapBytes,
                        limit,
                        observed: required,
                    }
                );
            }
        }
    }

    #[test]
    fn string_copy_has_inclusive_owned_byte_boundaries() {
        let path = WirePath::root().field(9);
        for (limit, accepted) in [(2, false), (3, true), (4, true)] {
            let mut meter = BudgetMeter::new(DecodeLimits {
                owned_bytes: limit,
                ..DecodeLimits::default()
            });
            let result = meter.try_copy_str("lib", &path);
            assert_eq!(result.is_ok(), accepted);
            if accepted {
                assert_eq!(result.unwrap(), "lib");
            } else {
                assert_eq!(
                    result.unwrap_err().kind(),
                    &WireErrorKind::LimitExceeded {
                        resource: ResourceKind::OwnedBytes,
                        limit,
                        observed: 3,
                    }
                );
            }
        }
    }

    #[test]
    fn hash_index_reservation_has_inclusive_boundaries() {
        let path = WirePath::root().field(12);
        let required = 2 * super::COLLECTION_ELEMENT_BYTES;
        for (limit, accepted) in [
            (required - 1, false),
            (required, true),
            (required + 1, true),
        ] {
            let mut meter = BudgetMeter::new(DecodeLimits {
                logical_heap_bytes: limit,
                ..DecodeLimits::default()
            });
            let mut map = HashMap::<u8, u8>::new();
            let mut set = HashSet::<u8>::new();
            meter.try_reserve_map_slots(&mut map, 1, &path).unwrap();
            let result = meter.try_reserve_set_slots(&mut set, 1, &path);
            assert_eq!(result.is_ok(), accepted);
            if !accepted {
                assert_eq!(
                    result.unwrap_err().kind(),
                    &WireErrorKind::LimitExceeded {
                        resource: ResourceKind::LogicalHeapBytes,
                        limit,
                        observed: required,
                    }
                );
            }
        }
    }

    #[test]
    fn sha256_and_kahn_work_use_fixed_formulas() {
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        let path = WirePath::default();
        meter.charge_sha256(56, &path).unwrap();
        meter.charge_canonical_sequence(5, &path).unwrap();
        meter.charge_stable_kahn(5, 7, &path).unwrap();

        assert_eq!(meter.usage().validation_work_units, 2 + 4 + 22);
        assert_eq!(meter.usage().logical_heap_bytes, 5 * 40);
    }

    #[test]
    fn cumulative_usage_does_not_refund_after_failure() {
        let mut meter = BudgetMeter::new(tiny_limits(3));
        let path = WirePath::default();
        meter.charge_nodes(2, &path).unwrap();
        meter.charge_nodes(2, &path).unwrap_err();
        assert_eq!(meter.usage().decoded_nodes, 2);
    }
}
