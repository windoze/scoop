//! Typed, visibility-filtered lookup layers shared by semantic consumers.

use super::{
    CurrentUnitBindingId, CurrentUnitTarget, ImportCandidateLayer, ImportLookupLayer,
    ResolvedNamespace,
};
use crate::{
    Lowerer, NominalTarget,
    namespace::{PackageId, TopLevelLookupLayer, TopLevelTypeTarget},
};
use scoop_ast as ast;
use scoop_hir as hir;

pub(crate) mod calls;
pub(crate) mod values;

#[derive(Debug, Clone)]
pub(crate) struct LookupLayer<T> {
    pub(crate) kind: ImportLookupLayer,
    pub(crate) candidates: Vec<T>,
    /// Invalid duplicate declarations that own this layer's spelling without
    /// participating in semantic candidate selection.
    pub(crate) suppressed_callables: Vec<scoop_hir::FunctionId>,
}

#[derive(Debug, Clone)]
pub(crate) enum LookupResult<T> {
    Missing,
    Unique(T),
    Ambiguous {
        layer: ImportLookupLayer,
        candidates: ast::NonEmptyVec<T>,
    },
    Inaccessible(ast::NonEmptyVec<T>),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum TypeLookupOrigin {
    CurrentUnit(CurrentUnitBindingId),
    ExistingM22Core,
    Dependency,
}

#[derive(Debug, Clone)]
pub(crate) enum TypeLookupTarget {
    Current(TopLevelTypeTarget),
    Dependency(scoop_hir::DirectImportedTargetBinding),
}

impl TypeLookupTarget {
    fn same_declaration(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Current(left), Self::Current(right)) => left == right,
            (Self::Dependency(left), Self::Dependency(right)) => left.target() == right.target(),
            (Self::Current(_), Self::Dependency(_)) | (Self::Dependency(_), Self::Current(_)) => {
                false
            }
        }
    }

    pub(crate) const fn current(&self) -> Option<TopLevelTypeTarget> {
        match self {
            Self::Current(target) => Some(*target),
            Self::Dependency(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct TypeLookupCandidate {
    pub(crate) target: TypeLookupTarget,
    pub(crate) origin: TypeLookupOrigin,
}

impl CurrentUnitTarget {
    fn type_target(self) -> Option<TopLevelTypeTarget> {
        Some(match self {
            Self::Class(id) => TopLevelTypeTarget::Nominal(NominalTarget::Class(id)),
            Self::Interface(id) => TopLevelTypeTarget::Nominal(NominalTarget::Interface(id)),
            Self::Struct(id) => TopLevelTypeTarget::Nominal(NominalTarget::Struct(id)),
            Self::Enum(id) => TopLevelTypeTarget::Nominal(NominalTarget::Enum(id)),
            Self::Object(id) => TopLevelTypeTarget::Nominal(NominalTarget::Object(id)),
            Self::TypeAlias(id) => TopLevelTypeTarget::Alias(id),
            Self::Function(_)
            | Self::Property(_)
            | Self::EnumVariant(_)
            | Self::SourceProperty(_)
            | Self::SourceVariant(_) => return None,
        })
    }
}

impl Lowerer {
    /// Body lookup may continue after an unrelated declaration error, but an
    /// unmaterialized value in a higher-priority layer must not expose a
    /// lower-priority declaration. The failed origins remain lowerer-local
    /// diagnostic state and are never returned as semantic candidates.
    fn expression_import_layers(
        &self,
        package: PackageId,
        name: &str,
    ) -> Vec<ImportCandidateLayer> {
        let mut layers = self.imports.layers(self.current_file, package, name);
        let mut terminal = None;
        for (index, layer) in layers.iter_mut().enumerate() {
            let mut suppressed_callables = std::mem::take(&mut layer.suppressed_callables);
            layer.bindings.retain(|binding| {
                let declaration = self.imports.binding(*binding);
                let CurrentUnitTarget::Function(function) = declaration.target else {
                    return true;
                };
                if !self.declaration_surface.rejects_function(function) {
                    return true;
                }
                if self.access_domain_allows(&declaration.access.0) {
                    suppressed_callables.push(function);
                }
                false
            });
            suppressed_callables.sort_by_key(|function| function.into_raw().into_u32());
            suppressed_callables.dedup();
            layer.suppressed_callables = suppressed_callables;
            let unmaterialized_value = layer
                .suppressed_values
                .iter()
                .any(|binding| self.access_domain_allows(&self.imports.binding(*binding).access.0));
            // Callable roles are consumer-specific: an ordinary function in
            // this layer must not hide a lower extension/property/reference
            // role. Keep every typed suppression attached to its raw layer;
            // each callable consumer filters by role before deciding whether
            // that layer is terminal. Unmaterialized values retain their
            // existing cross-kind terminal semantics.
            if unmaterialized_value {
                terminal.get_or_insert(index);
            }
        }
        if let Some(index) = terminal {
            layers.truncate(index + 1);
        }
        layers
    }

    fn imported_type_candidates(
        &self,
        bindings: impl IntoIterator<Item = CurrentUnitBindingId>,
    ) -> Vec<TypeLookupCandidate> {
        let mut candidates = Vec::new();
        for binding in bindings {
            if let Some(target) = self.imports.binding(binding).target.type_target()
                && !candidates.iter().any(|candidate: &TypeLookupCandidate| {
                    candidate.target.current() == Some(target)
                })
            {
                candidates.push(TypeLookupCandidate {
                    target: TypeLookupTarget::Current(target),
                    origin: TypeLookupOrigin::CurrentUnit(binding),
                });
            }
        }
        candidates
    }

    fn dependency_type_candidates(
        &self,
        bindings: impl IntoIterator<Item = hir::DirectImportedTargetBinding>,
    ) -> Vec<TypeLookupCandidate> {
        bindings
            .into_iter()
            .filter(|binding| {
                matches!(
                    binding.target(),
                    hir::ImportedTarget::Type(_)
                        | hir::ImportedTarget::GenericType(_)
                        | hir::ImportedTarget::TypeAlias(_)
                )
            })
            .map(|binding| TypeLookupCandidate {
                target: TypeLookupTarget::Dependency(binding),
                origin: TypeLookupOrigin::Dependency,
            })
            .collect()
    }

    fn type_candidates(&self, layer: ImportCandidateLayer) -> LookupLayer<TypeLookupCandidate> {
        let mut candidates = self.imported_type_candidates(layer.bindings);
        candidates.extend(self.dependency_type_candidates(layer.dependency_bindings));
        LookupLayer {
            kind: layer.kind,
            candidates,
            suppressed_callables: Vec::new(),
        }
    }

    pub(crate) fn type_lookup_layers(&self, name: &str) -> Vec<LookupLayer<TypeLookupCandidate>> {
        let core = || LookupLayer {
            kind: ImportLookupLayer::CorePrelude,
            suppressed_callables: Vec::new(),
            candidates: self
                .top_level_namespaces
                .type_layers(self.current_file, name)
                .into_iter()
                .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
                .flat_map(|layer| layer.candidates)
                .map(|target| TypeLookupCandidate {
                    target: TypeLookupTarget::Current(target),
                    origin: TypeLookupOrigin::ExistingM22Core,
                })
                .chain(
                    self.dependency_type_candidates(
                        self.imports
                            .prelude_bindings(scoop_identity::BindingNamespace::Type, name)
                            .to_vec(),
                    ),
                )
                .collect(),
        };
        match self
            .top_level_namespaces
            .source_namespace(self.current_file)
        {
            TopLevelLookupLayer::CorePrelude => vec![core()],
            TopLevelLookupLayer::CurrentPackage(package) => self
                .imports
                .layers(self.current_file, package, name)
                .into_iter()
                .map(|layer| {
                    if layer.kind == ImportLookupLayer::CorePrelude {
                        core()
                    } else {
                        self.type_candidates(layer)
                    }
                })
                .collect(),
        }
    }

    fn select_type_layer(
        &self,
        layers: Vec<LookupLayer<TypeLookupCandidate>>,
    ) -> LookupResult<TypeLookupCandidate> {
        let mut inaccessible = Vec::new();
        for layer in layers {
            let mut accessible = Vec::new();
            for candidate in layer.candidates {
                let is_accessible = match &candidate.target {
                    TypeLookupTarget::Current(target) => {
                        self.top_level_type_target_is_accessible(*target)
                    }
                    TypeLookupTarget::Dependency(_) => true,
                };
                if is_accessible {
                    if !accessible.iter().any(|other: &TypeLookupCandidate| {
                        other.target.same_declaration(&candidate.target)
                    }) {
                        accessible.push(candidate);
                    }
                } else if !inaccessible.iter().any(|other: &TypeLookupCandidate| {
                    other.target.same_declaration(&candidate.target)
                }) {
                    inaccessible.push(candidate);
                }
            }
            match accessible.as_slice() {
                [] => {}
                [candidate] => return LookupResult::Unique(candidate.clone()),
                [first, rest @ ..] => {
                    return LookupResult::Ambiguous {
                        layer: layer.kind,
                        candidates: ast::NonEmptyVec::new(first.clone(), rest.to_vec()),
                    };
                }
            }
        }
        match inaccessible.as_slice() {
            [] => LookupResult::Missing,
            [first, rest @ ..] => {
                LookupResult::Inaccessible(ast::NonEmptyVec::new(first.clone(), rest.to_vec()))
            }
        }
    }

    pub(crate) fn lookup_type(&self, name: &str) -> LookupResult<TypeLookupCandidate> {
        self.select_type_layer(self.type_lookup_layers(name))
    }

    pub(crate) fn lookup_package_type(
        &self,
        package: PackageId,
        name: &str,
    ) -> LookupResult<TypeLookupCandidate> {
        if !self.source_is_current_cone(self.current_file) {
            return LookupResult::Missing;
        }
        let bindings = self
            .imports
            .namespaces
            .get(&ResolvedNamespace::Package(package))
            .and_then(|members| members.get(name))
            .into_iter()
            .flatten()
            .copied();
        self.select_type_layer(vec![LookupLayer {
            kind: ImportLookupLayer::CurrentPackage(package),
            suppressed_callables: Vec::new(),
            candidates: self.imported_type_candidates(self.imports.canonicalize(bindings)),
        }])
    }

    fn type_candidate_location(&self, candidate: &TypeLookupCandidate) -> (usize, ast::Span) {
        match candidate.origin {
            TypeLookupOrigin::CurrentUnit(binding) => {
                let binding = self.imports.binding(binding);
                (binding.file, binding.span)
            }
            TypeLookupOrigin::ExistingM22Core => match &candidate.target {
                TypeLookupTarget::Dependency(_) => {
                    unreachable!("the M22 core lookup layer contains only current HIR targets")
                }
                TypeLookupTarget::Current(target) => match *target {
                    TopLevelTypeTarget::Alias(id) => {
                        let origin = self.source_type_aliases[id].origin;
                        (origin.file as usize, origin.span)
                    }
                    TopLevelTypeTarget::Nominal(NominalTarget::Struct(id)) => {
                        (self.struct_files[&id], self.structs[id].span)
                    }
                    TopLevelTypeTarget::Nominal(NominalTarget::Enum(id)) => {
                        (self.enum_files[&id], self.enums[id].span)
                    }
                    TopLevelTypeTarget::Nominal(NominalTarget::Class(id)) => {
                        (self.class_files[&id], self.classes[id].span)
                    }
                    TopLevelTypeTarget::Nominal(NominalTarget::Interface(id)) => {
                        (self.interface_files[&id], self.interfaces[id].span)
                    }
                    TopLevelTypeTarget::Nominal(NominalTarget::Object(id)) => {
                        (self.object_files[&id], self.objects[id].span)
                    }
                },
            },
            TypeLookupOrigin::Dependency => (self.current_file, ast::Span::new(0, 0)),
        }
    }

    /// Resolve a real source use. Pure classifiers may inspect `lookup_type`,
    /// but cannot choose a declaration from an ambiguous layer.
    pub(crate) fn resolve_type_lookup(
        &mut self,
        name: &ast::Ident,
    ) -> Result<Option<TypeLookupTarget>, ()> {
        let result = self.lookup_type(&name.text);
        self.commit_type_lookup(name, result)
    }

    pub(crate) fn commit_type_lookup(
        &mut self,
        name: &ast::Ident,
        result: LookupResult<TypeLookupCandidate>,
    ) -> Result<Option<TypeLookupTarget>, ()> {
        let (message, candidates) = match result {
            LookupResult::Unique(candidate) => return Ok(Some(candidate.target)),
            LookupResult::Missing => return Ok(None),
            LookupResult::Inaccessible(candidates) if candidates.len() == 1 => {
                // Existing alias/nominal consumers attach the established
                // access diagnostic to this one structurally identified target.
                return Ok(Some(candidates.first().target.clone()));
            }
            LookupResult::Inaccessible(candidates) => (
                format!(
                    "type `{}` is not accessible from this source location",
                    name.text
                ),
                candidates,
            ),
            LookupResult::Ambiguous { layer, candidates } => {
                let layer = match layer {
                    ImportLookupLayer::Exact => "exact import",
                    ImportLookupLayer::CurrentPackage(_) => "current package",
                    ImportLookupLayer::Star => "star import",
                    ImportLookupLayer::CorePrelude => "core prelude",
                };
                (
                    format!("type `{}` is ambiguous in the {layer} layer", name.text),
                    candidates,
                )
            }
        };
        let mut diagnostic = ast::Diagnostic::at_file(self.current_file, name.span, message);
        let mut locations = candidates
            .iter()
            .map(|candidate| self.type_candidate_location(candidate))
            .collect::<Vec<_>>();
        // This order is diagnostic-only and never chooses a semantic winner.
        locations.sort_by_key(|(file, span)| (*file, span.start, span.end));
        for (file, span) in locations {
            diagnostic.notes.push(ast::DiagnosticNote::at(
                file,
                span,
                "candidate declared here",
            ));
        }
        self.diagnostics.push(diagnostic);
        Err(())
    }
}
