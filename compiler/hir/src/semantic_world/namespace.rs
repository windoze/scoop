use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{BindingNamespace, CanonicalIdentifier, ConeIdentity, PackagePath};

use super::{
    DirectNamespaceLookupError, ImportedNominal, ImportedPublicBinding, ImportedSemanticWorld,
    provider::ImportedProvider,
};
use crate::SourceNominalId;

mod group;
pub use group::{DirectNamedPublicBindingGroup, DirectPublicBindingGroup};
use group::{named_groups, non_empty_group};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BindingLocator {
    provider: ConeIdentity,
    binding: usize,
}

#[derive(Clone, Debug)]
struct DirectPackageContribution {
    provider: ConeIdentity,
    bindings: Vec<BindingLocator>,
}

#[derive(Clone, Debug)]
struct DirectPackageEntry {
    contributions: Vec<DirectPackageContribution>,
}

/// Canonical package index containing contributions from direct providers
/// only. Support providers are absent even though their exact typed entities
/// remain available to route/static-owner traversal.
pub struct DirectPackageIndex {
    entries: BTreeMap<PackagePath, DirectPackageEntry>,
}

impl DirectPackageIndex {
    pub fn paths(&self) -> impl ExactSizeIterator<Item = &PackagePath> {
        self.entries.keys()
    }

    pub fn package_count(&self) -> usize {
        self.entries.len()
    }

    pub fn contains(&self, path: &PackagePath) -> bool {
        self.entries.contains_key(path)
    }

    pub fn longest_prefix<'index>(
        &'index self,
        segments: &[CanonicalIdentifier],
    ) -> Option<DirectPackageMatch<'index>> {
        (0..=segments.len()).rev().find_map(|consumed| {
            let path = PackagePath::from_segments(segments[..consumed].to_vec());
            self.entries
                .get_key_value(&path)
                .map(|(path, entry)| DirectPackageMatch {
                    path,
                    consumed,
                    provider_count: entry.contributions.len(),
                })
        })
    }
}

/// Longest visible package-prefix result. It deliberately contains only
/// direct provider handles and no support-provider enumeration channel.
#[derive(Clone, Copy)]
pub struct DirectPackageMatch<'index> {
    path: &'index PackagePath,
    consumed: usize,
    provider_count: usize,
}

impl<'index> DirectPackageMatch<'index> {
    pub const fn path(self) -> &'index PackagePath {
        self.path
    }

    pub const fn consumed_segments(self) -> usize {
        self.consumed
    }

    pub const fn provider_count(self) -> usize {
        self.provider_count
    }
}

pub(super) fn build_direct_package_index(providers: &[ImportedProvider<'_>]) -> DirectPackageIndex {
    let mut pending = BTreeMap::<PackagePath, BTreeMap<ConeIdentity, Vec<BindingLocator>>>::new();
    for provider in providers.iter().filter(|provider| provider.is_direct()) {
        let provider_id = provider.identity();
        for (binding_index, binding) in provider.public_bindings().iter().enumerate() {
            if !provider.is_package_binding(binding.identity().persistent()) {
                continue;
            }
            pending
                .entry(binding.key().package().clone())
                .or_default()
                .entry(provider_id)
                .or_default()
                .push(BindingLocator {
                    provider: provider_id,
                    binding: binding_index,
                });
        }
    }
    let entries = pending
        .into_iter()
        .map(|(path, contributions)| {
            let contributions = contributions
                .into_iter()
                .map(|(provider, bindings)| DirectPackageContribution { provider, bindings })
                .collect();
            (path, DirectPackageEntry { contributions })
        })
        .collect();
    DirectPackageIndex { entries }
}

/// Direct package namespace selected by longest-prefix lookup.
pub struct DirectPackageView<'world, 'input> {
    world: &'world ImportedSemanticWorld<'input>,
    path: &'world PackagePath,
    entry: &'world DirectPackageEntry,
}

impl<'world, 'input> DirectPackageView<'world, 'input> {
    pub const fn path(&self) -> &'world PackagePath {
        self.path
    }

    pub fn contribution_count(&self) -> usize {
        self.entry.contributions.len()
    }

    pub fn provider_ids(&self) -> impl ExactSizeIterator<Item = ConeIdentity> + '_ {
        self.entry
            .contributions
            .iter()
            .map(|contribution| contribution.provider)
    }

    pub fn bindings(&self) -> impl Iterator<Item = &ImportedPublicBinding<'input>> + '_ {
        self.entry
            .contributions
            .iter()
            .flat_map(|contribution| contribution.bindings.iter())
            .map(|locator| {
                self.world
                    .binding(*locator)
                    .expect("the direct package index contains checked binding locators")
            })
    }

    pub fn binding_group(
        &self,
        namespace: BindingNamespace,
        name: &str,
    ) -> Option<DirectPublicBindingGroup<'world, 'input>> {
        non_empty_group(
            self.entry
                .contributions
                .iter()
                .flat_map(|contribution| contribution.bindings.iter())
                .filter_map(|locator| self.world.binding(*locator))
                .filter(|binding| {
                    binding.key().namespace() == namespace && binding.key().name().as_str() == name
                })
                .collect(),
        )
    }

    pub fn snapshot(&self) -> Vec<DirectNamedPublicBindingGroup> {
        named_groups(self.bindings())
    }

    /// Filters providers before folding targets, so a lookup layer retains
    /// exactly its own routes when another layer provides the same entity.
    pub fn snapshot_filtered(
        &self,
        include: impl Fn(ConeIdentity) -> bool,
    ) -> Vec<DirectNamedPublicBindingGroup> {
        named_groups(
            self.bindings()
                .filter(|binding| include(binding.provider())),
        )
    }
}

