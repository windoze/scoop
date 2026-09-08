//! Typed, visibility-filtered lookup layers shared by semantic consumers.

use super::{CurrentUnitBindingId, CurrentUnitTarget, ImportLookupLayer, ResolvedNamespace};
use crate::{
    Lowerer, NominalTarget,
    namespace::{PackageId, TopLevelLookupLayer, TopLevelTypeTarget},
};
use scoop_ast as ast;

#[derive(Debug, Clone)]
pub(crate) struct LookupLayer<T> {
    pub(crate) kind: ImportLookupLayer,
    pub(crate) candidates: Vec<T>,
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
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct TypeLookupCandidate {
    pub(crate) target: TopLevelTypeTarget,
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
    fn imported_type_candidates(
        &self,
        bindings: impl IntoIterator<Item = CurrentUnitBindingId>,
    ) -> Vec<TypeLookupCandidate> {
        let mut candidates = Vec::new();
        for binding in bindings {
            if let Some(target) = self.imports.binding(binding).target.type_target()
                && !candidates
                    .iter()
                    .any(|candidate: &TypeLookupCandidate| candidate.target == target)
            {
                candidates.push(TypeLookupCandidate {
                    target,
                    origin: TypeLookupOrigin::CurrentUnit(binding),
                });
            }
        }
        candidates
    }

    pub(crate) fn type_lookup_layers(&self, name: &str) -> Vec<LookupLayer<TypeLookupCandidate>> {
        let core = || LookupLayer {
            kind: ImportLookupLayer::CorePrelude,
            candidates: self
                .top_level_namespaces
                .type_layers(self.current_file, name)
                .into_iter()
                .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
                .flat_map(|layer| layer.candidates)
                .map(|target| TypeLookupCandidate {
                    target,
                    origin: TypeLookupOrigin::ExistingM22Core,
                })
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
                        LookupLayer {
                            kind: layer.kind,
                            candidates: self.imported_type_candidates(layer.bindings),
                        }
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
                if self.top_level_type_target_is_accessible(candidate.target) {
                    if !accessible
                        .iter()
                        .any(|other: &TypeLookupCandidate| other.target == candidate.target)
                    {
                        accessible.push(candidate);
                    }
                } else if !inaccessible
                    .iter()
                    .any(|other: &TypeLookupCandidate| other.target == candidate.target)
                {
                    inaccessible.push(candidate);
                }
            }
            match accessible.as_slice() {
                [] => {}
                [candidate] => return LookupResult::Unique(*candidate),
                [first, rest @ ..] => {
                    return LookupResult::Ambiguous {
                        layer: layer.kind,
                        candidates: ast::NonEmptyVec::new(*first, rest.to_vec()),
                    };
                }
            }
        }
        match inaccessible.as_slice() {
            [] => LookupResult::Missing,
            [first, rest @ ..] => {
                LookupResult::Inaccessible(ast::NonEmptyVec::new(*first, rest.to_vec()))
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
        if self.current_source_is_core() {
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
            candidates: self.imported_type_candidates(self.imports.canonicalize(bindings)),
        }])
    }

    fn type_candidate_location(&self, candidate: TypeLookupCandidate) -> (usize, ast::Span) {
        match candidate.origin {
            TypeLookupOrigin::CurrentUnit(binding) => {
                let binding = self.imports.binding(binding);
                (binding.file, binding.span)
            }
            TypeLookupOrigin::ExistingM22Core => match candidate.target {
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
        }
    }

    /// Resolve a real source use. Pure classifiers may inspect `lookup_type`,
    /// but cannot choose a declaration from an ambiguous layer.
    pub(crate) fn resolve_type_lookup(
        &mut self,
        name: &ast::Ident,
    ) -> Result<Option<TopLevelTypeTarget>, ()> {
        let result = self.lookup_type(&name.text);
        self.commit_type_lookup(name, result)
    }

    pub(crate) fn commit_type_lookup(
        &mut self,
        name: &ast::Ident,
        result: LookupResult<TypeLookupCandidate>,
    ) -> Result<Option<TopLevelTypeTarget>, ()> {
        let (message, candidates) = match result {
            LookupResult::Unique(candidate) => return Ok(Some(candidate.target)),
            LookupResult::Missing => return Ok(None),
            LookupResult::Inaccessible(candidates) if candidates.len() == 1 => {
                // Existing alias/nominal consumers attach the established
                // access diagnostic to this one structurally identified target.
                return Ok(Some(candidates.first().target));
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
            .map(|candidate| self.type_candidate_location(*candidate))
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
