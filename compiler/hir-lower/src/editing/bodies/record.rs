use super::*;
use observations::ObservedContext;

impl Lowerer {
    pub(crate) fn observe_local_function(
        &mut self,
        function: hir::FunctionId,
        declaration: ast::FunctionHeaderRef<'_>,
    ) {
        if !self.observes_current_body() {
            return;
        }
        let name_origin = hir::SemanticSourceLocation {
            source: self.source_identity(self.current_file).clone(),
            span: declaration.name.span,
        };
        self.editing_body
            .as_mut()
            .expect("active query")
            .local_functions
            .insert(
                function,
                hir::EditingFunction {
                    function,
                    name: declaration.name.text.clone(),
                    name_origin,
                    span: declaration.span,
                },
            );
    }

    pub(crate) fn body_cancelled(&self) -> bool {
        self.cancellation
            .as_ref()
            .is_some_and(|cancel| cancel.cancelled())
            || self
                .editing_body
                .as_ref()
                .is_some_and(BodyObservation::cancelled)
    }

    pub(crate) fn observes_current_body(&self) -> bool {
        self.editing_body.as_ref().is_some_and(|body| {
            body.file == self.current_file && self.definition_root == Some(body.root)
        })
    }

    pub(crate) fn observe_expression_failure(&mut self) {
        if self.observes_current_body() {
            self.editing_body.as_mut().expect("active query").failed = true;
        }
    }

    pub(in crate::editing) fn observe_body_context(&mut self) -> hir::SourceContextId {
        let context = self
            .current_source_context
            .expect("a body has its source context");
        let body = self.editing_body.as_mut().expect("active query");
        body.contexts
            .entry(context)
            .or_insert_with(|| ObservedContext {
                file: self.current_file,
                owner: self.current_owner,
                parameters: self.type_params_in_scope.clone(),
            });
        context
    }

    pub(crate) fn observe_expression(
        &mut self,
        span: ast::Span,
        value: &hir::Expr,
        expected_type: V<Option<hir::TypeId>>,
    ) {
        if !self.observes_current_body() {
            return;
        }
        let context = self.observe_body_context();
        self.observe_completed_call(span, value.ty, context);
        self.observe_selector_receiver(span, value, context);
        self.editing_body
            .as_mut()
            .expect("active query")
            .expressions
            .insert(
                span,
                hir::EditingExpression {
                    span,
                    context,
                    value_type: value.ty,
                    expected_type,
                },
            );
    }

    pub(crate) fn observe_local_definition(&mut self, local: hir::LocalId) {
        if !self.observes_current_body() {
            return;
        }
        let local = &self.locals[local];
        let hir::LocalValueDefinitionSite::Source(definition) = local.definition else {
            return;
        };
        if definition.file as usize != self.current_file
            || (definition.span.start == definition.span.end
                && !matches!(
                    local.selector,
                    scoop_identity::LocalValueSelector::Parameter { .. }
                ))
            || matches!(local.selector, scoop_identity::LocalValueSelector::This)
        {
            return;
        }
        let reference = self
            .editing_local_reference(local)
            .expect("source local was checked");
        let value_type = local.ty;
        let context = self.observe_body_context();
        let occurrence = hir::EditingLocalOccurrence {
            span: definition.span,
            context,
            reference,
            value_type,
            role: hir::EditingLocalRole::Declaration,
        };
        self.editing_local_definitions
            .insert(occurrence.reference.binding, occurrence.clone());
        self.editing_body
            .as_mut()
            .expect("active query")
            .bindings
            .insert(occurrence.reference.binding, occurrence);
    }

    pub(crate) fn observe_local_use(
        &mut self,
        binding: hir::BindingId,
        span: ast::Span,
        role: hir::EditingLocalRole,
    ) {
        if !self.observes_current_body() {
            return;
        }
        let context = self.observe_body_context();
        let body = self.editing_body.as_mut().expect("active query");
        let previous = body.uses.get(&span).copied();
        let role = match previous {
            Some((previous, _, previous_role)) if previous == binding && previous_role != role => {
                hir::EditingLocalRole::ReadWrite
            }
            _ => role,
        };
        body.uses.insert(span, (binding, context, role));
    }
}
