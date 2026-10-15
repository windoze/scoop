//! Reachable dependency positions for the shared foundation reader.
pub(crate) fn transitive_dependency_positions(
    position: usize,
    dependency_positions: &[Vec<usize>],
) -> Vec<usize> {
    let mut reachable = vec![false; position];
    let mut pending = dependency_positions[position].clone();
    while let Some(dependency) = pending.pop() {
        if reachable[dependency] {
            continue;
        }
        reachable[dependency] = true;
        pending.extend(dependency_positions[dependency].iter().copied());
    }
    reachable
        .into_iter()
        .enumerate()
        .filter_map(|(dependency, reachable)| reachable.then_some(dependency))
        .collect()
}
