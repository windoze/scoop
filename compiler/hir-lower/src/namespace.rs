//! Request-local top-level package namespace used by HIR lowering.
//!
//! M23-1 deliberately keeps this identity out of export HIR.  A package id is
//! only a typed key for one lowering request; declaration arena ids remain the
//! entity identities.

use std::collections::HashMap;

use scoop_ast as ast;
use scoop_hir as hir;

use crate::{NominalTarget, SourceKind, aliases::SourceTypeAliasId};

pub(crate) const fn is_file_private(syntax: ast::VisibilitySyntax) -> bool {
    matches!(
        syntax,
        ast::VisibilitySyntax::Explicit {
            visibility: ast::DeclaredVisibility::Private,
            ..
        }
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct PackageId(u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum TopLevelLookupLayer {
    CorePrelude,
    CurrentPackage(PackageId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TopLevelCandidateLayer<T> {
    pub(crate) kind: TopLevelLookupLayer,
    pub(crate) candidates: Vec<T>,
}

impl<T> IntoIterator for TopLevelCandidateLayer<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.candidates.into_iter()
    }
}

impl<'a, T> IntoIterator for &'a TopLevelCandidateLayer<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.candidates.iter()
    }
}

impl<T> std::ops::Deref for TopLevelCandidateLayer<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.candidates
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TopLevelTypeTarget {
    Alias(SourceTypeAliasId),
    Nominal(NominalTarget),
}

impl TopLevelTypeTarget {
    pub(crate) const fn description(self) -> &'static str {
        match self {
            Self::Alias(_) => "a typealias",
            Self::Nominal(NominalTarget::Struct(_)) => "a struct",
            Self::Nominal(NominalTarget::Enum(_)) => "an enum",
            Self::Nominal(NominalTarget::Class(_)) => "a class",
            Self::Nominal(NominalTarget::Interface(_)) => "an interface",
            Self::Nominal(NominalTarget::Object(_)) => "an object",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct TypeBinding {
    target: TopLevelTypeTarget,
    file: usize,
    file_private: bool,
}

#[derive(Clone, Default)]
struct NamespaceBindings {
    types: HashMap<String, Vec<TypeBinding>>,
    functions: HashMap<String, Vec<hir::FunctionId>>,
    extensions: HashMap<String, Vec<hir::FunctionId>>,
    properties: HashMap<String, Vec<hir::PropertyId>>,
    extension_properties: HashMap<String, Vec<hir::PropertyId>>,
}

#[derive(Clone)]
struct PackageNode {
    parent: Option<PackageId>,
    segment: Option<String>,
}

#[derive(Clone)]
pub(crate) struct TopLevelNamespaces {
    packages: Vec<PackageNode>,
    package_children: HashMap<(PackageId, String), PackageId>,
    source_namespaces: Vec<TopLevelLookupLayer>,
    current: HashMap<PackageId, NamespaceBindings>,
    core_prelude: NamespaceBindings,
}

impl Default for TopLevelNamespaces {
    fn default() -> Self {
        Self {
            packages: vec![PackageNode {
                parent: None,
                segment: None,
            }],
            package_children: HashMap::new(),
            source_namespaces: Vec::new(),
            current: HashMap::new(),
            core_prelude: NamespaceBindings::default(),
        }
    }
}

impl TopLevelNamespaces {
    pub(crate) const fn root_package() -> PackageId {
        PackageId(0)
    }

    pub(crate) fn initialize_sources(
        &mut self,
        kinds: impl IntoIterator<Item = SourceKind>,
        files: &[ast::SourceFile],
    ) {
        assert!(
            self.source_namespaces.is_empty(),
            "source namespaces are initialized exactly once"
        );
        let kinds = kinds.into_iter().collect::<Vec<_>>();
        assert_eq!(kinds.len(), files.len());
        for (kind, file) in kinds.into_iter().zip(files) {
            let namespace = match kind {
                SourceKind::ExistingM22Core => TopLevelLookupLayer::CorePrelude,
                SourceKind::CurrentUnit => {
                    let package = self.intern_package(&file.package);
                    self.current.entry(package).or_default();
                    TopLevelLookupLayer::CurrentPackage(package)
                }
            };
            self.source_namespaces.push(namespace);
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

    pub(crate) fn longest_package_prefix(&self, path: &[ast::Ident]) -> (PackageId, usize) {
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
            package = child;
            length += 1;
        }
        (package, length)
    }

    fn namespace(&self, namespace: TopLevelLookupLayer) -> &NamespaceBindings {
        match namespace {
            TopLevelLookupLayer::CorePrelude => &self.core_prelude,
            TopLevelLookupLayer::CurrentPackage(package) => self
                .current
                .get(&package)
                .expect("every source package has a namespace"),
        }
    }

    fn namespace_mut(&mut self, namespace: TopLevelLookupLayer) -> &mut NamespaceBindings {
        match namespace {
            TopLevelLookupLayer::CorePrelude => &mut self.core_prelude,
            TopLevelLookupLayer::CurrentPackage(package) => self
                .current
                .get_mut(&package)
                .expect("every source package has a namespace"),
        }
    }

    fn lookup_namespaces(&self, file: usize) -> Vec<(TopLevelLookupLayer, &NamespaceBindings)> {
        match self.source_namespace(file) {
            TopLevelLookupLayer::CorePrelude => {
                vec![(TopLevelLookupLayer::CorePrelude, &self.core_prelude)]
            }
            TopLevelLookupLayer::CurrentPackage(package) => vec![
                (
                    TopLevelLookupLayer::CurrentPackage(package),
                    self.current
                        .get(&package)
                        .expect("every source package has a namespace"),
                ),
                (TopLevelLookupLayer::CorePrelude, &self.core_prelude),
            ],
        }
    }

    pub(crate) fn type_conflict(
        &self,
        file: usize,
        name: &str,
        new_is_private: bool,
    ) -> Option<TopLevelTypeTarget> {
        self.namespace(self.source_namespace(file))
            .types
            .get(name)?
            .iter()
            .find(|binding| !new_is_private || !binding.file_private || binding.file == file)
            .map(|binding| binding.target)
    }

    pub(crate) fn register_type(
        &mut self,
        file: usize,
        name: String,
        target: TopLevelTypeTarget,
        file_private: bool,
    ) {
        let namespace = self.source_namespace(file);
        self.namespace_mut(namespace)
            .types
            .entry(name)
            .or_default()
            .push(TypeBinding {
                target,
                file,
                file_private,
            });
    }

    pub(crate) fn type_layers(
        &self,
        file: usize,
        name: &str,
    ) -> Vec<TopLevelCandidateLayer<TopLevelTypeTarget>> {
        self.lookup_namespaces(file)
            .into_iter()
            .map(|(kind, namespace)| TopLevelCandidateLayer {
                kind,
                candidates: namespace
                    .types
                    .get(name)
                    .into_iter()
                    .flatten()
                    .map(|binding| binding.target)
                    .collect(),
            })
            .collect()
    }

    pub(crate) fn package_types(&self, package: PackageId, name: &str) -> Vec<TopLevelTypeTarget> {
        self.current
            .get(&package)
            .and_then(|namespace| namespace.types.get(name))
            .into_iter()
            .flatten()
            .map(|binding| binding.target)
            .collect()
    }

    pub(crate) fn core_type(&self, name: &str) -> Option<TopLevelTypeTarget> {
        self.core_prelude
            .types
            .get(name)
            .and_then(|bindings| bindings.first())
            .map(|binding| binding.target)
    }

    pub(crate) fn register_function(
        &mut self,
        file: usize,
        name: String,
        function: hir::FunctionId,
        extension: bool,
    ) {
        let namespace = self.source_namespace(file);
        let bindings = self.namespace_mut(namespace);
        let index = if extension {
            &mut bindings.extensions
        } else {
            &mut bindings.functions
        };
        index.entry(name).or_default().push(function);
    }

    pub(crate) fn function_layers(
        &self,
        file: usize,
        name: &str,
    ) -> Vec<TopLevelCandidateLayer<hir::FunctionId>> {
        self.lookup_namespaces(file)
            .into_iter()
            .map(|(kind, namespace)| TopLevelCandidateLayer {
                kind,
                candidates: namespace.functions.get(name).cloned().unwrap_or_default(),
            })
            .collect()
    }

    pub(crate) fn extension_layers(
        &self,
        file: usize,
        name: &str,
    ) -> Vec<TopLevelCandidateLayer<hir::FunctionId>> {
        self.lookup_namespaces(file)
            .into_iter()
            .map(|(kind, namespace)| TopLevelCandidateLayer {
                kind,
                candidates: namespace.extensions.get(name).cloned().unwrap_or_default(),
            })
            .collect()
    }

    pub(crate) fn named_callable_layers(
        &self,
        file: usize,
        name: &str,
    ) -> Vec<TopLevelCandidateLayer<hir::FunctionId>> {
        self.lookup_namespaces(file)
            .into_iter()
            .map(|(kind, namespace)| TopLevelCandidateLayer {
                kind,
                candidates: namespace
                    .functions
                    .get(name)
                    .into_iter()
                    .chain(namespace.extensions.get(name))
                    .flatten()
                    .copied()
                    .collect(),
            })
            .collect()
    }

    pub(crate) fn all_extension_layers(
        &self,
        file: usize,
    ) -> Vec<TopLevelCandidateLayer<hir::FunctionId>> {
        self.lookup_namespaces(file)
            .into_iter()
            .map(|(kind, namespace)| TopLevelCandidateLayer {
                kind,
                candidates: namespace.extensions.values().flatten().copied().collect(),
            })
            .collect()
    }

    pub(crate) fn current_unit_functions_named(&self, name: &str) -> Vec<hir::FunctionId> {
        let mut functions = self
            .current
            .values()
            .flat_map(|namespace| namespace.functions.get(name).into_iter().flatten())
            .copied()
            .collect::<Vec<_>>();
        functions.sort_by_key(|function| function.into_raw().into_u32());
        functions
    }

    pub(crate) fn register_property(
        &mut self,
        file: usize,
        name: String,
        property: hir::PropertyId,
        extension: bool,
    ) {
        let namespace = self.source_namespace(file);
        let bindings = self.namespace_mut(namespace);
        let index = if extension {
            &mut bindings.extension_properties
        } else {
            &mut bindings.properties
        };
        index.entry(name).or_default().push(property);
    }

    pub(crate) fn properties_in_declaration_scope(
        &self,
        file: usize,
        name: &str,
    ) -> Vec<hir::PropertyId> {
        self.namespace(self.source_namespace(file))
            .properties
            .get(name)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn property_layers(
        &self,
        file: usize,
        name: &str,
    ) -> Vec<TopLevelCandidateLayer<hir::PropertyId>> {
        self.lookup_namespaces(file)
            .into_iter()
            .map(|(kind, namespace)| TopLevelCandidateLayer {
                kind,
                candidates: namespace.properties.get(name).cloned().unwrap_or_default(),
            })
            .collect()
    }

    pub(crate) fn extension_property_layers(
        &self,
        file: usize,
        name: &str,
    ) -> Vec<TopLevelCandidateLayer<hir::PropertyId>> {
        self.lookup_namespaces(file)
            .into_iter()
            .map(|(kind, namespace)| TopLevelCandidateLayer {
                kind,
                candidates: namespace
                    .extension_properties
                    .get(name)
                    .cloned()
                    .unwrap_or_default(),
            })
            .collect()
    }

    pub(crate) fn all_extension_property_groups(&self) -> Vec<Vec<hir::PropertyId>> {
        std::iter::once(&self.core_prelude)
            .chain(self.current.values())
            .flat_map(|namespace| namespace.extension_properties.values().cloned())
            .collect()
    }
}