/// Public nested namespace of one exact imported nominal owner.
pub struct ImportedStaticNamespace<'world, 'input> {
    world: &'world ImportedSemanticWorld<'input>,
    owner: SourceNominalId,
}

impl<'world, 'input> ImportedStaticNamespace<'world, 'input> {
    pub fn owner(&self) -> ImportedNominal<'input> {
        self.world
            .nominal(self.owner)
            .expect("a static namespace is constructed only for an indexed nominal")
    }

    pub fn bindings(&self) -> impl Iterator<Item = &ImportedPublicBinding<'input>> + '_ {
        let owner = self.owner();
        let provider = self
            .world
            .provider(owner.provider())
            .expect("the nominal provider belongs to this world");
        owner
            .record()
            .nested_bindings()
            .values()
            .iter()
            .filter_map(|binding| provider.binding_by_id(*binding))
    }

    pub fn binding_group(
        &self,
        namespace: BindingNamespace,
        name: &str,
    ) -> Option<DirectPublicBindingGroup<'world, 'input>> {
        self.direct_binding_group(namespace, name).or_else(|| {
            self.companion_namespace()?
                .direct_binding_group(namespace, name)
        })
    }

    fn direct_binding_group(
        &self,
        namespace: BindingNamespace,
        name: &str,
    ) -> Option<DirectPublicBindingGroup<'world, 'input>> {
        let owner = self.owner();
        let provider = self.world.provider(owner.provider())?;
        non_empty_group(
            owner
                .record()
                .nested_bindings()
                .values()
                .iter()
                .filter_map(|binding| provider.binding_by_id(*binding))
                .filter(|binding| {
                    binding.key().namespace() == namespace && binding.key().name().as_str() == name
                })
                .collect(),
        )
    }

    fn companion_namespace(&self) -> Option<ImportedStaticNamespace<'world, 'input>> {
        self.bindings()
            .filter_map(|binding| binding.target().source_nominal())
            .find(|owner| {
                self.world.nominal(*owner).is_some_and(|nominal| {
                matches!(nominal.record().source_shape(), crate::NominalSourceShapeV1::Object(shape)
                    if shape.object_kind() == crate::ObjectSourceKindV1::Companion)
            })
            })
            .map(|owner| ImportedStaticNamespace {
                world: self.world,
                owner,
            })
    }

    pub fn snapshot(&self) -> Vec<DirectNamedPublicBindingGroup> {
        let mut groups = named_groups(self.bindings());
        if let Some(companion) = self.companion_namespace() {
            for group in named_groups(companion.bindings()) {
                if !groups
                    .iter()
                    .any(|own| own.namespace() == group.namespace() && own.name() == group.name())
                {
                    groups.push(group);
                }
            }
            groups.sort_by(|left, right| {
                (left.namespace(), left.name()).cmp(&(right.namespace(), right.name()))
            });
        }
        groups
    }
}

pub enum DirectNamespaceView<'world, 'input> {
    Package(DirectPackageView<'world, 'input>),
    Static(ImportedStaticNamespace<'world, 'input>),
}

impl<'world, 'input> DirectNamespaceView<'world, 'input> {
    pub fn binding_group(
        &self,
        namespace: BindingNamespace,
        name: &str,
    ) -> Option<DirectPublicBindingGroup<'world, 'input>> {
        match self {
            Self::Package(view) => view.binding_group(namespace, name),
            Self::Static(view) => view.binding_group(namespace, name),
        }
    }

    pub fn snapshot(&self) -> Vec<DirectNamedPublicBindingGroup> {
        match self {
            Self::Package(namespace) => namespace.snapshot(),
            Self::Static(namespace) => namespace.snapshot(),
        }
    }
}

impl<'input> ImportedSemanticWorld<'input> {
    pub fn direct_package(&self, path: &PackagePath) -> Option<DirectPackageView<'_, 'input>> {
        let (path, entry) = self.direct_packages.entries.get_key_value(path)?;
        Some(DirectPackageView {
            world: self,
            path,
            entry,
        })
    }

    /// Reopens the public nested namespace of an already selected nominal.
    ///
    /// The caller must first hold the typed nominal identity; this method does
    /// not enumerate support providers or create a name-lookup path to them.
    pub fn imported_static_namespace(
        &self,
        owner: SourceNominalId,
    ) -> Option<ImportedStaticNamespace<'_, 'input>> {
        self.entities
            .nominal_provider(owner)
            .map(|_| ImportedStaticNamespace { world: self, owner })
    }

