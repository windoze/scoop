use std::collections::BTreeMap;

use scoop_identity::BindableEntity;

use super::super::{DirectImportedTargetBinding, ImportedPublicBinding};

/// Non-empty public binding group at an exact package or static-owner name.
/// Raw bindings remain available for diagnostics; semantic targets are
/// folded by kind-specific persistent origin and retain every source route.
pub struct DirectPublicBindingGroup<'world, 'input> {
    bindings: Vec<&'world ImportedPublicBinding<'input>>,
    targets: Vec<DirectImportedTargetBinding>,
}

impl<'world, 'input> DirectPublicBindingGroup<'world, 'input> {
    pub fn bindings(
        &self,
    ) -> impl ExactSizeIterator<Item = &'world ImportedPublicBinding<'input>> + '_ {
        self.bindings.iter().copied()
    }

    pub fn targets(&self) -> impl ExactSizeIterator<Item = &DirectImportedTargetBinding> + '_ {
        self.targets.iter()
    }

    pub fn len(&self) -> usize {
        self.targets.len()
    }

    pub fn binding_count(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }
}

pub(super) fn non_empty_group<'world, 'input>(
    bindings: Vec<&'world ImportedPublicBinding<'input>>,
) -> Option<DirectPublicBindingGroup<'world, 'input>> {
    let bindings = bindings
        .into_iter()
        .filter(|binding| !binding.lookup_sources().is_empty())
        .collect::<Vec<_>>();
    if bindings.is_empty() {
        return None;
    }

    let mut targets = BTreeMap::<BindableEntity, _>::new();
    for binding in &bindings {
        let entry = targets
            .entry(binding.target().persistent())
            .or_insert_with(|| (binding.target(), Vec::new()));
        entry.1.extend(binding.lookup_sources().iter().cloned());
    }
    let targets = targets
        .into_values()
        .map(|(target, sources)| DirectImportedTargetBinding::new(target, sources))
        .collect();
    Some(DirectPublicBindingGroup { bindings, targets })
}
