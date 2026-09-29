use super::*;

impl Lowerer {
    fn imported_body_generic_call(
        &self,
        application: hir::ImportedGenericCallableApplicationId,
        span: scoop_ast::Span,
    ) -> GenericCall {
        let application = &self.imported_generic_applications[application];
        let template = &self.imported_generic_templates[application.template];
        GenericCall {
            callee: GenericCallable::Imported(application.template),
            arguments: template
                .type_parameters
                .ids()
                .into_iter()
                .zip(application.arguments.substitution(&self.types))
                .collect(),
            span,
        }
    }

    pub(in crate::effects) fn collect_generic_calls_in_expr(
        &self,
        expr: &hir::Expr,
        out: &mut Vec<GenericCall>,
    ) {
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
            | ExprKind::ConstructorReceiver
            | ExprKind::ConstructorParam(_)
            | ExprKind::InitializingClassFieldAccess { .. }
            | ExprKind::InitializingStructFieldAccess { .. }
            | ExprKind::GlobalRead(_)
            | ExprKind::GenericDelegateStorageRead(_)
            | ExprKind::SingletonValue(_)
            | ExprKind::ImportedSingletonValue(_)
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
            ExprKind::StructInit { constructor, args } => {
                out.extend(self.generic_struct_constructor_call(*constructor, expr.span));
                for arg in args {
                    self.collect_generic_calls_in_expr(arg, out);
                }
            }
            ExprKind::ClassInit { constructor, args } => {
                out.extend(self.generic_class_constructor_call(*constructor, expr.span));
                for arg in args {
                    self.collect_generic_calls_in_expr(arg, out);
                }
            }
            ExprKind::VariantConstruct { args, .. }
            | ExprKind::ImportedVariantConstruct { args, .. } => {
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
            ExprKind::IntegerOperation { arguments, .. } => match arguments {
                hir::HirIntegerOperationArguments::Unary(operand) => {
                    self.collect_generic_calls_in_expr(operand, out);
                }
                hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                    self.collect_generic_calls_in_expr(lhs, out);
                    self.collect_generic_calls_in_expr(rhs, out);
                }
            },
            ExprKind::IntegerConversion { operand, .. } => {
                self.collect_generic_calls_in_expr(operand, out);
            }
            ExprKind::FunctionCoercion { source, .. }
            | ExprKind::PtrFromNonZeroULong(source)
            | ExprKind::PtrToULong(source)
            | ExprKind::PtrCast(source)
            | ExprKind::Box(source)
            | ExprKind::Unbox(source)
            | ExprKind::ReferenceUpcast(source)
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
            ExprKind::Call { callee, args, .. } => {
                record(*callee);
                for arg in args {
                    self.collect_generic_calls_in_expr(arg, out);
                }
            }
            ExprKind::ImportedConstructorInit { application, args } => {
                let application = &self.imported_constructor_applications[*application];
                let template = &self.imported_constructor_templates[application.template].signature;
                let (_, arguments) = self.types[application.owner]
                    .imported_nominal_application()
                    .expect("constructor applications retain their owner arguments");
                out.push(GenericCall {
                    callee: GenericCallable::ImportedConstructor(application.template),
                    arguments: template
                        .type_parameters
                        .iter()
                        .zip(arguments)
                        .map(|(p, a)| (p.id, *a))
                        .collect(),
                    span: expr.span,
                });
                for argument in args {
                    self.collect_generic_calls_in_expr(argument, out);
                }
            }
            ExprKind::ImportedGenericCall {
                application, args, ..
            } => {
                out.push(self.imported_body_generic_call(*application, expr.span));
                for argument in args {
                    self.collect_generic_calls_in_expr(argument, out);
                }
            }
            ExprKind::ImportedClosure(closure) => {
                out.push(self.imported_body_generic_call(closure.application, expr.span));
                for capture in &closure.captures {
                    self.collect_generic_calls_in_expr(&capture.source, out);
                }
            }
            ExprKind::ImportedMethodCall {
                receiver,
                callee,
                args,
            } => {
                if let Some(hir::ImportedCallableTarget::Application(application)) =
                    callee.declared_callable()
                {
                    out.push(self.imported_body_generic_call(application, expr.span));
                }
                self.collect_generic_calls_in_expr(receiver, out);
                for arg in args {
                    self.collect_generic_calls_in_expr(arg, out);
                }
            }
            ExprKind::ImportedCallableReference(reference) => {
                if let Some(hir::ImportedCallableTarget::Application(application)) =
                    reference.target.callee()
                {
                    out.push(self.imported_body_generic_call(application, expr.span));
                }
                if let Some(receiver) = reference.target.receiver() {
                    self.collect_generic_calls_in_expr(receiver, out);
                }
                for capture in &reference.captures {
                    self.collect_generic_calls_in_expr(&capture.source, out);
                }
            }
            ExprKind::ImportedDependencyCall { args, .. } => {
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
