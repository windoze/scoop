use super::*;
use crate::pipeline::cancellation::FrontendCancellation;
use std::rc::Rc;

#[derive(Clone)]
pub(crate) struct BodyObservation {
    pub(super) function: hir::FunctionId,
    pub(super) file: usize,
    pub(super) diagnostics_start: usize,
    cancellation: FrontendCancellation,
    pub(super) failed: bool,
    pub(super) bindings: imbl::HashMap<hir::BindingId, hir::EditingLocalOccurrence>,
    pub(super) uses:
        imbl::HashMap<ast::Span, (hir::BindingId, hir::SourceContextId, hir::EditingLocalRole)>,
    pub(super) expressions: imbl::HashMap<ast::Span, hir::EditingExpression>,
    pub(super) call_names: imbl::HashSet<ast::Span>,
    pub(super) callables: imbl::HashMap<ast::Span, hir::EditingCallableReference>,
    pub(super) call_inputs: imbl::HashMap<ast::Span, calls::CallInput>,
    pub(super) argument_maps: imbl::HashMap<ast::Span, calls::CheckedArguments>,
    pub(super) calls: imbl::HashMap<ast::Span, hir::EditingAppliedCall>,
    pub(super) selector_inputs: imbl::HashMap<ast::Span, Vec<selectors::SelectorInput>>,
    pub(super) selectors: imbl::HashMap<ast::Span, hir::EditingSelector>,
    pub(super) contexts: imbl::HashMap<hir::SourceContextId, ObservedContext>,
}

#[derive(Clone)]
pub(super) struct ObservedContext {
    pub(super) owner: Option<crate::Owner>,
    pub(super) parameters: Vec<hir::TypeParamDecl>,
}

pub(crate) struct ObservedBody {
    pub(super) facts: V<hir::EditingBodyFacts>,
    pub(super) selectors: Vec<hir::EditingSelector>,
    pub(super) calls: Vec<hir::EditingAppliedCall>,
    pub(super) contexts: imbl::HashMap<hir::SourceContextId, ObservedContext>,
}

impl BodyObservation {
    pub(super) fn new(
        function: hir::FunctionId,
        file: usize,
        diagnostics_start: usize,
        check: impl Fn() -> bool + 'static,
    ) -> Self {
        Self {
            function,
            file,
            diagnostics_start,
            cancellation: FrontendCancellation::new(check),
            failed: false,
            bindings: Default::default(),
            uses: Default::default(),
            expressions: Default::default(),
            call_names: Default::default(),
            callables: Default::default(),
            call_inputs: Default::default(),
            argument_maps: Default::default(),
            calls: Default::default(),
            selector_inputs: Default::default(),
            selectors: Default::default(),
            contexts: Default::default(),
        }
    }

    pub(super) fn cancelled(&self) -> bool {
        self.cancellation.cancelled()
    }

    pub(super) fn cancellation(&self) -> Rc<dyn Fn() -> bool> {
        let cancellation = self.cancellation.clone();
        Rc::new(move || cancellation.cancelled())
    }

    pub(super) fn finish(self, lowerer: &Lowerer, complete: bool) -> ObservedBody {
        let local_type = |binding, ty| {
            lowerer
                .local_delegate_plans
                .get(&binding)
                .map_or(ty, |plan| plan.property_ty)
        };
        let mut locals = self
            .bindings
            .values()
            .cloned()
            .map(|mut definition| {
                definition.value_type =
                    local_type(definition.reference.binding, definition.value_type);
                definition
            })
            .collect::<Vec<_>>();
        for (span, (binding, context, role)) in &self.uses {
            let Some(definition) = self.bindings.get(binding) else {
                continue;
            };
            let value_type = self.expressions.get(span).map_or_else(
                || local_type(*binding, definition.value_type),
                |expression| expression.value_type,
            );
            locals.push(hir::EditingLocalOccurrence {
                span: *span,
                context: *context,
                reference: definition.reference.clone(),
                value_type,
                role: *role,
            });
        }
        locals.sort_by_key(|local| (local.span.start, local.span.end));
        let mut expressions = self
            .expressions
            .into_iter()
            .map(|(_, expression)| expression)
            .collect::<Vec<_>>();
        expressions.sort_by_key(|expression| (expression.span.start, expression.span.end));
        let mut callables = self
            .callables
            .into_iter()
            .map(|(_, reference)| reference)
            .collect::<Vec<_>>();
        callables.sort_by_key(|reference| (reference.span.start, reference.span.end));
        let mut selectors = self
            .selectors
            .into_iter()
            .map(|(_, value)| value)
            .collect::<Vec<_>>();
        selectors.sort_by_key(|selector| selector.selector.start);
        let mut calls = self
            .calls
            .into_iter()
            .map(|(_, call)| call)
            .collect::<Vec<_>>();
        calls.sort_by_key(|call| (call.span.start, call.span.end));
        ObservedBody {
            facts: if complete {
                V::Known(hir::EditingBodyFacts {
                    locals,
                    expressions,
                    callables,
                })
            } else {
                V::Unavailable(U::BodyFailure)
            },
            selectors,
            calls,
            contexts: self.contexts,
        }
    }
}
