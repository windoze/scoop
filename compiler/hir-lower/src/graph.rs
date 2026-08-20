//! SCC ordering shared by value-layout and release-call dependency analysis.

use std::collections::{HashMap, HashSet};

pub(crate) fn strongly_connected_components<N: Copy + Eq + std::hash::Hash>(
    nodes: &[N],
    graph: &HashMap<N, Vec<N>>,
) -> Vec<Vec<N>> {
    fn finish<N: Copy + Eq + std::hash::Hash>(
        node: N,
        graph: &HashMap<N, Vec<N>>,
        visited: &mut HashSet<N>,
        order: &mut Vec<N>,
    ) {
        if !visited.insert(node) {
            return;
        }
        for &target in &graph[&node] {
            finish(target, graph, visited, order);
        }
        order.push(node);
    }

    fn collect<N: Copy + Eq + std::hash::Hash>(
        node: N,
        reverse: &HashMap<N, Vec<N>>,
        visited: &mut HashSet<N>,
        component: &mut Vec<N>,
    ) {
        if !visited.insert(node) {
            return;
        }
        component.push(node);
        for &target in &reverse[&node] {
            collect(target, reverse, visited, component);
        }
    }

    let mut finish_order = Vec::with_capacity(nodes.len());
    let mut visited = HashSet::new();
    for &node in nodes {
        finish(node, graph, &mut visited, &mut finish_order);
    }

    let mut reverse = nodes
        .iter()
        .copied()
        .map(|node| (node, Vec::new()))
        .collect::<HashMap<_, _>>();
    for (&source, targets) in graph {
        for &target in targets {
            reverse
                .get_mut(&target)
                .expect("every dependency belongs to the graph")
                .push(source);
        }
    }

    visited.clear();
    let mut components = Vec::new();
    while let Some(node) = finish_order.pop() {
        if visited.contains(&node) {
            continue;
        }
        let mut component = Vec::new();
        collect(node, &reverse, &mut visited, &mut component);
        components.push(component);
    }
    components
}
