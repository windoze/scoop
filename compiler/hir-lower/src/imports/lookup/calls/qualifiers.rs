//! Expression receiver and static type qualifier lookup.

use super::{
    ExpressionQualifierLookup, ExpressionQualifierTarget, NamedCallBinding, NamedCallOrigin,
    NamedCallTarget,
};
use crate::Lowerer;
use crate::imports::lookup::LookupLayer;
use crate::imports::{CurrentUnitBindingId, CurrentUnitTarget, ImportLookupLayer};
use crate::namespace::{TopLevelLookupLayer, TopLevelTypeTarget};
use scoop_ast as ast;
use scoop_hir as hir;

use super::super::values::ValueTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpressionQualifierValueOrigin {
    CurrentUnit(CurrentUnitBindingId),
    Core(NamedCallTarget),
    Dependency(hir::ImportedTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExpressionQualifierCandidate {
    Value(ExpressionQualifierValueOrigin),
    ExtensionProperty {
        origin: ExpressionQualifierValueOrigin,
        property: Option<hir::PropertyId>,
    },
    Type(TopLevelTypeTarget),
}

impl Lowerer {
    fn current_expression_qualifier_candidate(
        &self,
        binding: CurrentUnitBindingId,
    ) -> ExpressionQualifierCandidate {
        let binding_value = self.imports.binding(binding);
        match binding_value.target {
            CurrentUnitTarget::Object(_) => ExpressionQualifierCandidate::Value(
                ExpressionQualifierValueOrigin::CurrentUnit(binding),
            ),
            CurrentUnitTarget::Property(property)
                if matches!(
                    self.properties[property].owner,
                    hir::PropertyOwner::Extension(_)
                ) =>
            {
                ExpressionQualifierCandidate::ExtensionProperty {
                    origin: ExpressionQualifierValueOrigin::CurrentUnit(binding),
                    property: Some(property),
                }
            }
            CurrentUnitTarget::SourceProperty(property)
                if self.imports.source_extension_properties.contains(&property) =>
            {
                ExpressionQualifierCandidate::ExtensionProperty {
                    origin: ExpressionQualifierValueOrigin::CurrentUnit(binding),
                    property: self.imports.resolved_properties.get(&property).copied(),
                }
            }
            target => target.type_target().map_or_else(
                || {
                    ExpressionQualifierCandidate::Value(
                        ExpressionQualifierValueOrigin::CurrentUnit(binding),
                    )
                },
                ExpressionQualifierCandidate::Type,
            ),
        }
    }

    fn core_expression_qualifier_candidates(
        &self,
        name: &str,
    ) -> Vec<ExpressionQualifierCandidate> {
        self.core_named_call_targets(name)
            .0
            .into_iter()
            .map(|target| match target {
                NamedCallTarget::Type(target) => ExpressionQualifierCandidate::Type(target),
                NamedCallTarget::ExtensionProperty(property) => {
                    ExpressionQualifierCandidate::ExtensionProperty {
                        origin: ExpressionQualifierValueOrigin::Core(target),
                        property: Some(property),
                    }
                }
                NamedCallTarget::Function(_)
                | NamedCallTarget::ImportedCoreCallable(_)
                | NamedCallTarget::ImportedDependency(_)
                | NamedCallTarget::Value(_) => ExpressionQualifierCandidate::Value(
                    ExpressionQualifierValueOrigin::Core(target),
                ),
            })
            .collect()
    }

    fn expression_qualifier_origin_accessible(
        &self,
        origin: ExpressionQualifierValueOrigin,
    ) -> bool {
        match origin {
            ExpressionQualifierValueOrigin::CurrentUnit(binding) => {
                self.access_domain_allows(&self.imports.binding(binding).access.0, None)
            }
            ExpressionQualifierValueOrigin::Core(target) => {
                self.named_call_binding_accessible(&NamedCallBinding {
                    target,
                    origin: NamedCallOrigin::Core(target),
                })
            }
            ExpressionQualifierValueOrigin::Dependency(_) => true,
        }
    }

    fn expression_qualifier_extension_layer_blocks(
        &self,
        name: &ast::Ident,
        properties: &[(ExpressionQualifierValueOrigin, Option<hir::PropertyId>)],
    ) -> bool {
        if properties.is_empty() {
            return false;
        }
        // Outside an ordinary implicit-this read, an extension property is a
        // visible non-value binding and therefore remains a terminal blocker.
        if self.initialization_context.is_some() || self.current_this_ty().is_none() {
            return true;
        }
        let Some(properties) = properties
            .iter()
            .map(|(_, property)| *property)
            .collect::<Option<Vec<_>>>()
        else {
            // Declaration preflight may run before source properties have HIR
            // ids. It cannot prove NoApplicable, so it must not fall through.
            return true;
        };
        let mut state = self.clone();
        let Some(receiver) = state.lower_current_this(name.span) else {
            return true;
        };
        let mut sink = Vec::new();
        !matches!(
            state.resolve_extension_property_candidates_outcome(
                receiver,
                name,
                &properties,
                &mut sink,
                false,
            ),
            crate::properties::ExtensionPropertyCandidateOutcome::NoCandidate
                | crate::properties::ExtensionPropertyCandidateOutcome::NoApplicable
        )
    }

    /// Classify the first source layer that can supply an expression receiver
    /// or a static type qualifier. A value in that layer owns the receiver
    /// spelling; only a layer without values may expose a type/typealias.
    pub(crate) fn lookup_expression_qualifier(
        &self,
        name: &ast::Ident,
    ) -> ExpressionQualifierLookup {
        let core = || {
            let (_, suppressed_callables) = self.core_named_callable_partition(&name.text);
            LookupLayer {
                kind: ImportLookupLayer::CorePrelude,
                suppressed_callables,
                candidates: self.core_expression_qualifier_candidates(&name.text),
            }
        };
        let layers = match self
            .top_level_namespaces
            .source_namespace(self.current_file)
        {
            TopLevelLookupLayer::CorePrelude => vec![core()],
            TopLevelLookupLayer::CurrentPackage(package) => self
                .expression_import_layers(package, &name.text)
                .into_iter()
                .map(|layer| {
                    if layer.kind == ImportLookupLayer::CorePrelude {
                        core()
                    } else {
                        let mut candidates = layer
                            .bindings
                            .into_iter()
                            .map(|binding| self.current_expression_qualifier_candidate(binding))
                            .collect::<Vec<_>>();
                        candidates.extend(layer.dependency_bindings.into_iter().map(|binding| {
                            ExpressionQualifierCandidate::Value(
                                ExpressionQualifierValueOrigin::Dependency(binding.target()),
                            )
                        }));
                        LookupLayer {
                            kind: layer.kind,
                            suppressed_callables: layer.suppressed_callables,
                            candidates,
                        }
                    }
                })
                .collect(),
        };
        let mut inaccessible = Vec::new();
        for layer in layers {
            let mut values = Vec::new();
            let mut extensions = Vec::new();
            let mut types = Vec::new();
            for candidate in layer.candidates {
                let accessible = match candidate {
                    ExpressionQualifierCandidate::Value(origin)
                    | ExpressionQualifierCandidate::ExtensionProperty { origin, .. } => {
                        self.expression_qualifier_origin_accessible(origin)
                    }
                    ExpressionQualifierCandidate::Type(target) => {
                        self.top_level_type_target_is_accessible(target)
                    }
                };
                if !accessible {
                    if !inaccessible.contains(&candidate) {
                        inaccessible.push(candidate);
                    }
                    continue;
                }
                match candidate {
                    ExpressionQualifierCandidate::Value(origin) => {
                        if !values.contains(&origin) {
                            values.push(origin);
                        }
                    }
                    ExpressionQualifierCandidate::ExtensionProperty { origin, property } => {
                        if !extensions.iter().any(
                            |(candidate, _): &(ExpressionQualifierValueOrigin, _)| {
                                *candidate == origin
                            },
                        ) {
                            extensions.push((origin, property));
                        }
                    }
                    ExpressionQualifierCandidate::Type(target) => {
                        if !types.contains(&target) {
                            types.push(target);
                        }
                    }
                }
            }
            let extension_blocks =
                self.expression_qualifier_extension_layer_blocks(name, &extensions);
            if !layer.suppressed_callables.is_empty() {
                return ExpressionQualifierLookup::Value;
            }
            if !values.is_empty() {
                if extension_blocks {
                    return ExpressionQualifierLookup::Value;
                }
                return match values.as_slice() {
                    [ExpressionQualifierValueOrigin::CurrentUnit(binding)] if types.is_empty() => {
                        match self.imports.binding(*binding).target {
                            CurrentUnitTarget::Object(object) => ExpressionQualifierLookup::Unique(
                                ExpressionQualifierTarget::Object(object),
                            ),
                            _ => ExpressionQualifierLookup::Value,
                        }
                    }
                    [
                        ExpressionQualifierValueOrigin::Core(NamedCallTarget::Value(
                            ValueTarget::Object(object),
                        )),
                    ] if types.is_empty() => ExpressionQualifierLookup::Unique(
                        ExpressionQualifierTarget::Object(*object),
                    ),
                    [] => unreachable!("the value layer is known to be non-empty"),
                    _ => ExpressionQualifierLookup::Value,
                };
            }
            if extension_blocks {
                return ExpressionQualifierLookup::Value;
            }
            return match types.as_slice() {
                [] => continue,
                [target] => {
                    ExpressionQualifierLookup::Unique(ExpressionQualifierTarget::Type(*target))
                }
                [_, ..] => ExpressionQualifierLookup::Ambiguous,
            };
        }
        let mut inaccessible_values = Vec::new();
        let mut inaccessible_types = Vec::new();
        for candidate in inaccessible {
            match candidate {
                ExpressionQualifierCandidate::Value(origin) => {
                    if !inaccessible_values.contains(&origin) {
                        inaccessible_values.push(origin);
                    }
                }
                ExpressionQualifierCandidate::ExtensionProperty { origin, .. } => {
                    if !inaccessible_values.contains(&origin) {
                        inaccessible_values.push(origin);
                    }
                }
                ExpressionQualifierCandidate::Type(target) => {
                    if !inaccessible_types.contains(&target) {
                        inaccessible_types.push(target);
                    }
                }
            }
        }
        match (
            inaccessible_values.as_slice(),
            inaccessible_types.as_slice(),
        ) {
            ([ExpressionQualifierValueOrigin::CurrentUnit(binding)], []) => {
                match self.imports.binding(*binding).target {
                    CurrentUnitTarget::Object(object) => ExpressionQualifierLookup::Inaccessible(
                        ExpressionQualifierTarget::Object(object),
                    ),
                    _ => ExpressionQualifierLookup::Value,
                }
            }
            (
                [
                    ExpressionQualifierValueOrigin::Core(NamedCallTarget::Value(
                        ValueTarget::Object(object),
                    )),
                ],
                [],
            ) => {
                ExpressionQualifierLookup::Inaccessible(ExpressionQualifierTarget::Object(*object))
            }
            ([], [target]) => {
                ExpressionQualifierLookup::Inaccessible(ExpressionQualifierTarget::Type(*target))
            }
            ([], []) => ExpressionQualifierLookup::Missing,
            _ => ExpressionQualifierLookup::Ambiguous,
        }
    }
}
