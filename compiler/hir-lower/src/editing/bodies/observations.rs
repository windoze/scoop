use super::*;
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
pub(crate) struct BodyObservation {
    pub(super) function: hir::FunctionId,
    pub(super) file: usize,
    check_cancelled: Rc<dyn Fn() -> bool>,
    cancelled: Rc<Cell<bool>>,
    pub(super) failed: bool,
    pub(super) bindings: imbl::HashMap<hir::BindingId, hir::EditingLocalOccurrence>,
    pub(super) uses:
        imbl::HashMap<ast::Span, (hir::BindingId, hir::SourceContextId, hir::EditingLocalRole)>,
    pub(super) expressions: imbl::HashMap<ast::Span, hir::EditingExpression>,
    pub(super) call_names: imbl::HashSet<ast::Span>,
    pub(super) callables: imbl::HashMap<ast::Span, hir::EditingCallableReference>,
    pub(super) contexts: imbl::HashMap<hir::SourceContextId, ObservedContext>,
}

#[derive(Clone)]
pub(super) struct ObservedContext {
    pub(super) owner: Option<crate::Owner>,
    pub(super) parameters: Vec<hir::TypeParamDecl>,
}

pub(crate) struct CheckedBody {
    pub(super) facts: hir::EditingBodyFacts,
    pub(super) contexts: imbl::HashMap<hir::SourceContextId, ObservedContext>,
}

impl BodyObservation {
    pub(super) fn new(
        function: hir::FunctionId,
        file: usize,
        check: impl Fn() -> bool + 'static,
    ) -> Self {
        Self {
            function,
            file,
            check_cancelled: Rc::new(check),
            cancelled: Rc::new(Cell::new(false)),
            failed: false,
            bindings: Default::default(),
            uses: Default::default(),
            expressions: Default::default(),
            call_names: Default::default(),
            callables: Default::default(),
            contexts: Default::default(),
        }
    }

    pub(super) fn cancelled(&self) -> bool {
        if (self.check_cancelled)() {
            self.cancelled.set(true);
        }
        self.cancelled.get()
    }

    pub(super) fn finish(self, lowerer: &Lowerer) -> CheckedBody {
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
        CheckedBody {
            facts: hir::EditingBodyFacts {
                locals,
                expressions,
                callables,
            },
            contexts: self.contexts,
        }
    }
}
