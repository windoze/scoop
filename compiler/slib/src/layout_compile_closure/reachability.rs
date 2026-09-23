use scoop_wire::{BudgetMeter, WireError, WirePath};

/// The validated graph puts every dependency before its user. A reverse walk
/// therefore closes reachability without a second, potentially repeated queue.
pub(super) fn transitive_positions(
    position: usize,
    dependencies: &[Vec<usize>],
    meter: &mut BudgetMeter,
) -> Result<Vec<usize>, WireError> {
    let path = WirePath::root();
    meter.charge_work((position as u64).saturating_mul(2).saturating_add(1), &path)?;
    let mut reachable = Vec::new();
    meter.try_reserve_collection_slots(&mut reachable, position, &path)?;
    reachable.resize(position, false);
    for node in std::iter::once(position).chain((0..position).rev()) {
        if node == position || reachable[node] {
            meter.charge_work(dependencies[node].len() as u64, &path)?;
            for dependency in &dependencies[node] {
                reachable[*dependency] = true;
            }
        }
    }
    let mut result = Vec::new();
    meter.try_reserve_collection_slots(
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
    use scoop_wire::DecodeLimits;

    #[test]
    fn transitive_queries_keep_diamond_siblings_scoped_to_their_own_dependencies() {
        let dependencies = [vec![], vec![0], vec![0], vec![1, 2], vec![1]];
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        assert_eq!(
            transitive_positions(3, &dependencies, &mut meter).unwrap(),
            [0, 1, 2]
        );
        assert_eq!(
            transitive_positions(4, &dependencies, &mut meter).unwrap(),
            [0, 1]
        );
        assert_eq!(
            transitive_positions(2, &dependencies, &mut meter).unwrap(),
            [0]
        );
    }

    #[test]
    fn temporary_reachability_storage_uses_the_callers_heap_budget() {
        let mut meter = BudgetMeter::new(DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        });
        assert!(transitive_positions(1, &[vec![], vec![0]], &mut meter).is_err());
    }
}
