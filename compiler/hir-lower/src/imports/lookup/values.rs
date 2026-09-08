//! Non-callable name selection. Selection is independent of expected types
//! and does not materialize getters, singleton receivers or enum applications.

use super::{LookupLayer, LookupResult};
use crate::{
    Lowerer, NominalTarget, Owner,
    imports::{CurrentUnitBindingId, CurrentUnitTarget, ImportLookupLayer},
    namespace::{TopLevelLookupLayer, TopLevelTypeTarget},
};
use scoop_ast as ast;
use scoop_hir as hir;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueTarget {
    Property(hir::PropertyId),
    Object(hir::ObjectId),
    Variant(hir::EnumVariantRef),
}

/// A declaration-side origin is usable even during static-image preflight,
/// before every source property has been allocated. It is never an HIR value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValueOrigin {
    CurrentUnit(CurrentUnitBindingId),
    NonValue {
        binding: CurrentUnitBindingId,
        target: NonValueTarget,
    },
    Core(ValueTarget),
    CoreNonValue(NonValueTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NonValueTarget {
    Function(hir::FunctionId),
    Type(TopLevelTypeTarget),
    ExtensionProperty(hir::PropertyId),
    SourceExtensionProperty(crate::imports::SourcePropertyId),
}

pub(crate) enum NamedPropertyReceiver {
    None,
    Singleton {
        owner: hir::MethodOwnerApplication,
        value: hir::Expr,
    },
}

impl NamedPropertyReceiver {
    pub(crate) fn parts(self) -> (Option<hir::MethodOwnerApplication>, Option<hir::Expr>) {
        match self {
            Self::None => (None, None),
            Self::Singleton { owner, value } => (Some(owner), Some(value)),
        }
    }
}

impl Lowerer {
    fn non_value_origin(origin: ValueOrigin) -> Option<NonValueTarget> {
        match origin {
            ValueOrigin::NonValue { target, .. } | ValueOrigin::CoreNonValue(target) => {
                Some(target)
            }
            _ => None,
        }
    }
    fn value_binding_origin(&self, binding: CurrentUnitBindingId) -> ValueOrigin {
        let target = match self.imports.binding(binding).target {
            CurrentUnitTarget::Function(id) => NonValueTarget::Function(id),
            CurrentUnitTarget::Property(id)
                if matches!(self.properties[id].owner, hir::PropertyOwner::Extension(_)) =>
            {
                NonValueTarget::ExtensionProperty(id)
            }
            CurrentUnitTarget::SourceProperty(id)
                if self.imports.source_extension_properties.contains(&id) =>
            {
                NonValueTarget::SourceExtensionProperty(id)
            }
            target
                if target.type_target().is_some()
                    && !matches!(target, CurrentUnitTarget::Object(_)) =>
            {
                NonValueTarget::Type(
                    target
                        .type_target()
                        .expect("a type binding has a typed target"),
                )
            }
            CurrentUnitTarget::Property(_)
            | CurrentUnitTarget::SourceProperty(_)
            | CurrentUnitTarget::Object(_)
            | CurrentUnitTarget::EnumVariant(_)
            | CurrentUnitTarget::SourceVariant(_) => return ValueOrigin::CurrentUnit(binding),
            _ => unreachable!("all nominal bindings carry a type target"),
        };
        ValueOrigin::NonValue { binding, target }
    }

    pub(crate) fn named_call_value_origin(
        &self,
        binding: super::calls::NamedCallBinding,
    ) -> ValueOrigin {
        match binding.origin {
            super::calls::NamedCallOrigin::CurrentUnit(id) => self.value_binding_origin(id),
            super::calls::NamedCallOrigin::Core(target) => match target {
                super::calls::NamedCallTarget::Function(id) => {
                    ValueOrigin::CoreNonValue(NonValueTarget::Function(id))
                }
                super::calls::NamedCallTarget::Type(target) => {
                    ValueOrigin::CoreNonValue(NonValueTarget::Type(target))
                }
                super::calls::NamedCallTarget::Value(value) => ValueOrigin::Core(value),
                super::calls::NamedCallTarget::ExtensionProperty(id) => {
                    ValueOrigin::CoreNonValue(NonValueTarget::ExtensionProperty(id))
                }
            },
        }
    }

    fn core_value_candidates(&self, name: &str) -> Vec<ValueOrigin> {
        let mut result = self
            .top_level_namespaces
            .property_layers(self.current_file, name)
            .into_iter()
            .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
            .flat_map(|layer| layer.candidates)
            .map(|id| ValueOrigin::Core(ValueTarget::Property(id)))
            .collect::<Vec<_>>();
        result.extend(
            self.top_level_namespaces
                .type_layers(self.current_file, name)
                .into_iter()
                .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
                .flat_map(|layer| layer.candidates)
                .map(|target| match target {
                    TopLevelTypeTarget::Nominal(NominalTarget::Object(id)) => {
                        ValueOrigin::Core(ValueTarget::Object(id))
                    }
                    target => ValueOrigin::CoreNonValue(NonValueTarget::Type(target)),
                }),
        );
        result.extend(
            self.top_level_namespaces
                .named_callable_layers(self.current_file, name)
                .into_iter()
                .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
                .flat_map(|layer| layer.candidates)
                .map(|id| ValueOrigin::CoreNonValue(NonValueTarget::Function(id))),
        );
        result.extend(
            self.top_level_namespaces
                .extension_property_layers(self.current_file, name)
                .into_iter()
                .filter(|layer| layer.kind == TopLevelLookupLayer::CorePrelude)
                .flat_map(|layer| layer.candidates)
                .map(|id| ValueOrigin::CoreNonValue(NonValueTarget::ExtensionProperty(id))),
        );
        result.extend(
            self.core_prelude_variant_refs(name)
                .iter()
                .copied()
                .map(|target| ValueOrigin::Core(ValueTarget::Variant(target))),
        );
        result
    }

    fn value_origin_accessible(&self, origin: ValueOrigin) -> bool {
        if let ValueOrigin::CoreNonValue(target) = origin {
            return match target {
                NonValueTarget::Function(id) => self.function_is_accessible(id, None),
                NonValueTarget::Type(target) => self.top_level_type_target_is_accessible(target),
                NonValueTarget::ExtensionProperty(id) => {
                    self.access_domain_allows(&self.properties[id].access.lookup.0, None)
                }
                NonValueTarget::SourceExtensionProperty(_) => {
                    unreachable!("core has no provisional import properties")
                }
            };
        }
        let domain = match origin {
            ValueOrigin::CurrentUnit(id) | ValueOrigin::NonValue { binding: id, .. } => {
                &self.imports.binding(id).access.0
            }
            ValueOrigin::Core(ValueTarget::Property(id)) => &self.properties[id].access.lookup.0,
            ValueOrigin::Core(ValueTarget::Object(id)) => &self.objects[id].access.lookup.0,
            ValueOrigin::Core(ValueTarget::Variant(target)) => {
                &self.enums[target.enumeration()].access.lookup.0
            }
            ValueOrigin::CoreNonValue(_) => unreachable!("core blocker access was checked above"),
        };
        self.access_domain_allows(domain, None)
    }

    pub(crate) fn lookup_value_origin(&self, name: &str) -> LookupResult<ValueOrigin> {
        let core = || LookupLayer {
            kind: ImportLookupLayer::CorePrelude,
            candidates: self.core_value_candidates(name),
        };
        let layers = match self
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
                            candidates: layer
                                .bindings
                                .into_iter()
                                .map(|binding| self.value_binding_origin(binding))
                                .collect(),
                        }
                    }
                })
                .collect(),
        };
        let mut inaccessible = Vec::new();
        for layer in layers {
            let mut visible = Vec::new();
            for origin in layer.candidates {
                let candidates = if self.value_origin_accessible(origin) {
                    &mut visible
                } else {
                    &mut inaccessible
                };
                if !candidates.contains(&origin) {
                    candidates.push(origin);
                }
            }
            if visible
                .iter()
                .any(|origin| Self::non_value_origin(*origin).is_none())
            {
                visible.retain(|origin| Self::non_value_origin(*origin).is_none());
            }
            match visible.as_slice() {
                [] => {}
                [one] => return LookupResult::Unique(*one),
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

    /// Early static preflight may return None for an unallocated declaration;
    /// it must then decline folding, not select a different origin.
    pub(crate) fn materialized_value_target(&self, origin: ValueOrigin) -> Option<ValueTarget> {
        Some(match origin {
            ValueOrigin::NonValue { .. } | ValueOrigin::CoreNonValue(_) => return None,
            ValueOrigin::Core(target) => target,
            ValueOrigin::CurrentUnit(id) => match self.imports.binding(id).target {
                CurrentUnitTarget::Property(id) => ValueTarget::Property(id),
                CurrentUnitTarget::Object(id) => ValueTarget::Object(id),
                CurrentUnitTarget::EnumVariant(target) => ValueTarget::Variant(target),
                CurrentUnitTarget::SourceProperty(id) => {
                    ValueTarget::Property(*self.imports.resolved_properties.get(&id)?)
                }
                CurrentUnitTarget::SourceVariant(id) => {
                    let source = &self.imports.source_variants[id.0];
                    ValueTarget::Variant(hir::EnumVariantRef::checked(
                        &self.enums,
                        source.enumeration,
                        source.index,
                    )?)
                }
                _ => unreachable!("value origin was selected from the value surface"),
            },
        })
    }

    fn value_origin_location(&self, origin: ValueOrigin) -> (usize, ast::Span) {
        match origin {
            ValueOrigin::CoreNonValue(NonValueTarget::Function(id)) => {
                (self.function_files[&id], self.functions[id].span)
            }
            ValueOrigin::CoreNonValue(NonValueTarget::Type(target)) => self
                .type_candidate_location(super::TypeLookupCandidate {
                    target,
                    origin: super::TypeLookupOrigin::ExistingM22Core,
                }),
            ValueOrigin::CoreNonValue(NonValueTarget::ExtensionProperty(id)) => {
                (self.property_files[&id], self.properties[id].span)
            }
            ValueOrigin::CoreNonValue(NonValueTarget::SourceExtensionProperty(_)) => {
                unreachable!("core has no provisional import properties")
            }
            ValueOrigin::CurrentUnit(id) | ValueOrigin::NonValue { binding: id, .. } => {
                let binding = self.imports.binding(id);
                (binding.file, binding.span)
            }
            ValueOrigin::Core(ValueTarget::Property(id)) => {
                (self.property_files[&id], self.properties[id].span)
            }
            ValueOrigin::Core(ValueTarget::Object(id)) => {
                (self.object_files[&id], self.objects[id].span)
            }
            ValueOrigin::Core(ValueTarget::Variant(target)) => (
                self.enum_files[&target.enumeration()],
                self.enums[target.enumeration()].span,
            ),
        }
    }

    pub(crate) fn resolve_value_name(
        &mut self,
        name: &ast::Ident,
    ) -> Result<Option<ValueTarget>, ()> {
        self.resolve_value_origin(name).map(|origin| {
            origin.map(|origin| {
                self.materialized_value_target(origin)
                    .expect("body lookup follows complete declaration materialization")
            })
        })
    }

    pub(crate) fn resolve_value_origin(
        &mut self,
        name: &ast::Ident,
    ) -> Result<Option<ValueOrigin>, ()> {
        let (message, candidates) = match self.lookup_value_origin(&name.text) {
            LookupResult::Missing => return Ok(None),
            LookupResult::Unique(origin @ ValueOrigin::NonValue { target, .. }) => (
                Self::non_value_message(name, target),
                ast::NonEmptyVec::new(origin, Vec::new()),
            ),
            LookupResult::Unique(origin @ ValueOrigin::CoreNonValue(target)) => (
                Self::non_value_message(name, target),
                ast::NonEmptyVec::new(origin, Vec::new()),
            ),
            LookupResult::Unique(origin) => return Ok(Some(origin)),
            LookupResult::Ambiguous { layer, candidates } => {
                let layer = match layer {
                    ImportLookupLayer::Exact => "exact import",
                    ImportLookupLayer::CurrentPackage(_) => "current package",
                    ImportLookupLayer::Star => "star import",
                    ImportLookupLayer::CorePrelude => "core prelude",
                };
                let message = if candidates.iter().all(|origin| {
                    matches!(
                        Self::non_value_origin(*origin),
                        Some(NonValueTarget::Function(_))
                    )
                }) {
                    format!(
                        "function `{}` is not a value; use `::{}` to create a callable reference",
                        name.text, name.text
                    )
                } else {
                    format!("value `{}` is ambiguous in the {layer} layer", name.text)
                };
                (message, candidates)
            }
            LookupResult::Inaccessible(candidates) => (
                format!("value `{}` is not accessible here", name.text),
                candidates,
            ),
        };
        self.push_value_origin_diagnostic(name, message, candidates.as_slice());
        Err(())
    }

    pub(crate) fn diagnose_value_layer(
        &mut self,
        name: &ast::Ident,
        layer: ImportLookupLayer,
        candidates: &[ValueOrigin],
    ) {
        assert!(
            !candidates.is_empty(),
            "a failed value layer has candidates"
        );
        let message = if candidates.len() == 1 {
            let target = Self::non_value_origin(candidates[0])
                .expect("one selected value candidate is not an ambiguity");
            Self::non_value_message(name, target)
        } else if candidates.iter().all(|origin| {
            matches!(
                Self::non_value_origin(*origin),
                Some(NonValueTarget::Function(_))
            )
        }) {
            format!(
                "function `{}` is not a value; use `::{}` to create a callable reference",
                name.text, name.text
            )
        } else {
            let layer = match layer {
                ImportLookupLayer::Exact => "exact import",
                ImportLookupLayer::CurrentPackage(_) => "current package",
                ImportLookupLayer::Star => "star import",
                ImportLookupLayer::CorePrelude => "core prelude",
            };
            format!("value `{}` is ambiguous in the {layer} layer", name.text)
        };
        self.push_value_origin_diagnostic(name, message, candidates);
    }

    fn push_value_origin_diagnostic(
        &mut self,
        name: &ast::Ident,
        message: String,
        candidates: &[ValueOrigin],
    ) {
        let mut diagnostic = ast::Diagnostic::at_file(self.current_file, name.span, message);
        let mut locations = candidates
            .iter()
            .map(|origin| self.value_origin_location(*origin))
            .collect::<Vec<_>>();
        locations.sort_by_key(|(file, span)| (*file, span.start, span.end));
        for (file, span) in locations {
            diagnostic.notes.push(ast::DiagnosticNote {
                file,
                span,
                message: "candidate declared here".to_string(),
            });
        }
        self.diagnostics.push(diagnostic);
    }

    fn non_value_message(name: &ast::Ident, target: NonValueTarget) -> String {
        match target {
            NonValueTarget::Function(_) => format!(
                "function `{}` is not a value; use `::{}` to create a callable reference",
                name.text, name.text
            ),
            NonValueTarget::Type(TopLevelTypeTarget::Alias(_)) => {
                format!("typealias `{}` is a type, not a value", name.text)
            }
            NonValueTarget::Type(TopLevelTypeTarget::Nominal(_)) => {
                format!("type `{}` is a type, not a value", name.text)
            }
            NonValueTarget::ExtensionProperty(_) | NonValueTarget::SourceExtensionProperty(_) => {
                format!("extension property `{}` requires a receiver", name.text)
            }
        }
    }

    pub(crate) fn named_property_receiver(
        &mut self,
        property: hir::PropertyId,
        span: ast::Span,
    ) -> Option<NamedPropertyReceiver> {
        if matches!(
            self.properties[property].representation,
            hir::PropertyRepresentation::Const { .. }
        ) {
            return Some(NamedPropertyReceiver::None);
        }
        Some(match self.properties[property].owner {
            hir::PropertyOwner::TopLevel => NamedPropertyReceiver::None,
            hir::PropertyOwner::Object(object) => NamedPropertyReceiver::Singleton {
                value: self.lower_singleton_value(object, span)?,
                owner: self.method_owner_application(Owner::Object(object), Vec::new()),
            },
            _ => unreachable!("bare property origin is top-level or singleton-owned"),
        })
    }
}
