use super::{PackageId, PackageNode, TopLevelLookupLayer, TopLevelNamespaces};
use scoop_ast as ast;
use scoop_identity::SourceRole;

impl TopLevelNamespaces {
    pub(crate) fn initialize_sources(
        &mut self,
        source_cones: impl IntoIterator<
            Item = (scoop_identity::ConeIdentity, scoop_identity::SourceRole),
        >,
        current_cone: scoop_identity::ConeIdentity,
        files: &[ast::SourceFile],
    ) {
        assert!(
            self.source_namespaces.is_empty(),
            "source namespaces are initialized exactly once"
        );
        let source_cones = source_cones.into_iter().collect::<Vec<_>>();
        assert_eq!(source_cones.len(), files.len());
        for ((cone, role), file) in source_cones.into_iter().zip(files) {
            let namespace = if cone == current_cone {
                let package = if cone == scoop_identity::ConeIdentity::SINGLE_FILE {
                    Self::root_package()
                } else {
                    self.intern_package(&file.package)
                };
                self.current.entry(package).or_default();
                if role == scoop_identity::SourceRole::Production {
                    let mut ancestor = Some(package);
                    while let Some(package) = ancestor {
                        self.production_packages.insert(package);
                        ancestor = self.packages[package.0 as usize].parent;
                    }
                }
                TopLevelLookupLayer::CurrentPackage(package)
            } else {
                TopLevelLookupLayer::CorePrelude
            };
            self.source_namespaces.push(namespace);
            self.source_roles.push(role);
        }
    }

    fn intern_package(&mut self, package: &ast::PackageSyntax) -> PackageId {
        let ast::PackageSyntax::QualifiedPackage { path, .. } = package else {
            return Self::root_package();
        };
        let mut parent = Self::root_package();
        for identifier in path.segments() {
            let key = (parent, identifier.text.clone());
            parent = if let Some(package) = self.package_children.get(&key) {
                *package
            } else {
                let package = PackageId(
                    u32::try_from(self.packages.len()).expect("package identity space exhausted"),
                );
                self.packages.push(PackageNode {
                    parent: Some(parent),
                    segment: Some(identifier.text.clone()),
                });
                self.package_children.insert(key, package);
                package
            };
        }
        parent
    }

    pub(crate) fn source_namespace(&self, file: usize) -> TopLevelLookupLayer {
        self.source_namespaces[file]
    }

    pub(crate) fn sources_share_namespace(&self, left: usize, right: usize) -> bool {
        self.source_namespace(left) == self.source_namespace(right)
    }

    /// Returns the unqualified lookup priority of `candidate_file` from
    /// `reference_file`. A missing rank means that the candidate belongs to
    /// another current-unit package and is not visible through these layers.
    pub(crate) fn source_lookup_rank(
        &self,
        reference_file: usize,
        candidate_file: usize,
    ) -> Option<usize> {
        if !self.source_in_scope(reference_file, candidate_file) {
            return None;
        }
        let reference = self.source_namespace(reference_file);
        let candidate = self.source_namespace(candidate_file);
        match (reference, candidate) {
            (TopLevelLookupLayer::CorePrelude, TopLevelLookupLayer::CorePrelude) => Some(0),
            (
                TopLevelLookupLayer::CurrentPackage(reference),
                TopLevelLookupLayer::CurrentPackage(candidate),
            ) if reference == candidate => Some(0),
            (TopLevelLookupLayer::CurrentPackage(_), TopLevelLookupLayer::CorePrelude) => Some(1),
            _ => None,
        }
    }

    pub(crate) fn package_segments(&self, package: PackageId) -> Vec<&str> {
        let mut reversed = Vec::new();
        let mut current = Some(package);
        while let Some(id) = current {
            let node = &self.packages[id.0 as usize];
            if let Some(segment) = node.segment.as_deref() {
                reversed.push(segment);
            }
            current = node.parent;
        }
        reversed.reverse();
        reversed
    }

    pub(crate) fn longest_package_prefix(
        &self,
        file: usize,
        path: &[ast::Ident],
    ) -> (PackageId, usize) {
        let mut package = Self::root_package();
        let mut length = 0;
        for segment in path {
            let Some(child) = self
                .package_children
                .get(&(package, segment.text.clone()))
                .copied()
            else {
                break;
            };
            if self.source_roles[file] == scoop_identity::SourceRole::Production
                && !self.production_packages.contains(&child)
            {
                break;
            }
            package = child;
            length += 1;
        }
        (package, length)
    }

    pub(crate) fn source_in_scope(&self, reference: usize, declaration: usize) -> bool {
        self.source_roles[reference] == SourceRole::Test
            || self.source_roles[declaration] == SourceRole::Production
    }
}
