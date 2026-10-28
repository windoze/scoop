//! Immutable direct package bindings shared by qualified source lookups.

use super::*;
use crate::Lowerer;
use scoop_identity::{CanonicalIdentifier, PackagePath};

pub(super) type DirectPackageIndex = BTreeMap<PackagePath, Vec<DirectPackageBindings>>;

pub(super) struct DirectPackageBindings {
    pub provider: scoop_identity::ConeIdentity,
    pub groups: Vec<hir::DirectNamedPublicBindingGroup>,
}

pub(crate) struct QualifiedPackagePrefix {
    pub(crate) current: Option<PackageId>,
    pub(crate) path: PackagePath,
}

impl QualifiedPackagePrefix {
    pub(crate) fn consumed(&self) -> usize {
        self.path.segments().len()
    }

    pub(crate) fn name(&self) -> String {
        self.path
            .segments()
            .iter()
            .map(|segment| segment.as_str())
            .collect::<Vec<_>>()
            .join(".")
    }
}

impl CurrentUnitImports {
    pub(super) fn collect_direct_packages(
        &mut self,
        world: Option<&hir::ImportedSemanticWorld<'_>>,
    ) {
        let mut packages = DirectPackageIndex::new();
        if let Some(world) = world {
            for path in world.direct_packages().paths() {
                let package = world
                    .direct_package(path)
                    .expect("an indexed package belongs to the same semantic world");
                let contributions = package
                    .provider_ids()
                    .map(|provider| DirectPackageBindings {
                        provider,
                        groups: package.snapshot_filtered(|candidate| candidate == provider),
                    })
                    .collect();
                // A package with only values still owns its prefix.
                packages.insert(path.clone(), contributions);
            }
        }
        self.direct_packages = std::sync::Arc::new(packages);
    }
}

impl Lowerer {
    pub(crate) fn qualified_package_prefix(
        &self,
        path: &[ast::Ident],
    ) -> Option<QualifiedPackagePrefix> {
        let (current, current_length) = self
            .top_level_namespaces
            .longest_package_prefix(self.current_file, path);
        let segments = path
            .iter()
            .map(|segment| {
                CanonicalIdentifier::new(&segment.text).expect("source identifiers are canonical")
            })
            .collect::<Vec<_>>();
        let direct = (1..=segments.len()).rev().find_map(|length| {
            let prefix = PackagePath::from_segments(segments[..length].to_vec());
            self.imports
                .direct_packages
                .get_key_value(&prefix)
                .filter(|(_, contributions)| {
                    contributions
                        .iter()
                        .any(|contribution| self.dependency_in_source_scope(contribution.provider))
                })
                .map(|(path, _)| path)
        });
        if let Some(direct) = direct
            && direct.segments().len() >= current_length
        {
            return Some(QualifiedPackagePrefix {
                current: (direct.segments().len() == current_length).then_some(current),
                path: direct.clone(),
            });
        }
        (current_length != 0).then(|| QualifiedPackagePrefix {
            current: Some(current),
            path: PackagePath::from_segments(segments[..current_length].to_vec()),
        })
    }
}
