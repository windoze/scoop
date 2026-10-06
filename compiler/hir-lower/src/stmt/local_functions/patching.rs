use super::*;

/// Recursive calls may be lowered before a later source use discovers the
/// complete capture set. Once the local body has been analyzed, rewrite every
/// self-call and reference to the final hidden-argument list. Binding identity,
/// rather than source names, makes this stable under shadowing.
pub(super) fn patch_local_function_calls(
    lowerer: &mut Lowerer,
    statements: &mut [hir::Statement],
    target: hir::LocalFunctionId,
    captures: &[hir::Capture],
) {
    LocalFunctionCallPatcher {
        lowerer,
        target,
        captures,
    }
    .statements(statements);
}

struct LocalFunctionCallPatcher<'a> {
    lowerer: &'a mut Lowerer,
    target: hir::LocalFunctionId,
    captures: &'a [hir::Capture],
}

impl LocalFunctionCallPatcher<'_> {
    fn statements(&mut self, statements: &mut [hir::Statement]) {
        for statement in statements {
            match &mut statement.kind {
                hir::StatementKind::ContextScope { value, body } => {
                    self.expression(value);
                    self.statements(body);
                }
                hir::StatementKind::InitializationEnsure(_)
                | hir::StatementKind::GenericDelegateEnsure(_) => {}
                hir::StatementKind::Expr(expr) | hir::StatementKind::Throw(expr) => {
                    self.expression(expr)
                }
                hir::StatementKind::Return { value } => {
                    if let Some(value) = value {
                        self.expression(value);
                    }
                }
                hir::StatementKind::LocalFunction(_)
                | hir::StatementKind::Break { .. }
                | hir::StatementKind::Continue { .. } => {}
                hir::StatementKind::ValDecl { pattern, init } => {
                    self.pattern(pattern);
                    self.expression(init);
                }
                hir::StatementKind::Assign {
                    target: place,
                    value,
                } => {
                    match place {
                        hir::AssignTarget::Local(_)
                        | hir::AssignTarget::Global(_)
                        | hir::AssignTarget::GenericDelegateStorage(_)
                        | hir::AssignTarget::SingletonPublishedRoot(_) => {}
                        hir::AssignTarget::Index { array, index } => {
                            self.expression(array);
                            self.expression(index);
                        }
                        hir::AssignTarget::Field { receiver, .. } => {
                            self.expression(receiver);
                        }
                        hir::AssignTarget::InitializingClassField { .. } => {}
                    }
                    self.expression(value);
                }
                hir::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                } => {
                    self.expression(cond);
                    self.statements(then_body);
                    if let Some(else_body) = else_body {
                        self.statements(else_body);
                    }
                }
                hir::StatementKind::While {
                    target: _,
                    condition_setup,
                    cond,
                    body,
                } => {
                    self.statements(condition_setup);
                    self.expression(cond);
                    self.statements(body);
                }
                hir::StatementKind::When(when) => {
                    self.expression(&mut when.subject);
                    for arm in &mut when.arms {
                        self.pattern(&mut arm.pattern);
                        if let Some(guard) = &mut arm.guard {
                            self.statements(&mut guard.setup);
                            self.expression(&mut guard.condition);
                        }
                        self.statements(&mut arm.body);
                    }
                    if let hir::WhenFallback::Else(body) = &mut when.fallback {
                        self.statements(body);
                    }
                }
                hir::StatementKind::Try(try_) => {
                    self.statements(&mut try_.body);
                    for catch in &mut try_.catches {
                        self.statements(&mut catch.body);
                    }
                    if let Some(finally_body) = &mut try_.finally_body {
                        self.statements(finally_body);
                    }
                }
            }
        }
    }

    fn pattern(&mut self, pattern: &mut hir::Pattern) {
        match pattern {
            hir::Pattern::Literal { value, .. } => self.expression(value),
            hir::Pattern::Variant { fields, .. } | hir::Pattern::Struct { fields, .. } => {
                for (_, field) in fields {
                    self.pattern(field);
                }
            }
            hir::Pattern::Tuple(elements) => {
                for element in elements {
                    self.pattern(element);
                }
            }
            hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => {}
        }
    }

    fn expression(&mut self, expr: &mut hir::Expr) {
        let target = self.target;
        let target_captures = self.captures;
        let span = expr.span;
        let origin = expr.origin;
        match &mut expr.kind {
            hir::ExprKind::Lambda(id) => {
                let mut closure = self.lowerer.lambdas[*id].clone();
                for capture in &mut closure.captures {
                    self.expression(&mut capture.source);
                }
                self.lowerer.lambdas[*id] = closure;
            }
            hir::ExprKind::AnonymousFunction(id) => {
                let mut closure = self.lowerer.anonymous_functions[*id].clone();
                for capture in &mut closure.captures {
                    self.expression(&mut capture.source);
                }
                self.lowerer.anonymous_functions[*id] = closure;
            }
            hir::ExprKind::CallableReference(id) => {
                let mut reference = self.lowerer.callable_references[*id].clone();
                if let Some(receiver) = reference.target.receiver_mut() {
                    self.expression(receiver);
                }
                for capture in &mut reference.captures {
                    self.expression(&mut capture.source);
                }
                if let hir::CallableReferenceTarget::Local {
                    callee: hir::CallableTarget::Local(callee),
                    ..
                } = reference.target
                    && self.lowerer.callable_function_id(callee)
                        == self.lowerer.local_functions[target].source_function()
                {
                    // A self reference preserves its lexical owner arguments;
                    // captured outer bindings have the same types in this body.
                    reference.captures = target_captures
                        .iter()
                        .map(|capture| hir::Capture {
                            source: hir::Expr {
                                kind: hir::ExprKind::Capture(capture.binding),
                                ty: capture.ty,
                                span,
                                origin,
                            },
                            ..capture.clone()
                        })
                        .collect();
                }
                self.lowerer.callable_references[*id] = reference;
            }
            hir::ExprKind::Call { callee, args, .. } => {
                for argument in args.iter_mut() {
                    self.expression(argument);
                }
                if let hir::CallableTarget::Local(callee) = *callee {
                    let function = self.lowerer.callable_function_id(callee);
                    if function == self.lowerer.local_functions[target].source_function() {
                        let parameter_count = self.lowerer.signatures[&function].params.len();
                        let previous_count = args
                            .len()
                            .checked_sub(parameter_count)
                            .expect("a resolved recursive call contains every source parameter");
                        if previous_count != target_captures.len() {
                            args.drain(..previous_count);
                            let mut complete = target_captures
                                .iter()
                                .map(|capture| hir::Expr {
                                    kind: hir::ExprKind::Capture(capture.binding),
                                    ty: capture.ty,
                                    span,
                                    origin,
                                })
                                .collect::<Vec<_>>();
                            complete.append(args);
                            *args = complete;
                        }
                    }
                }
            }
            hir::ExprKind::TupleLiteral(elements)
            | hir::ExprKind::ArrayLiteral(elements)
            | hir::ExprKind::StructInit { args: elements, .. }
            | hir::ExprKind::ClassInit { args: elements, .. }
            | hir::ExprKind::VariantConstruct { args: elements, .. } => {
                for element in elements {
                    self.expression(element);
                }
            }
            hir::ExprKind::StructConstruct {
                fields: elements, ..
            } => {
                for element in elements {
                    self.expression(element);
                }
            }
            hir::ExprKind::ArrayAssembly(assembly) => {
                for part in &mut assembly.parts {
                    match part {
                        hir::ArrayAssemblyPart::Element(value)
                        | hir::ArrayAssemblyPart::CopyArray(value) => self.expression(value),
                    }
                }
            }
            hir::ExprKind::FieldAccess { receiver, .. }
            | hir::ExprKind::VariantTest {
                operand: receiver, ..
            }
            | hir::ExprKind::VariantPayloadProject {
                operand: receiver, ..
            }
            | hir::ExprKind::ForeignCallbackRegister {
                closure: receiver, ..
            }
            | hir::ExprKind::ForeignCallbackOperation {
                callback: receiver, ..
            }
            | hir::ExprKind::FunctionCoercion {
                source: receiver, ..
            }
            | hir::ExprKind::Box(receiver)
            | hir::ExprKind::Unbox(receiver)
            | hir::ExprKind::ReferenceUpcast(receiver)
            | hir::ExprKind::IsInstance {
                operand: receiver, ..
            }
            | hir::ExprKind::Cast {
                operand: receiver, ..
            }
            | hir::ExprKind::ArrayLen(receiver)
            | hir::ExprKind::ArrayClone(receiver)
            | hir::ExprKind::Unary {
                operand: receiver, ..
            }
            | hir::ExprKind::PrimitiveUnary {
                operand: receiver, ..
            }
            | hir::ExprKind::SomeWrap(receiver)
            | hir::ExprKind::IsSome(receiver)
            | hir::ExprKind::Unwrap {
                operand: receiver, ..
            }
            | hir::ExprKind::PtrFromNonZeroULong(receiver)
            | hir::ExprKind::CharCode(receiver)
            | hir::ExprKind::CharFromCodeUnchecked(receiver)
            | hir::ExprKind::PtrToULong(receiver)
            | hir::ExprKind::PtrCast(receiver) => self.expression(receiver),
            hir::ExprKind::MethodCall { receiver, args, .. }
            | hir::ExprKind::DirectSuperMethodCall { receiver, args, .. }
            | hir::ExprKind::CallableCall {
                callee: receiver,
                args,
                ..
            } => {
                self.expression(receiver);
                for arg in args {
                    self.expression(arg);
                }
            }
            hir::ExprKind::Index {
                receiver, index, ..
            } => {
                self.expression(receiver);
                self.expression(index);
            }
            hir::ExprKind::PrimitiveBinary { lhs, rhs, .. }
            | hir::ExprKind::ArrayGenerate {
                count: lhs,
                initializer: rhs,
            }
            | hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.expression(lhs);
                self.expression(rhs);
            }
            hir::ExprKind::IntegerOperation { arguments, .. } => match arguments {
                hir::HirIntegerOperationArguments::Unary(operand) => {
                    self.expression(operand);
                }
                hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                    self.expression(lhs);
                    self.expression(rhs);
                }
            },
            hir::ExprKind::IntegerConversion { operand, .. } => {
                self.expression(operand);
            }
            hir::ExprKind::ArraySet {
                receiver,
                index,
                value,
                ..
            } => {
                self.expression(receiver);
                self.expression(index);
                self.expression(value);
            }
            hir::ExprKind::PtrLoad { pointer, offset } => {
                self.expression(pointer);
                if let Some(offset) = offset {
                    self.expression(offset);
                }
            }
            hir::ExprKind::PtrOffset {
                pointer, offset, ..
            } => {
                self.expression(pointer);
                self.expression(offset);
            }
            hir::ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => {
                self.expression(pointer);
                if let Some(offset) = offset {
                    self.expression(offset);
                }
                self.expression(value);
            }
            hir::ExprKind::ContextLookup(_)
            | hir::ExprKind::StringLiteral { .. }
            | hir::ExprKind::IntegerLiteral(_)
            | hir::ExprKind::CharLiteral(_)
            | hir::ExprKind::FloatLiteral(_)
            | hir::ExprKind::BoolLiteral(_)
            | hir::ExprKind::UnitLiteral
            | hir::ExprKind::Local(_)
            | hir::ExprKind::ConstructorReceiver
            | hir::ExprKind::ConstructorParam(_)
            | hir::ExprKind::InitializingClassFieldAccess { .. }
            | hir::ExprKind::ReleaseFieldLoad(_)
            | hir::ExprKind::InitializingStructFieldAccess { .. }
            | hir::ExprKind::GlobalRead(_)
            | hir::ExprKind::GenericDelegateStorageRead(_)
            | hir::ExprKind::SingletonValue(_)
            | hir::ExprKind::Capture(_)
            | hir::ExprKind::NoneLiteral
            | hir::ExprKind::AddressOf(_)
            | hir::ExprKind::SizeOf(_)
            | hir::ExprKind::AlignOf(_)
            | hir::ExprKind::FunctionAddress(_) => {}
        }
    }
}
