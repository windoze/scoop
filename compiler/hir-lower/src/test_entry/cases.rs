use super::*;

impl Lowerer {
    pub(super) fn generate_test_case(
        &mut self,
        entry: hir::FunctionId,
        container: &hir::TestContainer,
        case: &hir::TestCase,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let outer_file = std::mem::replace(&mut self.current_file, container.origin.file as usize);
        let outer_owner = self
            .current_owner
            .replace(crate::Owner::Class(container.class));
        let outer_context = self.current_source_context;
        self.set_source_context(hir::SourceContextSubject::Function(entry));
        let result = self.generate_test_case_inner(container, case, sink);
        self.current_file = outer_file;
        self.current_owner = outer_owner;
        self.current_source_context = outer_context;
        result
    }

    fn generate_test_case_inner(
        &mut self,
        container: &hir::TestContainer,
        case: &hir::TestCase,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let span = case.method.origin.span;
        let actions = self.test_api_type("TestActions", span)?;
        let action = self.test_api_type("TestAction", span)?;
        let initializer =
            self.lower_synthesized_lambda(false, &[], actions, span, |state, _| {
                let mut statements = Vec::new();
                let instance = state.construct_test_instance(container, span, &mut statements)?;
                let instance = state.materialize_temporary(
                    "$test_instance".into(),
                    instance,
                    span,
                    &mut statements,
                );
                let hir::ExprKind::Local(local) = instance.kind else {
                    unreachable!("the initialized instance is a local")
                };
                state.scopes.declare("$test_instance".into(), local);
                let binding = state.locals[local].binding;
                let body = state.generate_test_action(binding, &case.method, &mut statements)?;
                let before = match &container.before {
                    Some(method) => {
                        Some(state.generate_test_action(binding, method, &mut statements)?)
                    }
                    None => None,
                };
                let after = match &container.after {
                    Some(method) => {
                        Some(state.generate_test_action(binding, method, &mut statements)?)
                    }
                    None => None,
                };
                let before = state.test_optional_action(before, action, span);
                let after = state.test_optional_action(after, action, span);
                let value = state.test_api_call(
                    "TestActions",
                    vec![body, before, after],
                    span,
                    &mut statements,
                )?;
                Some(crate::stmt::ValueBlock {
                    statements,
                    value: Some(value),
                })
            })?;
        let location = self.test_location(container.origin, sink)?;
        self.test_api_call("runTestCase", vec![location, initializer], span, sink)
    }

    fn generate_test_action(
        &mut self,
        instance: hir::BindingId,
        method: &hir::TestMethod,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let span = method.origin.span;
        let callable = self.lower_synthesized_lambda(false, &[], self.unit, span, |state, _| {
            let receiver = state.lower_capture_binding(instance, "$test_instance", span)?;
            let hir::Type::Class(owner) = state.types[receiver.ty] else {
                unreachable!("a test instance has its selected class type")
            };
            let method = state.record_method_application(
                method.function,
                hir::MethodOwnerApplication::Class(owner),
            );
            let value = hir::Expr {
                kind: hir::ExprKind::MethodCall {
                    receiver: Box::new(receiver),
                    callee: hir::MethodCallee::Callable(hir::Callable::Method(method).into()),
                    args: Vec::new(),
                },
                ty: state.unit,
                span,
                origin: state.expression_origin(span),
            };
            Some(crate::stmt::ValueBlock {
                statements: Vec::new(),
                value: Some(value),
            })
        })?;
        let location = self.test_location(method.origin, sink)?;
        self.test_api_call("TestAction", vec![callable, location], span, sink)
    }
}
