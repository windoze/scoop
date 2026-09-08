//! Native `FunPtr` reference candidate filtering, probing, and diagnostics.

use super::*;

use crate::call_resolution::candidates::CallableView;
use crate::call_resolution::constraints::{CallableCategory, ConstraintFailure};
use crate::call_resolution::diagnostics::{
    callable_layer_name, callable_source_signature, render_callable_constraint_failure,
};

struct ApplicableNativeReference {
    state: Box<Lowerer>,
    function: hir::FunctionId,
    view: CallableView,
}

struct NativeReferenceFailure {
    state: Box<Lowerer>,
    view: CallableView,
    kind: NativeReferenceFailureKind,
}

enum NativeReferenceFailureKind {
    Member,
    Extension,
    Generic,
    Suspend,
    RequiresNoGc,
    NotUserFunction,
    Constraint(ConstraintFailure),
}

enum NativeReferenceLayerOutcome {
    Resolved(hir::Expr),
    NoApplicable,
    Failed,
}

impl Lowerer {
    pub(super) fn lower_native_function_reference(
        &mut self,
        name: &ast::Ident,
        span: Span,
        expected: TypeId,
    ) -> Option<hir::Expr> {
        if self.scopes.lookup(&name.text).is_some()
            || !self.local_function_scopes.lookup(&name.text).is_empty()
        {
            self.error(
                span,
                "a native `FunPtr` address cannot target a local function or function value"
                    .to_string(),
            );
            return None;
        }
        let candidate_layers = self.named_reference_candidate_layers(&name.text);
        if candidate_layers.is_empty() {
            return match self.lower_native_function_reference_layer(name, span, expected, &[]) {
                NativeReferenceLayerOutcome::Resolved(expression) => Some(expression),
                NativeReferenceLayerOutcome::NoApplicable | NativeReferenceLayerOutcome::Failed => {
                    None
                }
            };
        }
        let mut first_failure = None;
        for layer in candidate_layers {
            let mut state = self.clone();
            match state.lower_native_function_reference_layer(
                name,
                span,
                expected,
                &layer.candidates,
            ) {
                NativeReferenceLayerOutcome::Resolved(expression) => {
                    *self = state;
                    return Some(expression);
                }
                NativeReferenceLayerOutcome::NoApplicable => {
                    first_failure.get_or_insert(Box::new(state));
                }
                NativeReferenceLayerOutcome::Failed => {
                    self.commit_layer_diagnostics(state);
                    return None;
                }
            }
        }
        self.commit_layer_diagnostics(*first_failure.expect("at least one native layer failed"));
        None
    }

