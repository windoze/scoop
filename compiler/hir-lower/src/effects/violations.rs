use super::*;

impl Lowerer {
    pub(super) fn collect_no_gc_statement_violations(
        &self,
        statements: &[hir::Statement],
        out: &mut Vec<(Span, String)>,
        requirements: &mut HashSet<hir::TypeParamId>,
    ) {
        for statement in statements {
            match &statement.kind {
                hir::StatementKind::InitializationEnsure(_)
                | hir::StatementKind::GenericDelegateEnsure(_) => out.push((
                    statement.span,
                    "an initialization gate is not allowed in `@NoGC` code".to_string(),
                )),
                hir::StatementKind::Expr(expr) => {
                    self.collect_no_gc_expr_violations(expr, out, requirements)
                }
                hir::StatementKind::LocalFunction(_)
                | hir::StatementKind::Break { .. }
                | hir::StatementKind::Continue { .. } => {}
                hir::StatementKind::Return { value } => {
                    if let Some(value) = value {
                        self.collect_no_gc_expr_violations(value, out, requirements);
                    }
                }
                hir::StatementKind::ValDecl { init, .. } => {
                    self.collect_no_gc_expr_violations(init, out, requirements)
                }
                hir::StatementKind::Assign { target, value } => {
                    match target {
                        hir::AssignTarget::Local(_)
                        | hir::AssignTarget::Global(_)
                        | hir::AssignTarget::GenericDelegateStorage(_)
                        | hir::AssignTarget::SingletonPublishedRoot(_) => {}
                        hir::AssignTarget::Index { array, index } => {
                            out.push((
                                statement.span,
                                "array assignment is not allowed in `@NoGC` code".to_string(),
                            ));
                            self.collect_no_gc_expr_violations(array, out, requirements);
                            self.collect_no_gc_expr_violations(index, out, requirements);
                        }
                        hir::AssignTarget::Field { receiver, .. } => {
                            out.push((
                                statement.span,
                                "managed field assignment is not allowed in `@NoGC` code"
                                    .to_string(),
                            ));
                            self.collect_no_gc_expr_violations(receiver, out, requirements);
                        }
                        hir::AssignTarget::InitializingClassField { .. } => out.push((
                            statement.span,
                            "managed field assignment is not allowed in `@NoGC` code".to_string(),
                        )),
                    }
                    self.collect_no_gc_expr_violations(value, out, requirements);
                }
                hir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                } => {
                    self.collect_no_gc_expr_violations(cond, out, requirements);
                    self.collect_no_gc_statement_violations(then_body, out, requirements);
                    if let Some(else_body) = else_body {
                        self.collect_no_gc_statement_violations(else_body, out, requirements);
                    }
                }
                hir::StatementKind::While {
                    target: _,
                    condition_setup,
                    cond,
                    body,
                } => {
                    self.collect_no_gc_statement_violations(condition_setup, out, requirements);
                    self.collect_no_gc_expr_violations(cond, out, requirements);
                    self.collect_no_gc_statement_violations(body, out, requirements);
                }
                hir::StatementKind::When(when) => {
                    self.collect_no_gc_expr_violations(&when.subject, out, requirements);
                    for arm in &when.arms {
                        if let Some(guard) = &arm.guard {
                            self.collect_no_gc_statement_violations(
                                &guard.setup,
                                out,
                                requirements,
                            );
                            self.collect_no_gc_expr_violations(&guard.condition, out, requirements);
                        }
                        self.collect_no_gc_statement_violations(&arm.body, out, requirements);
                    }
                    if let hir::WhenFallback::Else(body) = &when.fallback {
                        self.collect_no_gc_statement_violations(body, out, requirements);
                    }
                }
                hir::StatementKind::Try(_) => out.push((
                    statement.span,
                    "`try`/`catch` is not allowed in `@NoGC` code".to_string(),
                )),
                hir::StatementKind::Throw(expr) => {
                    out.push((
                        statement.span,
                        "`throw` is not allowed in `@NoGC` code".to_string(),
                    ));
                    self.collect_no_gc_expr_violations(expr, out, requirements);
                }
            }
        }
    }

    pub(super) fn collect_no_gc_expr_violations(
        &self,
        expr: &hir::Expr,
        out: &mut Vec<(Span, String)>,
        requirements: &mut HashSet<hir::TypeParamId>,
    ) {
        use hir::ExprKind;
        self.collect_no_gc_type_violations(expr.ty, expr.span, out, requirements);
        match &expr.kind {
            ExprKind::StringLiteral { .. } => out.push((
                expr.span,
                "string literals are not allowed in `@NoGC` code".to_string(),
            )),
            ExprKind::IntegerLiteral(_)
            | ExprKind::BoolLiteral(_)
            | ExprKind::UnitLiteral
            | ExprKind::Local(_)
            | ExprKind::ConstructorReceiver
            | ExprKind::ConstructorParam(_)
            | ExprKind::GlobalRead(_)
            | ExprKind::GenericDelegateStorageRead(_)
            | ExprKind::Capture(_)
            | ExprKind::InitializingStructFieldAccess { .. }
            | ExprKind::NoneLiteral => {}
            ExprKind::SingletonValue(_) => out.push((
                expr.span,
                "singleton access may initialize, allocate, and throw in `@NoGC` code".to_string(),
            )),
            ExprKind::TupleLiteral(elements) | ExprKind::ArrayLiteral(elements) => {
                if matches!(&expr.kind, ExprKind::ArrayLiteral(_)) {
                    out.push((
                        expr.span,
                        "array allocation is not allowed in `@NoGC` code".to_string(),
                    ));
                }
                for element in elements {
                    self.collect_no_gc_expr_violations(element, out, requirements);
                }
            }
            ExprKind::ArrayGenerate { count, initializer } => {
                out.push((
                    expr.span,
                    "array allocation is not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(count, out, requirements);
                self.collect_no_gc_expr_violations(initializer, out, requirements);
            }
            ExprKind::ArrayAssembly(assembly) => {
                out.push((
                    expr.span,
                    "array allocation is not allowed in `@NoGC` code".to_string(),
                ));
                for part in &assembly.parts {
                    match part {
                        hir::ArrayAssemblyPart::Element(value)
                        | hir::ArrayAssemblyPart::CopyArray(value) => {
                            self.collect_no_gc_expr_violations(value, out, requirements)
                        }
                    }
                }
            }
            ExprKind::StructInit { constructor, args } => {
                self.check_no_gc_constructor_call(
                    self.struct_constructor_applications[*constructor].constructor,
                    expr.span,
                    out,
                );
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out, requirements);
                }
            }
            ExprKind::VariantConstruct { args, .. } => {
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out, requirements);
                }
            }
            ExprKind::StructConstruct { fields, .. } => {
                for field in fields {
                    self.collect_no_gc_expr_violations(field, out, requirements);
                }
            }
            ExprKind::VariantTest { operand, .. }
            | ExprKind::VariantPayloadProject { operand, .. } => {
                self.collect_no_gc_expr_violations(operand, out, requirements);
            }
            ExprKind::ClassInit { args, .. } => {
                out.push((
                    expr.span,
                    "class allocation is not allowed in `@NoGC` code".to_string(),
                ));
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out, requirements);
                }
            }
            ExprKind::Lambda(_)
            | ExprKind::AnonymousFunction(_)
            | ExprKind::CallableReference(_)
            | ExprKind::FunctionCoercion { .. } => out.push((
                expr.span,
                "managed function values are not allowed in `@NoGC` code".to_string(),
            )),
            ExprKind::FieldAccess { receiver, field } => {
                if matches!(field, hir::FieldRef::ClassField { .. }) {
                    out.push((
                        expr.span,
                        "managed field access is not allowed in `@NoGC` code".to_string(),
                    ));
                }
                self.collect_no_gc_expr_violations(receiver, out, requirements);
            }
            ExprKind::InitializingClassFieldAccess { .. } => out.push((
                expr.span,
                "managed field access is not allowed in `@NoGC` code".to_string(),
            )),
            ExprKind::ReleaseFieldLoad(_) => {}
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
                let span = expr.origin.concrete().evaluation.span;
                if let Some(target) = callee.declared_callable(&self.bound_callable_refs) {
                    self.check_no_gc_call_target(target, span, out);
                } else if let hir::MethodCallee::ImportedDerivedEquality { owner, .. } = callee {
                    out.push((
                        span,
                        format!(
                            "`@NoGC` code may not call managed function `{}.equals`",
                            self.type_name(*owner)
                        ),
                    ));
                } else if let hir::MethodCallee::DerivedEquality(application) = callee {
                    self.check_no_gc_function(
                        self.derived_equality_applications[*application].function,
                        span,
                        out,
                    );
                }
                self.collect_no_gc_expr_violations(receiver, out, requirements);
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out, requirements);
                }
            }
            ExprKind::Box(operand) | ExprKind::Unbox(operand) => {
                out.push((
                    expr.span,
                    "boxing and unboxing are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(operand, out, requirements);
            }
            ExprKind::ReferenceUpcast(operand) => {
                self.collect_no_gc_expr_violations(operand, out, requirements);
            }
            ExprKind::IsInstance { operand, .. } | ExprKind::Cast { operand, .. } => {
                out.push((
                    expr.span,
                    "runtime type checks are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(operand, out, requirements);
            }
            ExprKind::Index {
                receiver, index, ..
            } => {
                out.push((
                    expr.span,
                    "array indexing is not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(receiver, out, requirements);
                self.collect_no_gc_expr_violations(index, out, requirements);
            }
            ExprKind::ArraySet {
                receiver,
                index,
                value,
                ..
            } => {
                out.push((
                    expr.span,
                    "array operations are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(receiver, out, requirements);
                self.collect_no_gc_expr_violations(index, out, requirements);
                self.collect_no_gc_expr_violations(value, out, requirements);
            }
            ExprKind::ArrayLen(operand) | ExprKind::ArrayClone(operand) => {
                out.push((
                    expr.span,
                    "array operations are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(operand, out, requirements);
            }
            ExprKind::Call { callee, args, .. } => {
                self.check_no_gc_call_target(*callee, expr.span, out);
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out, requirements);
                }
            }

            ExprKind::CallableCall { callee, args, .. } => {
                out.push((
                    expr.span,
                    "managed function-value calls are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(callee, out, requirements);
                for arg in args {
                    self.collect_no_gc_expr_violations(arg, out, requirements);
                }
            }
            ExprKind::ForeignCallbackRegister { closure, .. } => {
                out.push((
                    expr.span,
                    "managed callback registration is not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(closure, out, requirements);
            }
            ExprKind::ForeignCallbackOperation { callback, .. } => {
                out.push((
                    expr.span,
                    "managed callback token operations are not allowed in `@NoGC` code".to_string(),
                ));
                self.collect_no_gc_expr_violations(callback, out, requirements);
            }
            ExprKind::PrimitiveBinary { kind, lhs, rhs } => {
                if *kind == hir::PrimitiveBinaryKind::StringConcat {
                    out.push((
                        expr.span,
                        "string concatenation is not allowed in `@NoGC` code".to_string(),
                    ));
                }
                self.collect_no_gc_expr_violations(lhs, out, requirements);
                self.collect_no_gc_expr_violations(rhs, out, requirements);
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                self.collect_no_gc_expr_violations(lhs, out, requirements);
                self.collect_no_gc_expr_violations(rhs, out, requirements);
            }
            ExprKind::IntegerOperation {
                operation,
                arguments,
            } => {
                if matches!(operation, hir::IntegerOperation::Managed { .. }) {
                    out.push((
                        expr.span,
                        "integer division is not allowed in `@NoGC` code because it may throw"
                            .to_string(),
                    ));
                }
                match arguments {
                    hir::HirIntegerOperationArguments::Unary(operand) => {
                        self.collect_no_gc_expr_violations(operand, out, requirements);
                    }
                    hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                        self.collect_no_gc_expr_violations(lhs, out, requirements);
                        self.collect_no_gc_expr_violations(rhs, out, requirements);
                    }
                }
            }
            ExprKind::IntegerConversion { operand, .. } => {
                self.collect_no_gc_expr_violations(operand, out, requirements);
            }
            ExprKind::Unary { operand, .. }
            | ExprKind::PrimitiveUnary { operand, .. }
            | ExprKind::SomeWrap(operand)
            | ExprKind::IsSome(operand)
            | ExprKind::PtrFromNonZeroULong(operand)
            | ExprKind::PtrToULong(operand)
            | ExprKind::PtrCast(operand) => {
                self.collect_no_gc_expr_violations(operand, out, requirements)
            }
            ExprKind::PtrLoad { pointer, offset } => {
                self.collect_no_gc_expr_violations(pointer, out, requirements);
                if let Some(offset) = offset {
                    self.collect_no_gc_expr_violations(offset, out, requirements);
                }
            }
            ExprKind::PtrOffset {
                pointer, offset, ..
            } => {
                self.collect_no_gc_expr_violations(pointer, out, requirements);
                self.collect_no_gc_expr_violations(offset, out, requirements);
            }
            ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => {
                self.collect_no_gc_expr_violations(pointer, out, requirements);
                if let Some(offset) = offset {
                    self.collect_no_gc_expr_violations(offset, out, requirements);
                }
                self.collect_no_gc_expr_violations(value, out, requirements);
            }
            // `addressOf` only materializes an already validated GC-free
            // place. It is unsafe, but does not allocate or enter the GC.
            ExprKind::AddressOf(_) => {}
            ExprKind::SizeOf(_) | ExprKind::AlignOf(_) | ExprKind::FunctionAddress(_) => {}
            ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => {
                if *trap_on_none {
                    out.push((
                        expr.span,
                        "`!!` is not allowed in `@NoGC` code because it may throw".to_string(),
                    ));
                }
                self.collect_no_gc_expr_violations(operand, out, requirements);
            }
        }
    }

    fn collect_no_gc_type_violations(
        &self,
        ty: hir::TypeId,
        span: Span,
        out: &mut Vec<(Span, String)>,
        requirements: &mut HashSet<hir::TypeParamId>,
    ) {
        match self.gc_free_requirements(ty) {
            Some(required) => requirements.extend(required),
            None => {
                out.push((
                    span,
                    format!(
                        "value of non-GC-free type {} is not allowed in `@NoGC` code",
                        self.type_name(ty)
                    ),
                ));
            }
        }
    }
}
