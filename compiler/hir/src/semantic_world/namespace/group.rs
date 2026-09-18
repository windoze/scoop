use std::collections::BTreeMap;

use scoop_identity::{BindingNamespace, BindingTarget, CanonicalIdentifier};

use super::super::{DirectImportedTargetBinding, ImportedPublicBinding};

/// Non-empty public binding group at an exact package or static-owner name.
/// Raw bindings remain available for diagnostics; semantic targets are
/// folded by kind-specific persistent origin and retain every source route.
pub struct DirectPublicBindingGroup<'world, 'input> {
    bindings: Vec<&'world ImportedPublicBinding<'input>>,
    targets: Vec<DirectImportedTargetBinding>,
}

#[derive(Clone, Debug)]
pub struct DirectNamedPublicBindingGroup {
    namespace: BindingNamespace,
    name: CanonicalIdentifier,
    targets: Vec<DirectImportedTargetBinding>,
}

impl DirectNamedPublicBindingGroup {
    pub const fn namespace(&self) -> BindingNamespace {
        self.namespace
    }

    pub const fn name(&self) -> &CanonicalIdentifier {
        &self.name
    }

    pub fn targets(&self) -> &[DirectImportedTargetBinding] {
        &self.targets
    }
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

    let targets = normalized_targets(&bindings);
    Some(DirectPublicBindingGroup { bindings, targets })
}

pub(super) fn named_groups<'world, 'input>(
    bindings: impl IntoIterator<Item = &'world ImportedPublicBinding<'input>>,
) -> Vec<DirectNamedPublicBindingGroup>
where
    'input: 'world,
{
    let mut groups = BTreeMap::<(BindingNamespace, CanonicalIdentifier), Vec<_>>::new();
    for binding in bindings {
        if !binding.lookup_sources().is_empty() {
            groups
                .entry((binding.key().namespace(), binding.key().name().clone()))
                .or_default()
                .push(binding);
        }
    }
    groups
        .into_iter()
        .map(
            |((namespace, name), bindings)| DirectNamedPublicBindingGroup {
                namespace,
                name,
                targets: normalized_targets(&bindings),
            },
        )
        .collect()
}

fn normalized_targets(bindings: &[&ImportedPublicBinding<'_>]) -> Vec<DirectImportedTargetBinding> {
    let mut targets = BTreeMap::<BindingTarget, _>::new();
    for binding in bindings {
        let binding_target = binding.key().binding_target();
        let entry = targets
            .entry(binding_target)
            .or_insert_with(|| (binding.target(), Vec::new()));
        entry.1.extend(binding.lookup_sources().iter().cloned());
    }
    targets
        .into_iter()
        .map(|(binding_target, (target, sources))| {
            DirectImportedTargetBinding::new(binding_target, target, sources)
        })
        .collect()
}
