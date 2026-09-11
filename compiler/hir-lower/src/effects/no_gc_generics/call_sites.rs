use super::*;

impl Lowerer {
    pub(in crate::effects) fn generic_call_sites(&self) -> Vec<GenericCallSite> {
        let mut out = Vec::new();
        for (caller, function) in self.functions.iter() {
            let hir::FunctionKind::User(body) = &function.kind else {
                continue;
            };
            out.extend(
                self.generic_calls_in_body(body)
                    .into_iter()
                    .map(|call| GenericCallSite {
                        caller,
                        callee: call.callee,
                        arguments: call.arguments,
                        span: call.span,
                    }),
            );
        }
        out
    }

    fn generic_call(&self, callable: hir::Callable, span: Span) -> Option<GenericCall> {
        let (callee, arguments) = match callable {
            hir::Callable::Function(_) => return None,
            hir::Callable::Generic(application) => {
                let application = &self.instantiations[application];
                let generic = &self.generic_functions[application.generic];
                let hir::FunctionGenericity::Generic { parameters, .. } =
                    &self.functions[generic.function].genericity
                else {
                    unreachable!("a generic application names a generic function declaration")
                };
                assert_eq!(parameters.len(), application.type_args.len());
                let arguments = parameters
                    .iter()
                    .zip(application.type_args.iter().copied())
                    .map(|(parameter, argument)| (parameter.id, argument))
                    .collect();
                (generic.function, arguments)
            }
            hir::Callable::Method(application) => {
                let application = &self.method_applications[application];
                let hir::FunctionGenericity::OwnerParameterizedMethod {
                    owner_parameters, ..
                } = &self.functions[application.function].genericity
                else {
                    // Parameter-free ordinary methods have no generic GC-free
                    // preconditions and therefore need no call-site record.
                    return None;
                };
                let owner_arguments = self.method_owner_arguments(application.owner);
                assert_eq!(owner_parameters.len(), owner_arguments.len());
                let arguments = owner_parameters
                    .iter()
                    .zip(owner_arguments.iter().copied())
                    .map(|(parameter, argument)| (parameter.id, argument))
                    .collect();
                (application.function, arguments)
            }
            hir::Callable::GenericMethod(application) => {
                let application = &self.generic_method_applications[application];
                let method = &self.generic_methods[application.method];
                let hir::FunctionGenericity::GenericMethod {
                    owner_parameters,
                    method_parameters,
                    ..
                } = &self.functions[method.function].genericity
                else {
                    unreachable!("a generic method application names a generic method declaration")
                };
                let owner_arguments = self.generic_method_owner_arguments(application.owner);
                assert_eq!(owner_parameters.len(), owner_arguments.len());
                assert_eq!(method_parameters.len(), application.method_arguments.len());
                let mut arguments = owner_parameters
                    .iter()
                    .zip(owner_arguments.iter().copied())
                    .map(|(parameter, argument)| (parameter.id, argument))
                    .collect::<Vec<_>>();
                arguments.extend(
                    method_parameters
                        .iter()
                        .zip(application.method_arguments.iter().copied())
                        .map(|(parameter, argument)| (parameter.id, argument)),
                );
                (method.function, arguments)
            }
        };
        Some(GenericCall {
            callee,
            arguments,
            span,
        })
    }

    fn callable_body_generic_call(
        &self,
        function: hir::FunctionId,
        body_type_arguments: &hir::CallableBodyTypeArguments,
        span: Span,
    ) -> Option<GenericCall> {
        let parameters = self.functions[function].type_params();
        if parameters.is_empty() {
            return None;
        }
        let argument_types = match body_type_arguments {
            hir::CallableBodyTypeArguments::Lexical => parameters
                .iter()
                .map(|parameter| {
                    self.types
                        .iter()
                        .find_map(|(ty, candidate)| {
                            matches!(candidate, hir::Type::Param(id) if *id == parameter.id)
                                .then_some(ty)
                        })
                        .expect("every callable body parameter has a canonical parameter type")
                })
                .collect::<Vec<_>>(),
            hir::CallableBodyTypeArguments::Explicit(arguments) => arguments.clone(),
        };
        assert_eq!(parameters.len(), argument_types.len());
        Some(GenericCall {
            callee: function,
            arguments: parameters
                .into_iter()
                .zip(argument_types)
                .map(|(parameter, argument)| (parameter.id, argument))
                .collect(),
            span,
        })
    }