    /// Resolves an exact import/qualified selector using one fixed longest
    /// package prefix, followed only by typed static-owner edges.
    pub fn resolve_direct_exact(
        &self,
        segments: &[CanonicalIdentifier],
        namespace: BindingNamespace,
    ) -> Result<DirectPublicBindingGroup<'_, 'input>, DirectNamespaceLookupError> {
        let (consumed, mut cursor) = self.longest_package_cursor(segments)?;
        let remaining = &segments[consumed..];
        let Some((last, owners)) = remaining.split_last() else {
            return Err(DirectNamespaceLookupError::ExpectedBindingAfterPackage);
        };
        for (offset, owner) in owners.iter().enumerate() {
            cursor = self.descend_static(cursor, owner, consumed + offset)?;
        }
        self.cursor_group(&cursor, namespace, last.as_str())
            .ok_or_else(|| DirectNamespaceLookupError::MissingBinding {
                segment: segments.len() - 1,
                name: last.as_str().to_owned(),
            })
    }

    /// Resolves a star-import endpoint. A full package match wins over a
    /// shorter package plus same-named static owner and is never retried.
    pub fn resolve_direct_namespace(
        &self,
        segments: &[CanonicalIdentifier],
    ) -> Result<DirectNamespaceView<'_, 'input>, DirectNamespaceLookupError> {
        let (consumed, mut cursor) = self.longest_package_cursor(segments)?;
        for (offset, owner) in segments[consumed..].iter().enumerate() {
            cursor = self.descend_static(cursor, owner, consumed + offset)?;
        }
        Ok(match cursor {
            NamespaceCursor::Package { path, entry } => {
                DirectNamespaceView::Package(DirectPackageView {
                    world: self,
                    path,
                    entry,
                })
            }
            NamespaceCursor::Static(owner) => {
                DirectNamespaceView::Static(ImportedStaticNamespace { world: self, owner })
            }
        })
    }

    fn longest_package_cursor(
        &self,
        segments: &[CanonicalIdentifier],
    ) -> Result<(usize, NamespaceCursor<'_>), DirectNamespaceLookupError> {
        let matched = self
            .direct_packages
            .longest_prefix(segments)
            .ok_or(DirectNamespaceLookupError::NoVisiblePackage)?;
        let entry = self
            .direct_packages
            .entries
            .get(matched.path())
            .expect("the longest-prefix match refers to its source package entry");
        Ok((
            matched.consumed_segments(),
            NamespaceCursor::Package {
                path: matched.path(),
                entry,
            },
        ))
    }

    fn descend_static<'world>(
        &'world self,
        cursor: NamespaceCursor<'world>,
        segment: &CanonicalIdentifier,
        segment_index: usize,
    ) -> Result<NamespaceCursor<'world>, DirectNamespaceLookupError> {
        let group = self
            .cursor_group(&cursor, BindingNamespace::Type, segment.as_str())
            .ok_or_else(|| DirectNamespaceLookupError::MissingBinding {
                segment: segment_index,
                name: segment.as_str().to_owned(),
            })?;
        let mut targets = BTreeSet::new();
        for binding in group.bindings() {
            targets.insert(binding.target().persistent());
        }
        if targets.len() != 1 {
            return Err(DirectNamespaceLookupError::AmbiguousStaticOwner {
                segment: segment_index,
                name: segment.as_str().to_owned(),
            });
        }
        let target = *targets
            .first()
            .expect("the exact binding group is structurally non-empty");
        let owner = self
            .entities
            .import_target(target)
            .and_then(|target| target.source_nominal())
            .ok_or_else(|| DirectNamespaceLookupError::NotStaticOwner {
                segment: segment_index,
                name: segment.as_str().to_owned(),
            })?;
        if self.entities.nominal_provider(owner).is_none() {
            return Err(DirectNamespaceLookupError::MissingNominalInterface(owner));
        }
        Ok(NamespaceCursor::Static(owner))
    }

    fn cursor_group<'world>(
        &'world self,
        cursor: &NamespaceCursor<'world>,
        namespace: BindingNamespace,
        name: &str,
    ) -> Option<DirectPublicBindingGroup<'world, 'input>> {
        match cursor {
            NamespaceCursor::Package { path, entry } => DirectPackageView {
                world: self,
                path,
                entry,
            }
            .binding_group(namespace, name),
            NamespaceCursor::Static(owner) => ImportedStaticNamespace {
                world: self,
                owner: *owner,
            }
            .binding_group(namespace, name),
        }
    }

    fn binding(&self, locator: BindingLocator) -> Option<&ImportedPublicBinding<'input>> {
        self.provider(locator.provider)?.binding(locator.binding)
    }
}

enum NamespaceCursor<'world> {
    Package {
        path: &'world PackagePath,
        entry: &'world DirectPackageEntry,
    },
    Static(SourceNominalId),
}
