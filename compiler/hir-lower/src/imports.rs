//! Current-unit import collection, transactional resolution and frozen scopes.

use crate::{aliases::SourceTypeAliasId, namespace::PackageId};
use scoop_ast as ast;
use scoop_hir as hir;
use std::collections::{BTreeMap, HashMap};

mod collect;
mod resolve;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CurrentUnitBindingId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SourcePropertyId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SourceVariantId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CurrentUnitTarget {
    Class(hir::ClassId),
    Interface(hir::InterfaceId),
    Struct(hir::StructId),
    Enum(hir::EnumId),
    Object(hir::ObjectId),
    TypeAlias(SourceTypeAliasId),
    Function(hir::FunctionId),
    Property(hir::PropertyId),
    EnumVariant(hir::EnumVariantRef),
    SourceProperty(SourcePropertyId),
    SourceVariant(SourceVariantId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StaticNamespace {
    Class(hir::ClassId),
    Interface(hir::InterfaceId),
    Struct(hir::StructId),
    Enum(hir::EnumId),
    Object(hir::ObjectId),
    Companion(hir::CompanionRelationId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ResolvedNamespace {
    Package(PackageId),
    Static(StaticNamespace),
}

/// A declaration-side identity plus complete diagnostic/access provenance.
#[derive(Debug, Clone)]
pub(crate) struct CurrentUnitBinding {
    pub(crate) target: CurrentUnitTarget,
    pub(crate) source: ast::Stage1SourceHandle,
    pub(crate) file: usize,
    pub(crate) span: ast::Span,
    pub(crate) access: hir::EffectiveLookupDomain,
    pub(crate) name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct CurrentUnitImportWitness {
    pub(crate) source_binding: CurrentUnitBindingId,
    pub(crate) site: hir::VisibilityFile,
    pub(crate) access: hir::EffectiveLookupDomain,
}

/// The binding id dereferences to the typed declaration target. A forwarding
/// static edge retains the same id rather than creating another entity.
#[derive(Debug, Clone)]
pub(crate) struct ImportedBinding {
    pub(crate) binding: CurrentUnitBindingId,
    pub(crate) sources: ast::NonEmptyVec<CurrentUnitImportWitness>,
}

#[derive(Debug, Clone)]
pub(crate) struct ImportSyntaxOrigin {
    pub(crate) source: ast::Stage1SourceHandle,
    pub(crate) span: ast::Span,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedExactImport {
    pub(crate) local_name: String,
    pub(crate) targets: ast::NonEmptyVec<ImportedBinding>,
    pub(crate) origin: ImportSyntaxOrigin,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedStarImport {
    pub(crate) namespace: ResolvedNamespace,
    pub(crate) snapshot: BTreeMap<String, ast::NonEmptyVec<ImportedBinding>>,
    pub(crate) origin: ImportSyntaxOrigin,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FrozenFileImports {
    pub(crate) exact: Vec<ResolvedExactImport>,
    pub(crate) stars: Vec<ResolvedStarImport>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImportLookupLayer {
    Exact,
    CurrentPackage(PackageId),
    Star,
    CorePrelude,
}

#[derive(Debug, Clone)]
pub(crate) struct ImportCandidateLayer {
    pub(crate) kind: ImportLookupLayer,
    pub(crate) bindings: Vec<CurrentUnitBindingId>,
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
    source_property_count: usize,
    global_property_sources: Vec<PropertyImportSource>,
    object_property_sources: HashMap<(hir::ObjectId, usize), SourcePropertyId>,
    resolved_properties: HashMap<SourcePropertyId, hir::PropertyId>,
    source_variants: Vec<SourceVariant>,
    pub(crate) files: Vec<FrozenFileImports>,
}

impl CurrentUnitImports {
    pub(crate) fn global_property_source(&self, declaration_index: usize) -> PropertyImportSource {
        self.global_property_sources[declaration_index]
    }

    pub(crate) fn object_property_source(
        &self,
        object: hir::ObjectId,
        member_index: usize,
        is_core: bool,
    ) -> PropertyImportSource {
        if is_core {
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
            (binding.source, binding.span.start, binding.span.end, id.0)
        });
        bindings.dedup();
        bindings
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
                        .flat_map(|import| import.targets.iter().map(|target| target.binding)),
                ),
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
            },
            ImportCandidateLayer {
                kind: ImportLookupLayer::Star,
                bindings: self.canonicalize(imports.stars.iter().flat_map(|import| {
                    import
                        .snapshot
                        .get(name)
                        .into_iter()
                        .flat_map(ast::NonEmptyVec::iter)
                        .map(|target| target.binding)
                })),
            },
            ImportCandidateLayer {
                kind: ImportLookupLayer::CorePrelude,
                bindings: Vec::new(),
            },
        ]
    }
}
