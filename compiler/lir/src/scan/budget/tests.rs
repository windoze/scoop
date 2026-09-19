use super::*;

struct Raw {
    nodes: u64,
    words: u64,
    bytes: u64,
    children: Vec<usize>,
}

#[derive(Clone, Copy)]
struct GraphNode<'a> {
    graph: &'a [Raw],
    id: usize,
}

impl Node for GraphNode<'_> {
    type Key = usize;
    fn key(self) -> usize {
        self.id
    }
    fn dimensions(self) -> Result<(u64, u64, u64), RefScanValidationError> {
        let node = &self.graph[self.id];
        Ok((node.nodes, node.words, node.bytes))
    }
    fn children(self) -> impl Iterator<Item = Self> {
        self.graph[self.id].children.iter().map(move |id| Self {
            graph: self.graph,
            id: *id,
        })
    }
}

fn measure(graph: &[Raw]) -> Result<ScanBudgetUsageV1, RefScanValidationError> {
    measure_node(GraphNode { graph, id: 0 })
}

#[test]
fn physical_shared_nodes_are_counted_once_but_expansion_is_per_path() {
    let graph = [
        Raw {
            nodes: 1,
            words: 4,
            bytes: 12,
            children: vec![1, 1],
        },
        Raw {
            nodes: 1,
            words: 2,
            bytes: 20,
            children: vec![],
        },
    ];
    assert_eq!(
        measure(&graph).unwrap(),
        ScanBudgetUsageV1 {
            depth: 2,
            distinct_nodes: 2,
            distinct_words: 6,
            expanded_nodes: 3,
            canonical_bytes: 52,
        }
    );
}

#[test]
fn active_path_rejects_cycle_and_memoized_depth_checks_deeper_paths() {
    let cycle = [Raw {
        nodes: 1,
        words: 3,
        bytes: 12,
        children: vec![0],
    }];
    assert_eq!(measure(&cycle), Err(RefScanValidationError::Cycle));
    let mut graph: Vec<_> = (0..65)
        .map(|i| Raw {
            nodes: 1,
            words: 5,
            bytes: 28,
            children: if i < 64 { vec![i + 1] } else { vec![] },
        })
        .collect();
    graph[0].children = vec![64, 1];
    assert!(matches!(
        measure(&graph),
        Err(RefScanValidationError::BudgetExceeded {
            resource: ScanBudgetResourceV1::Depth,
            ..
        })
    ));
}

#[test]
fn each_resource_accepts_its_boundary_and_rejects_checked_overflow() {
    for resource in [
        ScanBudgetResourceV1::Depth,
        ScanBudgetResourceV1::DistinctNodes,
        ScanBudgetResourceV1::DistinctWords,
        ScanBudgetResourceV1::ExpandedNodes,
        ScanBudgetResourceV1::CanonicalBytes,
    ] {
        resource.check(resource.maximum()).unwrap();
        assert!(resource.check(resource.maximum() + 1).is_err());
        assert!(resource.add(u64::MAX, 1).is_err());
    }
}

#[test]
fn compact_dag_cannot_expand_exponentially_before_budget_check() {
    let graph: Vec<_> = (0..24)
        .map(|i| Raw {
            nodes: 1,
            words: 4,
            bytes: 12,
            children: if i < 23 { vec![i + 1, i + 1] } else { vec![] },
        })
        .collect();
    assert!(matches!(
        measure(&graph),
        Err(RefScanValidationError::BudgetExceeded {
            resource: ScanBudgetResourceV1::ExpandedNodes,
            ..
        })
    ));
}
