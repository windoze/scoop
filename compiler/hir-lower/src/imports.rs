//! Current-unit import collection, transactional resolution and frozen scopes.

use crate::namespace::PackageId;
use scoop_ast as ast;
use scoop_hir as hir;
use std::collections::{BTreeMap, HashMap};

mod bindings;
mod collect;
pub(crate) mod lookup;
mod packages;
mod reexports;
mod resolve;
mod selector;
#[cfg(test)]
mod tests;

pub(crate) use bindings::*;
pub(crate) use reexports::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImportLookupLayer {
    Exact,
    CurrentPackage(PackageId),
    QualifiedPackage,
    Star,
    CorePrelude,
}

#[derive(Debug, Clone)]
pub(crate) struct ImportCandidateLayer {
    pub(crate) kind: ImportLookupLayer,
    pub(crate) bindings: Vec<CurrentUnitBindingId>,
    pub(crate) dependency_bindings: Vec<hir::DirectImportedTargetBinding>,
    /// Duplicate-signature functions which occupy this lookup layer but are
    /// forbidden from becoming semantic candidates.
    pub(crate) suppressed_callables: Vec<hir::FunctionId>,
    /// Declaration-side values which occupied this layer but could not be
    /// materialized because that declaration was already diagnosed. Body
    /// lookup uses these typed origins only to prevent fallthrough; they are
    /// never exposed as semantic candidates or written to HIR.
    pub(crate) suppressed_values: Vec<CurrentUnitBindingId>,
}

