use std::collections::{HashMap, HashSet};

use crate::ValueLayoutTemplate;

pub(super) fn strongly_connected_components(
    nodes: &[ValueLayoutTemplate],
    graph: &HashMap<ValueLayoutTemplate, Vec<ValueLayoutTemplate>>,
) -> Vec<Vec<ValueLayoutTemplate>> {
    fn finish(
        node: ValueLayoutTemplate,
        graph: &HashMap<ValueLayoutTemplate, Vec<ValueLayoutTemplate>>,
        visited: &mut HashSet<ValueLayoutTemplate>,
        order: &mut Vec<ValueLayoutTemplate>,
    ) {
        if !visited.insert(node) {
            return;
        }
        for &target in &graph[&node] {
            finish(target, graph, visited, order);
        }
        order.push(node);
    }

    fn collect(
        node: ValueLayoutTemplate,
        reverse: &HashMap<ValueLayoutTemplate, Vec<ValueLayoutTemplate>>,
        visited: &mut HashSet<ValueLayoutTemplate>,
        component: &mut Vec<ValueLayoutTemplate>,
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
                .expect("every dependency is a value template")
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

pub(super) fn find_component_cycle(
    root: ValueLayoutTemplate,
    component: &[ValueLayoutTemplate],
    graph: &HashMap<ValueLayoutTemplate, Vec<ValueLayoutTemplate>>,
) -> Vec<ValueLayoutTemplate> {
    fn path_to_root(
        current: ValueLayoutTemplate,
        root: ValueLayoutTemplate,
        allowed: &HashSet<ValueLayoutTemplate>,
        graph: &HashMap<ValueLayoutTemplate, Vec<ValueLayoutTemplate>>,
        visited: &mut HashSet<ValueLayoutTemplate>,
        path: &mut Vec<ValueLayoutTemplate>,
    ) -> bool {
        path.push(current);
        if current == root {
            return true;
        }
        visited.insert(current);
        for &next in &graph[&current] {
            if allowed.contains(&next)
                && !visited.contains(&next)
                && path_to_root(next, root, allowed, graph, visited, path)
            {
                return true;
            }
        }
        path.pop();
        false
    }

    if graph[&root].contains(&root) {
        return vec![root, root];
    }
    let allowed = component.iter().copied().collect::<HashSet<_>>();
    for &next in &graph[&root] {
        if !allowed.contains(&next) {
            continue;
        }
        let mut path = vec![root];
        let mut visited = HashSet::new();
        if path_to_root(next, root, &allowed, graph, &mut visited, &mut path) {
            return path;
        }
    }
    unreachable!("a root in a non-trivial strongly connected component has a cycle")
}