    fn generic_method_owner_arguments(&self, owner: hir::GenericMethodOwner) -> &[hir::TypeId] {
        match owner {
            hir::GenericMethodOwner::Class(id) => &self.class_applications[id].arguments,
            hir::GenericMethodOwner::Struct(id) => &self.struct_applications[id].arguments,
            hir::GenericMethodOwner::Enum(id) => &self.enum_applications[id].arguments,
            hir::GenericMethodOwner::Object(_) => &[],
        }
    }

    pub(in crate::effects) fn generic_calls_in_body(&self, body: &hir::Body) -> Vec<GenericCall> {
        let mut out = Vec::new();
        self.collect_generic_calls_in_statements(&body.statements, &mut out);
        out
    }

    pub(in crate::effects) fn generic_calls_in_default(
        &self,
        default: &hir::ExportDefaultExpr,
    ) -> Vec<GenericCall> {
        let mut out = Vec::new();
        self.collect_generic_calls_in_statements(&default.statements, &mut out);
        self.collect_generic_calls_in_expr(&default.value, &mut out);
        out
    }

    pub(in crate::effects) fn generic_calls_in_struct_constructor(
        &self,
        constructor: &hir::StructConstructor,
    ) -> Vec<GenericCall> {
        let mut out = Vec::new();
        if let hir::StructConstructorKind::Secondary { delegation, body } = &constructor.kind {
            self.collect_generic_calls_in_constructor_arguments(&delegation.arguments, &mut out);
            self.collect_generic_calls_in_statements(&body.statements, &mut out);
        }
        out
    }

    pub(in crate::effects) fn generic_calls_in_class_constructor(
        &self,
        constructor: &hir::ClassConstructor,
    ) -> Vec<GenericCall> {
        let mut out = Vec::new();
        match &constructor.kind {
            hir::ClassConstructorKind::Primary {
                base,
                common_initialization,
                ..
            } => {
                self.collect_generic_calls_in_base_initialization(base, &mut out);
                self.collect_generic_calls_in_class_initialization(common_initialization, &mut out);
            }
            hir::ClassConstructorKind::Secondary { delegation, body } => {
                match delegation {
                    hir::ClassSecondaryDelegation::This { arguments, .. } => {
                        self.collect_generic_calls_in_constructor_arguments(arguments, &mut out)
                    }
                    hir::ClassSecondaryDelegation::Terminal {
                        base,
                        common_initialization,
                    } => {
                        self.collect_generic_calls_in_base_initialization(base, &mut out);
                        self.collect_generic_calls_in_class_initialization(
                            common_initialization,
                            &mut out,
                        );
                    }
                }
                self.collect_generic_calls_in_statements(&body.statements, &mut out);
            }
        }
        out
    }

    fn collect_generic_calls_in_constructor_arguments(
        &self,
        arguments: &hir::ConstructorArguments,
        out: &mut Vec<GenericCall>,
    ) {
        self.collect_generic_calls_in_statements(&arguments.statements, out);
        for argument in &arguments.args {
            self.collect_generic_calls_in_expr(argument, out);
        }
    }

    fn collect_generic_calls_in_base_initialization(
        &self,
        base: &hir::BaseInitialization,
        out: &mut Vec<GenericCall>,
    ) {
        if let hir::BaseInitialization::Super { arguments, .. } = base {
            self.collect_generic_calls_in_constructor_arguments(arguments, out);
        }
    }

    fn collect_generic_calls_in_class_initialization(
        &self,
        initialization: &[hir::ClassInitializationStep],
        out: &mut Vec<GenericCall>,
    ) {
        for step in initialization {
            match step {
                hir::ClassInitializationStep::StoredProperty { initializer, .. }
                | hir::ClassInitializationStep::DelegatedProperty { initializer, .. } => {
                    self.collect_generic_calls_in_statements(&initializer.statements, out);
                    self.collect_generic_calls_in_expr(&initializer.value, out);
                }
                hir::ClassInitializationStep::InitBlock { body, .. } => {
                    self.collect_generic_calls_in_statements(&body.statements, out);
                }
            }
        }
    }

