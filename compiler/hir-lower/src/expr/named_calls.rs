//! Import-aware named calls. Every function-like origin in one final
//! partition is probed before the single cross-kind MSC decision.

use super::members::{
    PropertyExtensionInvokeInput, PropertyExtensionInvokeOrigin, PropertyExtensionInvokeOutcome,
};
use super::*;
use crate::call_resolution::candidates::{NominalConstructorSource, NominalConstructorView};
use crate::call_resolution::named::NamedFunctionLikeProbe;
use crate::imports::ImportLookupLayer;
use crate::imports::lookup::calls::{
    ExtensionPropertyTarget, NamedCallBinding, NamedCallOrigin, NamedCallTarget,
};
use crate::imports::lookup::values::{ResolvedValueTarget, ValueTarget};
use crate::namespace::TopLevelTypeTarget;
use crate::overload::{CallArgumentProtocol, NamedCallReceiver, OverloadCall};

pub(crate) mod imported_dependency;
mod nominals;
mod values;

struct NamedValueLayer {
    read: SuccessfulExprLayer,
    rank: usize,
    origin: PropertyExtensionInvokeOrigin,
}

enum NamedFunctionCommit {
    TopLevel,
    Member,
    ImportedDependency,
    Nominal,
    Intrinsic(SuccessfulExprLayer),
}

struct NamedApplicable {
    probe: NamedFunctionLikeProbe,
    commit: NamedFunctionCommit,
}

