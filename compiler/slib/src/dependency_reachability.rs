use scoop_wire::{WireError, WirePath};

/// The validated graph puts every dependency before its user. A reverse walk
/// therefore closes reachability without a second, potentially repeated queue.
pub(crate) fn transitive_positions(
    position: usize,
    dependencies: &[Vec<usize>],
) -> Result<Vec<usize>, WireError> {
    let path = WirePath::root();

    let mut reachable = Vec::new();
    scoop_wire::allocation::try_reserve(&mut reachable, position, &path)?;
    reachable.resize(position, false);
    for node in std::iter::once(position).chain((0..position).rev()) {
        if node == position || reachable[node] {
            for dependency in &dependencies[node] {
                reachable[*dependency] = true;
            }
        }
    }
    let mut result = Vec::new();
    scoop_wire::allocation::try_reserve(
        &mut result,
        reachable.iter().filter(|reachable| **reachable).count(),
        &path,
    )?;
    result.extend(
        reachable
            .into_iter()
            .enumerate()
            .filter_map(|(index, reachable)| reachable.then_some(index)),
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitive_queries_keep_diamond_siblings_scoped_to_their_own_dependencies() {
        let dependencies = [vec![], vec![0], vec![0], vec![1, 2], vec![1]];

        assert_eq!(transitive_positions(3, &dependencies).unwrap(), [0, 1, 2]);
        assert_eq!(transitive_positions(4, &dependencies).unwrap(), [0, 1]);
        assert_eq!(transitive_positions(2, &dependencies).unwrap(), [0]);
    }
}
