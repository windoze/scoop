use super::*;

impl Lowerer {
    pub(in crate::effects) fn imported_body_generic_call(
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
                .zip(application.arguments.substitution(
                    &self.types,
                    &self.enum_applications,
                    &self.struct_applications,
                    &self.class_applications,
                    &self.interface_applications,
                ))
                .collect(),
            span,
        }
    }

    fn generic_call_target(
        &self,
        target: hir::CallableTarget,
        span: scoop_ast::Span,
    ) -> Option<GenericCall> {
        match target {
            hir::CallableTarget::Local(callable) => self.generic_call(callable, span),
            hir::CallableTarget::Application(application) => {
                Some(self.imported_body_generic_call(application, span))
            }
            hir::CallableTarget::Dependency(_) => None,
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
            ExprKind::ContextLookup(_)
            | ExprKind::StringLiteral { .. }
            | ExprKind::IntegerLiteral(_)
            | ExprKind::CharLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::UnitLiteral
            | ExprKind::Local(_)
            | ExprKind::ConstructorReceiver
            | ExprKind::ConstructorParam(_)
            | ExprKind::InitializingClassFieldAccess { .. }
            | ExprKind::ReleaseFieldLoad(_)
            | ExprKind::InitializingStructFieldAccess { .. }
            | ExprKind::GlobalRead(_)
            | ExprKind::GenericDelegateStorageRead(_)
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
                    lambda.definition,
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
                    function.definition,
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
            ExprKind::VariantConstruct { args, .. } => {
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
                if let Some(call) = reference
                    .target
                    .callee(&self.bound_callable_refs)
                    .and_then(|target| self.generic_call_target(target, expr.span))
                {
                    out.push(call);
                }
                if let Some(receiver) = reference.target.receiver() {
                    self.collect_generic_calls_in_expr(receiver, out);
                }
                for capture in &reference.captures {
                    self.collect_generic_calls_in_expr(&capture.source, out);
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
            | ExprKind::CharCode(source)
            | ExprKind::CharFromCodeUnchecked(source)
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
            | ExprKind::ArrayGenerate {
                count: pointer,
                initializer: offset,
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
                if let Some(call) = callee
                    .declared_callable(&self.bound_callable_refs)
                    .and_then(|target| self.generic_call_target(target, expr.span))
                {
                    out.push(call);
                }
                self.collect_generic_calls_in_expr(receiver, out);
                for arg in args {
                    self.collect_generic_calls_in_expr(arg, out);
                }
            }
            ExprKind::Call { callee, args, .. } => {
                match callee {
                    hir::CallableTarget::Local(callee) => record(*callee),
                    hir::CallableTarget::Application(application) => {
                        out.push(self.imported_body_generic_call(*application, expr.span));
                    }
                    hir::CallableTarget::Dependency(_) => {}
                }
                for arg in args {
                    self.collect_generic_calls_in_expr(arg, out);
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