pub(crate) struct ImportDeclarationInputs<'a> {
    pub(crate) functions: &'a [(hir::FunctionId, &'a ast::FunctionDecl, usize)],
    pub(crate) methods: &'a [(hir::FunctionId, &'a ast::FunctionDecl, usize, crate::Owner)],
    pub(crate) properties: &'a [(&'a ast::GlobalDecl, usize)],
    pub(crate) enumerations: &'a [(hir::EnumId, &'a ast::EnumDecl, usize)],
    pub(crate) objects: &'a [(hir::ObjectId, crate::declarations::ObjectSource<'a>, usize)],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SuppressedValueScope {
    Exact { file: usize },
    Namespace(ResolvedNamespace),
    Star { file: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SuppressedValueLookup {
    scope: SuppressedValueScope,
    name: String,
}

#[derive(Clone, Default)]
struct ImportDiagnosticSuppressions {
    unmaterialized_values: HashMap<SuppressedValueLookup, Vec<CurrentUnitBindingId>>,
}

impl ImportDiagnosticSuppressions {
    fn insert(&mut self, scope: SuppressedValueScope, name: String, binding: CurrentUnitBindingId) {
        let bindings = self
            .unmaterialized_values
            .entry(SuppressedValueLookup { scope, name })
            .or_default();
        if !bindings.contains(&binding) {
            bindings.push(binding);
        }
    }

    fn get(&self, scope: SuppressedValueScope, name: &str) -> Vec<CurrentUnitBindingId> {
        self.unmaterialized_values
            .get(&SuppressedValueLookup {
                scope,
                name: name.to_string(),
            })
            .cloned()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourcePropertyOwner {
    TopLevel,
    Object(hir::ObjectId),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum PropertyImportSource {
    OutsideCurrentUnitSurface,
    CurrentUnit(SourcePropertyId),
}

#[derive(Debug, Clone)]
struct SourceVariant {
    enumeration: hir::EnumId,
    index: u32,
}

#[derive(Clone, Default)]
pub(crate) struct CurrentUnitImports {
    pub(crate) bindings: Vec<CurrentUnitBinding>,
    namespaces: HashMap<ResolvedNamespace, BTreeMap<String, Vec<CurrentUnitBindingId>>>,
    static_targets: HashMap<CurrentUnitBindingId, StaticNamespace>,
    direct_package_types: std::sync::Arc<packages::DirectPackageTypeIndex>,
    source_property_count: usize,
    source_extension_properties: std::collections::HashSet<SourcePropertyId>,
    global_property_sources: Vec<PropertyImportSource>,
    object_property_sources: HashMap<(hir::ObjectId, usize), SourcePropertyId>,
    resolved_properties: HashMap<SourcePropertyId, hir::PropertyId>,
    source_variants: Vec<SourceVariant>,
    enum_variant_sources: HashMap<(hir::EnumId, u32), SourceVariantId>,
    resolved_variants: HashMap<SourceVariantId, hir::EnumVariantRef>,
    prelude_dependencies: BTreeMap<
        scoop_identity::BindingNamespace,
        BTreeMap<String, Vec<hir::DirectImportedTargetBinding>>,
    >,
    pub(crate) files: Vec<FrozenFileImports>,
    pub(crate) reexports: Vec<FrozenReexportBinding>,
    diagnostic_suppressions: ImportDiagnosticSuppressions,
}

impl CurrentUnitImports {
    pub(crate) fn prelude_bindings(
        &self,
        namespace: scoop_identity::BindingNamespace,
        name: &str,
    ) -> &[hir::DirectImportedTargetBinding] {
        self.prelude_dependencies
            .get(&namespace)
            .and_then(|bindings| bindings.get(name))
            .map_or(&[], Vec::as_slice)
    }

    pub(crate) fn global_property_source(&self, declaration_index: usize) -> PropertyImportSource {
        self.global_property_sources[declaration_index]
    }

    pub(crate) fn object_property_source(
        &self,
        object: hir::ObjectId,
        member_index: usize,
        outside_current_cone: bool,
    ) -> PropertyImportSource {
        if outside_current_cone {
            PropertyImportSource::OutsideCurrentUnitSurface
        } else {
            PropertyImportSource::CurrentUnit(self.object_property_sources[&(object, member_index)])
        }
    }

    pub(crate) fn bind_property(
        &mut self,
        source: PropertyImportSource,
        property: hir::PropertyId,
    ) {
        if let PropertyImportSource::CurrentUnit(source) = source {
            assert!(
                self.resolved_properties.insert(source, property).is_none(),
                "a source property is resolved exactly once"
            );
        }
    }

    pub(crate) fn bind_variant(
        &mut self,
        enumeration: hir::EnumId,
        source_index: u32,
        target: hir::EnumVariantRef,
    ) {
        let Some(&source) = self.enum_variant_sources.get(&(enumeration, source_index)) else {
            return;
        };
        let declaration = &self.source_variants[source.0];
        assert_eq!(declaration.enumeration, enumeration);
        assert_eq!(declaration.index, source_index);
        assert_eq!(
            target.enumeration(),
            enumeration,
            "a resolved variant retains its source enum owner"
        );
        assert!(
            self.resolved_variants.insert(source, target).is_none(),
            "a source variant is resolved exactly once"
        );
    }

    pub(crate) fn binding(&self, id: CurrentUnitBindingId) -> &CurrentUnitBinding {
        &self.bindings[id.0]
    }

    fn insert(
        &mut self,
        namespace: ResolvedNamespace,
        binding: CurrentUnitBinding,
    ) -> CurrentUnitBindingId {
        let id = CurrentUnitBindingId(self.bindings.len());
        self.namespaces
            .entry(namespace)
            .or_default()
            .entry(binding.name.clone())
            .or_default()
            .push(id);
        self.bindings.push(binding);
        id
    }

    fn canonicalize(
        &self,
        bindings: impl IntoIterator<Item = CurrentUnitBindingId>,
    ) -> Vec<CurrentUnitBindingId> {
        let mut bindings = bindings.into_iter().collect::<Vec<_>>();
        bindings.sort_by_key(|id| {
            let binding = self.binding(*id);
            (
                binding.source.clone(),
                binding.span.start,
                binding.span.end,
                id.0,
            )
        });
        bindings.dedup();
        bindings
    }

    fn canonicalize_dependencies(
        &self,
        bindings: impl IntoIterator<Item = hir::DirectImportedTargetBinding>,
    ) -> Vec<hir::DirectImportedTargetBinding> {
        let mut canonical = BTreeMap::new();
        for binding in bindings {
            match canonical.entry(binding.binding_target()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(binding);
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => entry
                    .get_mut()
                    .try_merge(binding)
                    .expect("one frozen import scope belongs to one semantic world"),
            }
        }
        canonical.into_values().collect()
    }

    /// Core targets remain in the existing prelude index; the final empty
    /// layer is an explicit handoff to that complete M22 backing view.
    pub(crate) fn layers(
        &self,
        file: usize,
        package: PackageId,
        name: &str,
    ) -> Vec<ImportCandidateLayer> {
        let imports = &self.files[file];
        vec![
            ImportCandidateLayer {
                kind: ImportLookupLayer::Exact,
                bindings: self.canonicalize(
                    imports
                        .exact
                        .iter()
                        .filter(|import| import.local_name == name)
                        .flat_map(|import| {
                            import
                                .targets
                                .iter()
                                .filter_map(ImportedTargetBinding::current_binding)
                        }),
                ),
                dependency_bindings: self.canonicalize_dependencies(
                    imports
                        .exact
                        .iter()
                        .filter(|import| import.local_name == name)
                        .flat_map(|import| {
                            import
                                .targets
                                .iter()
                                .filter_map(ImportedTargetBinding::direct_binding)
                                .cloned()
                        }),
                ),
                suppressed_callables: Vec::new(),
                suppressed_values: self
                    .diagnostic_suppressions
                    .get(SuppressedValueScope::Exact { file }, name),
            },
            ImportCandidateLayer {
                kind: ImportLookupLayer::CurrentPackage(package),
                bindings: self.canonicalize(
                    self.namespaces
                        .get(&ResolvedNamespace::Package(package))
                        .and_then(|namespace| namespace.get(name))
                        .into_iter()
                        .flatten()
                        .copied(),
                ),
                dependency_bindings: self.canonicalize_dependencies(
                    imports
                        .current_package_dependencies
                        .get(name)
                        .into_iter()
                        .flatten()
                        .cloned(),
                ),
                suppressed_callables: Vec::new(),
                suppressed_values: self.diagnostic_suppressions.get(
                    SuppressedValueScope::Namespace(ResolvedNamespace::Package(package)),
                    name,
                ),
            },
            ImportCandidateLayer {
                kind: ImportLookupLayer::Star,
                bindings: self.canonicalize(imports.stars.iter().flat_map(|import| {
                    import
                        .snapshot
                        .get(name)
                        .into_iter()
                        .flat_map(ast::NonEmptyVec::iter)
                        .filter_map(ImportedTargetBinding::current_binding)
                })),
                dependency_bindings: self.canonicalize_dependencies(imports.stars.iter().flat_map(
                    |import| {
                        import
                            .snapshot
                            .get(name)
                            .into_iter()
                            .flat_map(ast::NonEmptyVec::iter)
                            .filter_map(ImportedTargetBinding::direct_binding)
                            .cloned()
                    },
                )),
                suppressed_callables: Vec::new(),
                suppressed_values: self
                    .diagnostic_suppressions
                    .get(SuppressedValueScope::Star { file }, name),
            },
            ImportCandidateLayer {
                kind: ImportLookupLayer::CorePrelude,
                bindings: Vec::new(),
                dependency_bindings: Vec::new(),
                suppressed_callables: Vec::new(),
                suppressed_values: Vec::new(),
            },
        ]
    }
}
