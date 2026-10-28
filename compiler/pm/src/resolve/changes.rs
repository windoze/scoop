use super::*;

pub(super) fn describe(
    previous: Option<&LockedResolution>,
    current: &LockedResolution,
    solution: &Solution,
) -> Vec<String> {
    let before: BTreeMap<_, _> = previous
        .into_iter()
        .flat_map(|graph| &graph.nodes)
        .map(|node| (Key::from_coordinate(&node.coordinate), node))
        .collect();
    let mut changes = Vec::new();
    for node in &current.nodes {
        let key = Key::from_coordinate(&node.coordinate);
        let message = match before.get(&key) {
            Some(old)
                if old.coordinate == node.coordinate
                    && old.source == node.source
                    && old.kind == node.kind =>
            {
                continue;
            }
            Some(old) => format!(
                "{key}: {} ({}) -> {} ({})",
                old.coordinate.version(),
                old.source,
                node.coordinate.version(),
                node.source
            ),
            None => format!(
                "{key}: added {} ({})",
                node.coordinate.version(),
                node.source
            ),
        };
        changes.push(message);
        for parent in solution.packages.values() {
            for edge in parent.dependencies.iter().filter(|edge| edge.key == key) {
                let versions = if edge.versions.is_empty() {
                    "declared source".into()
                } else {
                    edge.versions
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(" and ")
                };
                changes.push(format!(
                    "  required by {}: {versions} from {}",
                    parent.coordinate, edge.source
                ));
            }
        }
    }
    for (key, node) in before {
        if !solution.packages.contains_key(&key) {
            changes.push(format!("{key}: removed {}", node.coordinate.version()));
        }
    }
    changes
}
