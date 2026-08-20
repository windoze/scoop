use std::collections::HashSet;

use super::*;

impl Lowerer {
    pub(crate) fn bindings_skipped_by_continue(
        &self,
        body: &[hir::Statement],
        target: hir::LoopId,
    ) -> HashSet<hir::BindingId> {
        let flow = Flow {
            is_nothing: &|ty| self.is_nothing_ty(ty),
        };
        let mut outcomes = HirControlOutcomes::fallthrough();
        let mut skipped = HashSet::new();
        for statement in body {
            if outcomes.values.contains(&ControlOutcome::Continue(target))
                && let hir::StatementKind::ValDecl { pattern, .. } = &statement.kind
            {
                self.collect_pattern_bindings(pattern, &mut skipped);
            }
            outcomes = outcomes.then(flow.statement(statement));
        }
        skipped
    }

    fn collect_pattern_bindings(
        &self,
        pattern: &hir::Pattern,
        result: &mut HashSet<hir::BindingId>,
    ) {
        match pattern {
            hir::Pattern::Binding { local } => {
                result.insert(self.locals[*local].binding);
            }
            hir::Pattern::Tuple(elements) => {
                for element in elements {
                    self.collect_pattern_bindings(element, result);
                }
            }
            hir::Pattern::Variant { fields, .. } | hir::Pattern::Struct { fields, .. } => {
                for (_, field) in fields {
                    self.collect_pattern_bindings(field, result);
                }
            }
            hir::Pattern::Literal { .. } | hir::Pattern::Wildcard => {}
        }
    }

    pub(crate) fn require_initialized_condition_binding(
        &mut self,
        binding: hir::BindingId,
        name: &str,
        span: Span,
    ) -> Option<()> {
        if self.uninitialized_condition_bindings.contains(&binding) {
            self.error(span, format!("local `{name}` may be uninitialized when a do-while continue reaches the condition"));
            None
        } else {
            Some(())
        }
    }
}
