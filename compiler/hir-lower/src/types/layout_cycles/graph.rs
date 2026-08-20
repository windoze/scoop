use std::collections::{HashMap, HashSet};

use crate::ValueLayoutTemplate;

pub(super) use crate::graph::strongly_connected_components;

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