    fn lower_native_function_reference_layer(
        &mut self,
        name: &ast::Ident,
        span: Span,
        expected: TypeId,
        candidates: &[hir::FunctionId],
    ) -> NativeReferenceLayerOutcome {
        let mut matching = Vec::new();
        let mut failures = Vec::new();
        for &function in candidates {
            let mut state = self.clone();
            let extension = state.extension_receivers.contains_key(&function);
            let candidate = crate::CallableCandidate::function(
                function,
                Vec::new(),
                state.function_lookup_witness(function),
            );
            let view = state.callable_view(&candidate, extension);
            let ineligible = if state.functions[function].method.is_some() {
                Some(NativeReferenceFailureKind::Member)
            } else if extension {
                Some(NativeReferenceFailureKind::Extension)
            } else if !view.callable_parameters.is_empty() {
                Some(NativeReferenceFailureKind::Generic)
            } else if view.effects.is_suspend {
                Some(NativeReferenceFailureKind::Suspend)
            } else if view.effects.attributes.gc_effect != hir::GcEffect::NoGc {
                Some(NativeReferenceFailureKind::RequiresNoGc)
            } else if !matches!(state.functions[function].kind, hir::FunctionKind::User(_)) {
                Some(NativeReferenceFailureKind::NotUserFunction)
            } else {
                None
            };
            if let Some(kind) = ineligible {
                failures.push(NativeReferenceFailure {
                    state: Box::new(state),
                    view,
                    kind,
                });
                continue;
            }
            let parameters = view
                .value_parameters
                .iter()
                .map(|parameter| parameter.ty)
                .collect::<Vec<_>>();
            match state.check_concrete_callable_signature(
                CallableCategory::Native,
                view.effects.is_suspend,
                &parameters,
                view.return_type,
                expected,
            ) {
                Ok(()) => matching.push(ApplicableNativeReference {
                    state: Box::new(state),
                    function,
                    view,
                }),
                Err(failure) => failures.push(NativeReferenceFailure {
                    state: Box::new(state),
                    view,
                    kind: NativeReferenceFailureKind::Constraint(failure),
                }),
            }
        }
        let selected = match matching.len() {
            1 => matching
                .pop()
                .expect("one native reference candidate exists"),
            0 => {
                self.native_reference_failures_diagnostic(name, &failures, span);
                return NativeReferenceLayerOutcome::NoApplicable;
            }
            _ => {
                self.native_reference_ambiguity_diagnostic(name, &matching, span);
                return NativeReferenceLayerOutcome::Failed;
            }
        };
        let function = selected.function;
        *self = *selected.state;
        if self.functions[function].attributes.safety == hir::Safety::Unsafe {
            self.require_unsafe_operation(span, "taking the address of an unsafe callback");
        }
        NativeReferenceLayerOutcome::Resolved(hir::Expr {
            kind: ExprKind::FunctionAddress(function),
            ty: expected,
            span,
            origin: self.expression_origin(span),
        })
    }

    fn native_reference_failures_diagnostic(
        &mut self,
        name: &ast::Ident,
        failures: &[NativeReferenceFailure],
        span: Span,
    ) {
        if failures.is_empty() {
            self.error(
                span,
                format!(
                    "no eligible `@NoGC` top-level function `::{}` exactly matches the expected FunPtr signature",
                    name.text
                ),
            );
            return;
        }
        let views = failures
            .iter()
            .map(|failure| failure.view.clone())
            .collect::<Vec<_>>();
        let layer = callable_layer_name(self, &views);
        let traces = failures
            .iter()
            .map(|failure| {
                let signature = callable_source_signature(self, &name.text, &failure.view);
                let reason = match &failure.kind {
                    NativeReferenceFailureKind::Member => {
                        "native addresses cannot target member functions".to_string()
                    }
                    NativeReferenceFailureKind::Extension => {
                        "native addresses cannot target extension functions".to_string()
                    }
                    NativeReferenceFailureKind::Generic => {
                        "native addresses cannot target generic functions".to_string()
                    }
                    NativeReferenceFailureKind::Suspend => {
                        "native addresses cannot target suspend functions".to_string()
                    }
                    NativeReferenceFailureKind::RequiresNoGc => {
                        "native callbacks must be declared `@NoGC`".to_string()
                    }
                    NativeReferenceFailureKind::NotUserFunction => {
                        "native addresses require a source function body".to_string()
                    }
                    NativeReferenceFailureKind::Constraint(constraint) => {
                        render_callable_constraint_failure(
                            &failure.state,
                            &failure.view,
                            None,
                            &[],
                            constraint,
                        )
                    }
                };
                format!("  - {signature} — {reason}")
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.error(
            span,
            format!(
                "no applicable candidate for native function reference `::{}` in {layer} layer:\n{traces}",
                name.text
            ),
        );
    }

    fn native_reference_ambiguity_diagnostic(
        &mut self,
        name: &ast::Ident,
        matching: &[ApplicableNativeReference],
        span: Span,
    ) {
        let views = matching
            .iter()
            .map(|candidate| candidate.view.clone())
            .collect::<Vec<_>>();
        let layer = callable_layer_name(self, &views);
        let traces = matching
            .iter()
            .map(|candidate| {
                format!(
                    "  - {} — exactly matches the expected FunPtr signature",
                    callable_source_signature(self, &name.text, &candidate.view)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.error(
            span,
            format!(
                "native function reference `::{}` is ambiguous in {layer} layer:\n{traces}",
                name.text
            ),
        );
    }
}
