use super::*;
use std::sync::Arc;

impl BodyLowerer<'_> {
    pub(super) fn statement_origin(&mut self, span: Span) -> mir::DebugOrigin {
        let source = self
            .module
            .debug_sources
            .function(self.current_materialization, self.debug_records);
        let origin = self
            .module
            .debug_sources
            .in_source(&source, span.start, self.debug_records);
        self.debug_records.scoped(&origin, self.debug_scope.clone())
    }

    pub(super) fn enter_debug_scope(&mut self, span: Span) -> Option<Arc<mir::DebugScope>> {
        let origin = self.statement_origin(span);
        let parent = self.debug_scope.clone();
        self.debug_scope = self.debug_records.scope(parent.clone(), &origin);
        parent
    }

    pub(super) fn bind_debug_local(&mut self, local: mir::LocalId) {
        self.debug_records.scope_variable(
            &mut self.locals[local].debug_variable,
            self.debug_scope.clone(),
        );
    }
}
