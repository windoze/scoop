//! Dependency-view construction shared by the HIR surface type states.

use crate::{
    CallableValidatedCrossConeHirFrontSections, ConstValidatedCrossConeHirFrontSections,
    NominalValidatedCrossConeHirFrontSections, PropertyValidatedCrossConeHirFrontSections,
    SourceInterfaceValidatedCrossConeHirFrontSections, TypeAliasValidatedCrossConeHirFrontSections,
    cross_cone_hir_authority::ValidatedNominalProviderView,
};

pub(crate) trait NominalProviderFront {
    fn nominal_provider_view(&self) -> ValidatedNominalProviderView<'_>;
}

macro_rules! impl_nominal_provider_front {
    ($($state:ident),+ $(,)?) => {
        $(
            impl NominalProviderFront for $state<'_> {
                fn nominal_provider_view(&self) -> ValidatedNominalProviderView<'_> {
                    self.nominal_provider_view()
                }
            }
        )+
    };
}

impl_nominal_provider_front!(
    NominalValidatedCrossConeHirFrontSections,
    PropertyValidatedCrossConeHirFrontSections,
    CallableValidatedCrossConeHirFrontSections,
    TypeAliasValidatedCrossConeHirFrontSections,
    SourceInterfaceValidatedCrossConeHirFrontSections,
    ConstValidatedCrossConeHirFrontSections,
);

pub(crate) fn nominal_dependencies<'a, T: NominalProviderFront>(
    position: usize,
    dependency_positions: &[Vec<usize>],
    validated: &'a [T],
) -> Result<Vec<ValidatedNominalProviderView<'a>>, usize> {
    let reachable = transitive_dependency_positions(position, dependency_positions);
    let mut dependencies = Vec::new();
    dependencies
        .try_reserve_exact(reachable.len())
        .map_err(|_| reachable.len())?;
    dependencies.extend(
        reachable
            .into_iter()
            .map(|dependency| validated[dependency].nominal_provider_view()),
    );
    Ok(dependencies)
}

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

#[cfg(test)]
pub(crate) fn transitive_positions_for_test(
    position: usize,
    dependency_positions: &[Vec<usize>],
) -> Vec<usize> {
    transitive_dependency_positions(position, dependency_positions)
}