    fn collect_generic_calls_in_statements(
        &self,
        statements: &[hir::Statement],
        out: &mut Vec<GenericCall>,
    ) {
        for statement in statements {
            match &statement.kind {
                hir::StatementKind::InitializationEnsure(_) => {}
                hir::StatementKind::Expr(expr) | hir::StatementKind::Throw(expr) => {
                    self.collect_generic_calls_in_expr(expr, out);
                }
                hir::StatementKind::LocalFunction(_)
                | hir::StatementKind::Break { .. }
                | hir::StatementKind::Continue { .. } => {}
                hir::StatementKind::Return { value } => {
                    if let Some(value) = value {
                        self.collect_generic_calls_in_expr(value, out);
                    }
                }
                hir::StatementKind::ValDecl { pattern, init } => {
                    self.collect_generic_calls_in_pattern(pattern, out);
                    self.collect_generic_calls_in_expr(init, out);
                }
                hir::StatementKind::Assign { target, value } => {
                    match target {
                        hir::AssignTarget::Local(_)
                        | hir::AssignTarget::Global(_)
                        | hir::AssignTarget::SingletonPublishedRoot(_) => {}
                        hir::AssignTarget::Index { array, index } => {
                            self.collect_generic_calls_in_expr(array, out);
                            self.collect_generic_calls_in_expr(index, out);
                        }
                        hir::AssignTarget::Field { receiver, .. } => {
                            self.collect_generic_calls_in_expr(receiver, out);
                        }
                        hir::AssignTarget::InitializingClassField { .. } => {}
                    }
                    self.collect_generic_calls_in_expr(value, out);
                }
                hir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                } => {
                    self.collect_generic_calls_in_expr(cond, out);
                    self.collect_generic_calls_in_statements(then_body, out);
                    if let Some(else_body) = else_body {
                        self.collect_generic_calls_in_statements(else_body, out);
                    }
                }
                hir::StatementKind::While {
                    target: _,
                    condition_setup,
                    cond,
                    body,
                } => {
                    self.collect_generic_calls_in_statements(condition_setup, out);
                    self.collect_generic_calls_in_expr(cond, out);
                    self.collect_generic_calls_in_statements(body, out);
                }
                hir::StatementKind::For(plan) => {
                    self.collect_generic_calls_in_statements(plan.source_setup(), out);
                    self.collect_generic_calls_in_expr(plan.source_init(), out);
                    self.collect_generic_calls_in_statements(plan.iterator_setup(), out);
                    self.collect_generic_calls_in_expr(plan.iterator_call(), out);
                    let next = plan.next();
                    if let Some(call) =
                        self.generic_call(hir::Callable::Method(next.callable()), next.span())
                    {
                        out.push(call);
                    }
                    for action in &plan.binding().actions {
                        if let hir::IrrefutableBindingAction::Component { setup, call, .. } = action
                        {
                            self.collect_generic_calls_in_statements(setup, out);
                            self.collect_generic_calls_in_expr(call, out);
                        }
                    }
                    self.collect_generic_calls_in_statements(plan.body(), out);
                }
                hir::StatementKind::When(when) => {
                    self.collect_generic_calls_in_expr(&when.subject, out);
                    for arm in &when.arms {
                        self.collect_generic_calls_in_pattern(&arm.pattern, out);
                        if let Some(guard) = &arm.guard {
                            self.collect_generic_calls_in_statements(&guard.setup, out);
                            self.collect_generic_calls_in_expr(&guard.condition, out);
                        }
                        self.collect_generic_calls_in_statements(&arm.body, out);
                    }
                    if let hir::WhenFallback::Else(body) = &when.fallback {
                        self.collect_generic_calls_in_statements(body, out);
                    }
                }
                hir::StatementKind::Try(try_) => {
                    self.collect_generic_calls_in_statements(&try_.body, out);
                    for catch in &try_.catches {
                        self.collect_generic_calls_in_statements(&catch.body, out);
                    }
                    if let Some(finally_body) = &try_.finally_body {
                        self.collect_generic_calls_in_statements(finally_body, out);
                    }
                }
            }
        }
    }

    fn collect_generic_calls_in_pattern(&self, pattern: &hir::Pattern, out: &mut Vec<GenericCall>) {
        match pattern {
            hir::Pattern::Literal { value, .. } => self.collect_generic_calls_in_expr(value, out),
            hir::Pattern::Variant { fields, .. } | hir::Pattern::Struct { fields, .. } => {
                for (_, pattern) in fields {
                    self.collect_generic_calls_in_pattern(pattern, out);
                }
            }
            hir::Pattern::Tuple(elements) => {
                for pattern in elements {
                    self.collect_generic_calls_in_pattern(pattern, out);
                }
            }
            hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => {}
        }
    }

    fn collect_generic_calls_in_expr(&self, expr: &hir::Expr, out: &mut Vec<GenericCall>) {
        use hir::ExprKind;

        let mut record = |callable: hir::Callable| {
            if let Some(call) = self.generic_call(callable, expr.span) {
                out.push(call);
            }
        };
        match &expr.kind {
            ExprKind::StringLiteral { .. }
            | ExprKind::IntegerLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::UnitLiteral
            | ExprKind::Local(_)
            | ExprKind::ConstructorParam(_)
            | ExprKind::InitializingClassFieldAccess { .. }
            | ExprKind::InitializingStructFieldAccess { .. }
            | ExprKind::GlobalRead(_)
            | ExprKind::SingletonValue(_)
            | ExprKind::Capture(_)
            | ExprKind::NoneLiteral
            | ExprKind::AddressOf(_)
            | ExprKind::SizeOf(_)
            | ExprKind::AlignOf(_)
            | ExprKind::FunctionAddress(_) => {}
            ExprKind::Lambda(lambda) => {
                let lambda = &self.lambdas[*lambda];
                if let Some(call) = self.callable_body_generic_call(
                    lambda.function,
                    &lambda.body_type_arguments,
                    expr.span,
                ) {
                    out.push(call);
                }
                for capture in &lambda.captures {
                    self.collect_generic_calls_in_expr(&capture.source, out);
                }
            }
            ExprKind::AnonymousFunction(function) => {
                let function = &self.anonymous_functions[*function];
                if let Some(call) = self.callable_body_generic_call(
                    function.function,
                    &function.body_type_arguments,
                    expr.span,
                ) {
                    out.push(call);
                }
                for capture in &function.captures {
                    self.collect_generic_calls_in_expr(&capture.source, out);
                }
            }
            ExprKind::TupleLiteral(elements) | ExprKind::ArrayLiteral(elements) => {
                for element in elements {
                    self.collect_generic_calls_in_expr(element, out);
                }
            }
            ExprKind::ArrayAssembly(assembly) => {
                for part in &assembly.parts {
                    match part {
                        hir::ArrayAssemblyPart::Element(value)
                        | hir::ArrayAssemblyPart::CopyArray(value) => {
                            self.collect_generic_calls_in_expr(value, out)
                        }
                    }
                }
            }
            ExprKind::StructInit { args, .. }
            | ExprKind::ClassInit { args, .. }
            | ExprKind::VariantConstruct { args, .. } => {
                for arg in args {
                    self.collect_generic_calls_in_expr(arg, out);
                }
            }
            ExprKind::StructConstruct { fields, .. } => {
                for field in fields {
                    self.collect_generic_calls_in_expr(field, out);
                }
            }
            ExprKind::VariantTest { operand, .. }
            | ExprKind::VariantPayloadProject { operand, .. } => {
                self.collect_generic_calls_in_expr(operand, out);
            }
            ExprKind::CallableReference(reference) => {
                let reference = &self.callable_references[*reference];
                match &reference.target {
                    hir::CallableReferenceTarget::Named(callee) => record(*callee),
                    hir::CallableReferenceTarget::Local { callee, .. } => record(*callee),
                    hir::CallableReferenceTarget::BoundMember { receiver, callee } => {
                        if let hir::MethodCallee::Callable(callee) = callee {
                            record(*callee);
                        }
                        self.collect_generic_calls_in_expr(receiver, out);
                    }
                    hir::CallableReferenceTarget::BoundExtension { receiver, callee } => {
                        record(*callee);
                        self.collect_generic_calls_in_expr(receiver, out);
                    }
                }
            }
            ExprKind::IntegerOperation {
                operation,
                arguments,
            } => {
                record(hir::Callable::Function(match operation {
                    hir::IntegerOperation::NoGc { target, .. } => target.function(),
                    hir::IntegerOperation::Managed { target, .. } => target.function(),
                }));
                match arguments {
                    hir::HirIntegerOperationArguments::Unary(operand) => {
                        self.collect_generic_calls_in_expr(operand, out);
                    }
                    hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                        self.collect_generic_calls_in_expr(lhs, out);
                        self.collect_generic_calls_in_expr(rhs, out);
                    }
                }
            }
            ExprKind::IntegerConversion {
                conversion,
                operand,
            } => {
                record(hir::Callable::Function(conversion.target.function()));
                self.collect_generic_calls_in_expr(operand, out);
            }
            ExprKind::FunctionCoercion { source, .. }
            | ExprKind::PtrFromNonZeroULong(source)
            | ExprKind::PtrToULong(source)
            | ExprKind::PtrCast(source)
            | ExprKind::Box(source)
            | ExprKind::Unbox(source)
            | ExprKind::IsInstance {
                operand: source, ..
            }
            | ExprKind::Cast {
                operand: source, ..
            }
            | ExprKind::ArrayLen(source)
            | ExprKind::ArrayClone(source)
            | ExprKind::Unary {
                operand: source, ..
            }
            | ExprKind::PrimitiveUnary {
                operand: source, ..
            }
            | ExprKind::SomeWrap(source)
            | ExprKind::IsSome(source)
            | ExprKind::Unwrap {
                operand: source, ..
            } => self.collect_generic_calls_in_expr(source, out),
            ExprKind::PtrLoad { pointer, offset } => {
                self.collect_generic_calls_in_expr(pointer, out);
                if let Some(offset) = offset {
                    self.collect_generic_calls_in_expr(offset, out);
                }
            }
            ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => {
                self.collect_generic_calls_in_expr(pointer, out);
                if let Some(offset) = offset {
                    self.collect_generic_calls_in_expr(offset, out);
                }
                self.collect_generic_calls_in_expr(value, out);
            }
            ExprKind::PtrOffset {
                pointer, offset, ..
            }
            | ExprKind::Index {
                receiver: pointer,
                index: offset,
                ..
            }
            | ExprKind::PrimitiveBinary {
                lhs: pointer,
                rhs: offset,
                ..
            }
            | ExprKind::Binary {
                lhs: pointer,
                rhs: offset,
                ..
            } => {
                self.collect_generic_calls_in_expr(pointer, out);
                self.collect_generic_calls_in_expr(offset, out);
            }
            ExprKind::ArraySet {
                receiver,
                index,
                value,
                ..
            } => {
                self.collect_generic_calls_in_expr(receiver, out);
                self.collect_generic_calls_in_expr(index, out);
                self.collect_generic_calls_in_expr(value, out);
            }
            ExprKind::FieldAccess { receiver, .. } => {
                self.collect_generic_calls_in_expr(receiver, out);
            }
            ExprKind::MethodCall {
                receiver,
                callee,
                args,
            }
            | ExprKind::DirectSuperMethodCall {
                receiver,
                callee,
                args,
            } => {
                if let hir::MethodCallee::Callable(callee) = callee {
                    record(*callee);
                }
                self.collect_generic_calls_in_expr(receiver, out);
                for arg in args {
                    self.collect_generic_calls_in_expr(arg, out);
                }
            }
            ExprKind::Call { callee, args } => {
                record(*callee);
                for arg in args {
                    self.collect_generic_calls_in_expr(arg, out);
                }
            }
            ExprKind::LocalFunctionCall {
                callee,
                captures,
                args,
                ..
            } => {
                record(*callee);
                for value in captures.iter().chain(args) {
                    self.collect_generic_calls_in_expr(value, out);
                }
            }
            ExprKind::CallableCall { callee, args, .. } => {
                self.collect_generic_calls_in_expr(callee, out);
                for arg in args {
                    self.collect_generic_calls_in_expr(arg, out);
                }
            }
            ExprKind::ForeignCallbackRegister { closure, .. } => {
                self.collect_generic_calls_in_expr(closure, out);
            }
            ExprKind::ForeignCallbackOperation { callback, .. } => {
                self.collect_generic_calls_in_expr(callback, out);
            }
        }
    }
}
