use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::RefScan;

use super::RefScanValidationError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScanBudgetResourceV1 {
    Depth,
    DistinctNodes,
    DistinctWords,
    ExpandedNodes,
    CanonicalBytes,
}

impl ScanBudgetResourceV1 {
    pub const fn maximum(self) -> u64 {
        match self {
            Self::Depth => 64,
            Self::DistinctNodes => 65_536,
            Self::DistinctWords | Self::ExpandedNodes => 1_048_576,
            Self::CanonicalBytes => 16_777_216,
        }
    }

    fn check(self, actual: u64) -> Result<(), RefScanValidationError> {
        if actual <= self.maximum() {
            Ok(())
        } else {
            Err(RefScanValidationError::BudgetExceeded {
                resource: self,
                maximum: self.maximum(),
                actual,
            })
        }
    }

    fn add(self, left: u64, right: u64) -> Result<u64, RefScanValidationError> {
        let actual = left
            .checked_add(right)
            .ok_or(RefScanValidationError::BudgetExceeded {
                resource: self,
                maximum: self.maximum(),
                actual: u64::MAX,
            })?;
        self.check(actual)?;
        Ok(actual)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ScanBudgetUsageV1 {
    pub depth: u64,
    pub distinct_nodes: u64,
    pub distinct_words: u64,
    pub expanded_nodes: u64,
    pub canonical_bytes: u64,
}

#[derive(Clone, Copy, Debug)]
struct Cost {
    depth: u64,
    expanded_nodes: u64,
    bytes: u64,
}

/// The accounting algorithm also accepts shared physical graphs. The LIR
/// tree adapter cannot form cycles, but uses the same active-path and memo
/// rules as a graph reader rather than recursively expanding shared nodes.
trait Node: Copy {
    type Key: Copy + Eq + Hash;
    fn key(self) -> Self::Key;
    fn dimensions(self) -> Result<(u64, u64, u64), RefScanValidationError>;
    fn children(self) -> impl Iterator<Item = Self>;
}

struct Meter<K> {
    active: HashSet<K>,
    memo: HashMap<K, Cost>,
    nodes: u64,
    words: u64,
}

impl<K: Copy + Eq + Hash> Meter<K> {
    fn visit<N: Node<Key = K>>(
        &mut self,
        node: N,
        depth: u64,
    ) -> Result<Cost, RefScanValidationError> {
        use ScanBudgetResourceV1 as R;
        R::Depth.check(depth)?;
        let key = node.key();
        if self.active.contains(&key) {
            return Err(RefScanValidationError::Cycle);
        }
        if let Some(cost) = self.memo.get(&key).copied() {
            R::Depth.add(depth - 1, cost.depth)?;
            return Ok(cost);
        }
        let (nodes, words, bytes) = node.dimensions()?;
        self.nodes = R::DistinctNodes.add(self.nodes, nodes)?;
        self.words = R::DistinctWords.add(self.words, words)?;
        R::CanonicalBytes.check(bytes)?;
        self.active.insert(key);
        let mut cost = Cost {
            depth: 1,
            expanded_nodes: nodes,
            bytes,
        };
        for child in node.children() {
            let child = self.visit(child, depth + 1)?;
            cost.depth = cost.depth.max(child.depth + 1);
            cost.expanded_nodes =
                R::ExpandedNodes.add(cost.expanded_nodes, child.expanded_nodes)?;
            cost.bytes = R::CanonicalBytes.add(cost.bytes, child.bytes)?;
        }
        self.active.remove(&key);
        self.memo.insert(key, cost);
        Ok(cost)
    }
}

fn measure_node<N: Node>(node: N) -> Result<ScanBudgetUsageV1, RefScanValidationError> {
    let mut meter = Meter {
        active: HashSet::new(),
        memo: HashMap::new(),
        nodes: 0,
        words: 0,
    };
    let cost = meter.visit(node, 1)?;
    Ok(ScanBudgetUsageV1 {
        depth: cost.depth,
        distinct_nodes: meter.nodes,
        distinct_words: meter.words,
        expanded_nodes: cost.expanded_nodes,
        canonical_bytes: cost.bytes,
    })
}

impl Node for &RefScan {
    type Key = *const RefScan;
    fn key(self) -> Self::Key {
        self
    }
    fn dimensions(self) -> Result<(u64, u64, u64), RefScanValidationError> {
        use ScanBudgetResourceV1 as R;
        Ok(match self {
            RefScan::None => (0, 0, 4),
            RefScan::References(offsets) => {
                let count = offsets.len() as u64;
                let words = R::DistinctWords.add(1, count)?;
                let bytes = count
                    .checked_mul(8)
                    .ok_or(RefScanValidationError::BudgetExceeded {
                        resource: R::CanonicalBytes,
                        maximum: R::CanonicalBytes.maximum(),
                        actual: u64::MAX,
                    })?;
                (1, words, R::CanonicalBytes.add(12, bytes)?)
            }
            RefScan::Sequence(parts) => (1, R::DistinctWords.add(2, parts.len() as u64)?, 12),
            RefScan::Array { .. } => (1, 5, 28),
        })
    }
    fn children(self) -> impl Iterator<Item = Self> {
        let (parts, element) = match self {
            RefScan::Sequence(parts) => (parts.as_slice(), None),
            RefScan::Array { element, .. } => (&[][..], Some(element.as_ref_scan())),
            RefScan::None | RefScan::References(_) => (&[][..], None),
        };
        parts.iter().chain(element)
    }
}

pub(super) fn measure(scan: &RefScan) -> Result<ScanBudgetUsageV1, RefScanValidationError> {
    measure_node(scan)
}

#[cfg(test)]
mod tests;