impl Lowerer {
    pub(super) fn lower_layered_named_call(
        &mut self,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let name = &call.callee.text;
        let mut first_failure = None;
        let mut prelude_variant_failure = None;
        let local = self.local_function_scopes.lookup(name);
        if !local.is_empty() {
            let mut state = self.clone();
            let mut layer_sink = Vec::new();
            match state.lower_local_function_layer(name, &local, call, &mut layer_sink, expected) {
                Ok(Some(expression)) => {
                    return Some(self.commit_expr_layer(
                        SuccessfulExprLayer {
                            state: Box::new(state),
                            expression,
                            sink: layer_sink,
                        },
                        sink,
                    ));
                }
                Ok(None) => {
                    first_failure = Some(Box::new(state));
                }
                Err(()) => {
                    self.commit_layer_diagnostics(state);
                    return None;
                }
            }
        }
        if self.lexical_nested_nominal_target(name).is_some() {
            match self.probe_expr_layer(|state, sink| {
                let constructor = state.classify_constructor(&call.callee)?;
                state.lower_nominal_constructor_call(constructor, call, sink, expected)
            }) {
                Ok(layer) => return Some(self.commit_expr_layer(layer, sink)),
                Err(failure) => {
                    if failure.diagnostics.len() > self.diagnostics.len() {
                        first_failure.get_or_insert(failure);
                    }
                }
            }
        }
        if let Some(receiver_ty) = self.initializing_receiver_type()
            && !self.methods_by_name(receiver_ty, name).is_empty()
        {
            self.error(
                call.span,
                "initializing receiver cannot escape before construction completes".into(),
            );
            return None;
        }
        let site = CallSite {
            type_args: &call.type_args,
            args: &call.args,
            span: call.span,
        };
        let mut pending_values = Vec::new();
        let mut implicit_property = None;
        if let Some(receiver_ty) = self.current_this_ty() {
            let members = self.methods_by_name(receiver_ty, name);
            let mut state = self.clone();
            let receiver = state.lower_current_this(call.callee.span)?;
            match state.probe_member_call_partition(
                members,
                &call.callee,
                receiver,
                site,
                expected,
                RequiredCallableModifiers::default(),
            ) {
                PropertyExtensionInvokeOutcome::Resolved(layer) => {
                    return Some(self.commit_expr_layer(layer, sink));
                }
                PropertyExtensionInvokeOutcome::NoApplicable(failure) => {
                    if let Some(failure) = failure {
                        first_failure.get_or_insert(failure);
                    }
                }
                PropertyExtensionInvokeOutcome::Blocked => return None,
                PropertyExtensionInvokeOutcome::Failed(failure) => {
                    self.commit_layer_diagnostics(*failure);
                    return None;
                }
            }
        }
        // An initializer has a typed receiver but deliberately no escapable
        // `this` value. Its already-initialized stored properties still form
        // the real-member layer and may themselves be callable values.
        let property_receiver_ty = self
            .current_this_ty()
            .or_else(|| self.initializing_receiver_type());
        if let Some(receiver_ty) = property_receiver_ty
            && let Some((property_id, _, _)) =
                self.find_accessible_nominal_property(receiver_ty, name)
        {
            let property = self.probe_expr_layer(|state, _| {
                if state.initialization_context.is_some() {
                    if state.initializing_receiver_has_field(name) {
                        return state.bare_member_fallback(&call.callee);
                    }
                    return None;
                }
                let receiver = state.lower_current_this(call.callee.span)?;
                state.member_property_read(receiver, &call.callee)
            });
            if let Ok(read) = property {
                let outcome = self.probe_named_value_member(&read, site, expected);
                if let Some(expression) = self
                    .finish_property_partition(outcome, sink, &mut first_failure)
                    .ok()?
                {
                    return Some(expression);
                }
                implicit_property = Some(NamedValueLayer {
                    read,
                    rank: 0,
                    origin: PropertyExtensionInvokeOrigin::Member(property_id),
                });
            }
        }
        let layers = self.named_call_layers(name);
        let mut found = false;
        for layer in layers {
            let rank = layer.kind.call_rank();
            found |= !layer.candidates.is_empty();
            let mut state = self.clone();
            let mut layer_sink = Vec::new();
            match state.lower_named_function_partition(
                &layer.candidates,
                layer.kind,
                call,
                &mut layer_sink,
                expected,
            ) {
                Ok(Some(expression)) => {
                    return Some(self.commit_expr_layer(
                        SuccessfulExprLayer {
                            state: Box::new(state),
                            expression,
                            sink: layer_sink,
                        },
                        sink,
                    ));
                }
                Ok(None) => {
                    if state.diagnostics.len() > self.diagnostics.len() {
                        if layer.kind == ImportLookupLayer::CorePrelude
                            && layer.candidates.iter().all(|binding| {
                                matches!(
                                    binding.target,
                                    NamedCallTarget::Value(ValueTarget::Variant(_))
                                        | NamedCallTarget::ImportedDependency(
                                            hir::ImportedTarget::EnumVariant(_)
                                        )
                                )
                            })
                        {
                            prelude_variant_failure.get_or_insert(Box::new(state));
                        } else {
                            first_failure.get_or_insert(Box::new(state));
                        }
                    }
                }
                Err(()) => {
                    self.commit_layer_diagnostics(state);
                    return None;
                }
            }
            // Real-member property + extension invoke is c-level 4.
            if let Some(property) = &implicit_property {
                let outcome = self.probe_named_extension_values(
                    std::slice::from_ref(property),
                    site,
                    expected,
                    rank,
                    true,
                    layer.kind.call_name(),
                );
                if let Some(expression) = self
                    .finish_property_partition(outcome, sink, &mut first_failure)
                    .ok()?
                {
                    return Some(expression);
                }
            }
            let mut current_values = Vec::new();
            let values = layer
                .candidates
                .iter()
                .filter_map(|binding| match (&binding.target, &binding.origin) {
                    (NamedCallTarget::Value(ValueTarget::Variant(_)), _) => None,
                    (NamedCallTarget::Value(value), _) => Some((
                        ResolvedValueTarget::Materialized(*value),
                        PropertyExtensionInvokeOrigin::NamedValue(*value),
                    )),
                    (
                        NamedCallTarget::ImportedDependency(
                            target @ (hir::ImportedTarget::Property(_)
                            | hir::ImportedTarget::ObjectValue(_)),
                        ),
                        NamedCallOrigin::Dependency(binding),
                    ) => Some((
                        ResolvedValueTarget::Dependency(binding.clone()),
                        PropertyExtensionInvokeOrigin::DependencyValue(*target),
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if values.len() > 1 {
                self.error(
                    call.callee.span,
                    format!(
                        "value `{name}` is ambiguous in {} layer",
                        layer.kind.call_name()
                    ),
                );
                return None;
            }
            if let Some((value, origin)) = values.into_iter().next() {
                match self.probe_expr_layer(|state, _| {
                    state.lower_resolved_value_target(&call.callee, value, None)
                }) {
                    Ok(read) => current_values.push(NamedValueLayer { read, rank, origin }),
                    Err(failure) => {
                        first_failure.get_or_insert(failure);
                    }
                }
            }
            let extension_properties = layer
                .candidates
                .iter()
                .filter_map(|binding| match (binding.target, &binding.origin) {
                    (NamedCallTarget::ExtensionProperty(id), _) => {
                        Some(ExtensionPropertyTarget::Current(id))
                    }
                    (
                        NamedCallTarget::ImportedDependency(
                            hir::ImportedTarget::ExtensionProperty(_),
                        ),
                        NamedCallOrigin::Dependency(binding),
                    ) => Some(ExtensionPropertyTarget::Dependency(binding.clone())),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if !extension_properties.is_empty() && self.current_this_ty().is_some() {
                let mut state = self.clone();
                let mut setup = Vec::new();
                let receiver = state.lower_current_this(call.callee.span)?;
                match state.resolve_extension_property_candidates_outcome(
                    receiver,
                    &call.callee,
                    &extension_properties,
                    &mut setup,
                ) {
                    crate::properties::ExtensionPropertyCandidateOutcome::Resolved(property) => {
                        current_values.push(NamedValueLayer {
                            origin: PropertyExtensionInvokeOrigin::Extension(property.identity()),
                            read: SuccessfulExprLayer {
                                state: Box::new(state),
                                expression: property.read,
                                sink: setup,
                            },
                            rank,
                        })
                    }
                    crate::properties::ExtensionPropertyCandidateOutcome::NoApplicable => {
                        first_failure.get_or_insert(Box::new(state));
                    }
                    crate::properties::ExtensionPropertyCandidateOutcome::Failed => {
                        self.commit_layer_diagnostics(state);
                        return None;
                    }
                    crate::properties::ExtensionPropertyCandidateOutcome::NoCandidate => {}
                }
            }
            if current_values.len() > 1 {
                self.error(
                    call.callee.span,
                    format!(
                        "property `{name}` is ambiguous in {} layer",
                        layer.kind.call_name()
                    ),
                );
                return None;
            }
            // Scope property selection precedes invoke applicability (c5).
            for property in &current_values {
                let outcome = self.probe_named_value_member(&property.read, site, expected);
                if let Some(expression) = self
                    .finish_property_partition(outcome, sink, &mut first_failure)
                    .ok()?
                {
                    return Some(expression);
                }
            }
            pending_values.extend(current_values);
            // All c6 combinations whose effective origin is this scope form
            // one partition. Neither property nor import order picks a winner.
            let outcome = self.probe_named_extension_values(
                &pending_values,
                site,
                expected,
                rank,
                false,
                layer.kind.call_name(),
            );
            if let Some(expression) = self
                .finish_property_partition(outcome, sink, &mut first_failure)
                .ok()?
            {
                return Some(expression);
            }
            if !layer.suppressed_callables.is_empty() {
                if let Some(failure) = first_failure {
                    self.commit_layer_diagnostics(*failure);
                }
                return None;
            }
        }
        self.finish_named_call_fallback(
            call,
            sink,
            expected,
            first_failure,
            prelude_variant_failure,
            found,
        )
    }
}
