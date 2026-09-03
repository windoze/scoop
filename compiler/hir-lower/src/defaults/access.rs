use scoop_hir as hir;

use crate::Lowerer;

struct ReferenceCollector {
    references: hir::ExportDefaultReferences,
    witness: hir::ExportDefaultAccessWitness,
    fallback_origin: hir::DefinitionOrigin,
}

impl Lowerer {
    pub(super) fn collect_export_default_references(
        &self,
        owner: hir::ExportParameterOwner,
        template: &hir::ExportDefaultExpr,
    ) -> hir::ExportDefaultReferences {
        let mut collector = ReferenceCollector {
            references: hir::ExportDefaultReferences::default(),
            witness: hir::ExportDefaultAccessWitness {
                owner,
                coverage: hir::ExportDefaultAccessCoverage::ConeWide,
            },
            fallback_origin: template.origin,
        };
        for local in template.locals.values() {
            collector.type_reference(local.ty, template.origin);
        }
        for statement in &template.statements {
            collector.statement(statement);
        }
        collector.expression(&template.value);
        collector.references
    }
}

impl ReferenceCollector {
    fn statement(&mut self, statement: &hir::Statement) {
        match &statement.kind {
            hir::StatementKind::Expr(value) | hir::StatementKind::Throw(value) => {
                self.expression(value);
            }
            hir::StatementKind::LocalFunction(function) => {
                self.callable(
                    hir::ExportDefaultCallableTarget::LocalFunction(*function),
                    self.at(statement.span),
                );
            }
            hir::StatementKind::Return { value } => {
                if let Some(value) = value {
                    self.expression(value);
                }
            }
            hir::StatementKind::ValDecl { pattern, init } => {
                self.pattern(pattern);
                self.expression(init);
            }
            hir::StatementKind::Assign { target, value } => {
                self.assign_target(target);
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
                condition_setup,
                cond,
                body,
            } => {
                self.statements(condition_setup);
                self.expression(cond);
                self.statements(body);
            }
            hir::StatementKind::When(value) => {
                self.expression(&value.subject);
                for arm in &value.arms {
                    self.pattern(&arm.pattern);
                    if let Some(guard) = &arm.guard {
                        self.statements(&guard.setup);
                        self.expression(&guard.condition);
                    }
                    self.statements(&arm.body);
                }
                if let Some(else_body) = &value.else_body {
                    self.statements(else_body);
                }
            }
            hir::StatementKind::Try(value) => {
                self.statements(&value.body);
                for catch in &value.catches {
                    self.type_reference(catch.ty, self.at(statement.span));
                    self.statements(&catch.body);
                }
                if let Some(finally_body) = &value.finally_body {
                    self.statements(finally_body);
                }
            }
        }
    }

    fn statements(&mut self, statements: &[hir::Statement]) {
        for statement in statements {
            self.statement(statement);
        }
    }

    fn assign_target(&mut self, target: &hir::AssignTarget) {
        match target {
            hir::AssignTarget::Local(_) => {}
            hir::AssignTarget::Global(global) => {
                self.global(*global, self.fallback_origin);
            }
            hir::AssignTarget::Index { array, index } => {
                self.expression(array);
                self.expression(index);
            }
            hir::AssignTarget::Field { receiver, field } => {
                self.expression(receiver);
                self.field(*field, receiver.origin.definition());
            }
        }
    }

    fn pattern(&mut self, pattern: &hir::Pattern) {
        match pattern {
            hir::Pattern::Binding { .. } | hir::Pattern::Wildcard => {}
            hir::Pattern::Literal {
                value,
                equals,
                subject_ty,
            } => {
                self.expression(value);
                self.callable(
                    hir::ExportDefaultCallableTarget::Callable(*equals),
                    value.origin.definition(),
                );
                self.type_reference(*subject_ty, value.origin.definition());
            }
            hir::Pattern::Variant {
                application,
                variant,
                fields,
            } => {
                self.constructor(
                    hir::ExportDefaultConstructorTarget::Variant {
                        application: *application,
                        variant: *variant,
                    },
                    self.fallback_origin,
                );
                for (_, field) in fields {
                    self.pattern(field);
                }
            }
            hir::Pattern::Tuple(elements) => {
                for element in elements {
                    self.pattern(element);
                }
            }
            hir::Pattern::Struct {
                application,
                fields,
            } => {
                self.constructor(
                    hir::ExportDefaultConstructorTarget::Struct(*application),
                    self.fallback_origin,
                );
                for (_, field) in fields {
                    self.pattern(field);
                }
            }
        }
    }

    fn expression(&mut self, expression: &hir::Expr) {
        let origin = expression.origin.definition();
        self.type_reference(expression.ty, origin);
        match &expression.kind {
            hir::ExprKind::StringLiteral(_)
            | hir::ExprKind::IntLiteral(_)
            | hir::ExprKind::BoolLiteral(_)
            | hir::ExprKind::UnitLiteral
            | hir::ExprKind::ConstructorParam(_)
            | hir::ExprKind::Local(_)
            | hir::ExprKind::Capture(_)
            | hir::ExprKind::FunPtrNull
            | hir::ExprKind::NoneLiteral => {}
            hir::ExprKind::TupleLiteral(values) | hir::ExprKind::ArrayLiteral(values) => {
                self.expressions(values);
            }
            hir::ExprKind::StructInit { application, args } => {
                self.constructor(
                    hir::ExportDefaultConstructorTarget::Struct(*application),
                    origin,
                );
                self.expressions(args);
            }
            hir::ExprKind::ClassInit { application, args } => {
                self.constructor(
                    hir::ExportDefaultConstructorTarget::Class(*application),
                    origin,
                );
                self.expressions(args);
            }
            hir::ExprKind::VariantConstruct {
                application,
                variant,
                args,
            } => {
                self.constructor(
                    hir::ExportDefaultConstructorTarget::Variant {
                        application: *application,
                        variant: *variant,
                    },
                    origin,
                );
                self.expressions(args);
            }
            hir::ExprKind::GlobalRead(global) => self.global(*global, origin),
            hir::ExprKind::Lambda(lambda) => {
                self.callable(hir::ExportDefaultCallableTarget::Lambda(*lambda), origin);
            }
            hir::ExprKind::AnonymousFunction(function) => self.callable(
                hir::ExportDefaultCallableTarget::AnonymousFunction(*function),
                origin,
            ),
            hir::ExprKind::CallableReference(reference) => self.callable(
                hir::ExportDefaultCallableTarget::CallableReference(*reference),
                origin,
            ),
            hir::ExprKind::FunctionCoercion { source, .. }
            | hir::ExprKind::PtrFromUInt(source)
            | hir::ExprKind::PtrToUInt(source)
            | hir::ExprKind::PtrCast(source)
            | hir::ExprKind::Box(source)
            | hir::ExprKind::Unbox(source)
            | hir::ExprKind::ArrayLen(source)
            | hir::ExprKind::ArrayClone(source)
            | hir::ExprKind::SomeWrap(source)
            | hir::ExprKind::IsSome(source) => self.expression(source),
            hir::ExprKind::PtrLoad { pointer, offset } => {
                self.expression(pointer);
                if let Some(offset) = offset {
                    self.expression(offset);
                }
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
            hir::ExprKind::PtrOffset {
                pointer, offset, ..
            }
            | hir::ExprKind::Index {
                receiver: pointer,
                index: offset,
                ..
            }
            | hir::ExprKind::PrimitiveBinary {
                lhs: pointer,
                rhs: offset,
                ..
            }
            | hir::ExprKind::Binary {
                lhs: pointer,
                rhs: offset,
                ..
            } => {
                self.expression(pointer);
                self.expression(offset);
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
            hir::ExprKind::AddressOf(place) => {
                if let hir::Place::Global(global) = place {
                    self.global(*global, origin);
                }
            }
            hir::ExprKind::SizeOf(ty) | hir::ExprKind::AlignOf(ty) => {
                self.type_reference(*ty, origin);
            }
            hir::ExprKind::FunctionAddress(function) => self.callable(
                hir::ExportDefaultCallableTarget::FunctionAddress(*function),
                origin,
            ),
            hir::ExprKind::ForeignCallbackRegister { closure, .. } => self.expression(closure),
            hir::ExprKind::ForeignCallbackOperation { callback, .. } => self.expression(callback),
            hir::ExprKind::FieldAccess { receiver, field } => {
                self.expression(receiver);
                self.field(*field, origin);
            }
            hir::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => {
                self.expression(receiver);
                self.method_callee(*callee, origin);
                self.expressions(args);
            }
            hir::ExprKind::IsInstance { operand, check_ty } => {
                self.expression(operand);
                self.type_reference(*check_ty, origin);
            }
            hir::ExprKind::Cast { operand, .. }
            | hir::ExprKind::Unary { operand, .. }
            | hir::ExprKind::PrimitiveUnary { operand, .. }
            | hir::ExprKind::Unwrap { operand, .. } => self.expression(operand),
            hir::ExprKind::ArrayAssembly(assembly) => {
                self.type_reference(assembly.element_type, origin);
                for part in &assembly.parts {
                    match part {
                        hir::ArrayAssemblyPart::Element(value)
                        | hir::ArrayAssemblyPart::CopyArray(value) => self.expression(value),
                    }
                }
            }
            hir::ExprKind::Call { callee, args } => {
                self.callable(hir::ExportDefaultCallableTarget::Callable(*callee), origin);
                self.expressions(args);
            }
            hir::ExprKind::LocalFunctionCall {
                local_function,
                callee,
                captures,
                args,
            } => {
                self.callable(
                    hir::ExportDefaultCallableTarget::LocalFunction(*local_function),
                    origin,
                );
                self.callable(hir::ExportDefaultCallableTarget::Callable(*callee), origin);
                self.expressions(captures);
                self.expressions(args);
            }
            hir::ExprKind::CallableCall { callee, args, .. } => {
                self.expression(callee);
                self.expressions(args);
            }
        }
    }

    fn expressions(&mut self, expressions: &[hir::Expr]) {
        for expression in expressions {
            self.expression(expression);
        }
    }

    fn method_callee(&mut self, callee: hir::MethodCallee, origin: hir::DefinitionOrigin) {
        let target = match callee {
            hir::MethodCallee::Callable(callable) => {
                hir::ExportDefaultCallableTarget::Callable(callable)
            }
            hir::MethodCallee::Bound(bound) => hir::ExportDefaultCallableTarget::Bound(bound),
            hir::MethodCallee::DerivedEquality(application) => {
                hir::ExportDefaultCallableTarget::DerivedEquality(application)
            }
        };
        self.references
            .callables
            .push(hir::ExportDefaultCallableRef {
                target,
                witness: self.witness,
                origin,
            });
    }

    fn callable(
        &mut self,
        target: hir::ExportDefaultCallableTarget,
        origin: hir::DefinitionOrigin,
    ) {
        self.references
            .callables
            .push(hir::ExportDefaultCallableRef {
                target,
                witness: self.witness,
                origin,
            });
    }

    fn constructor(
        &mut self,
        target: hir::ExportDefaultConstructorTarget,
        origin: hir::DefinitionOrigin,
    ) {
        self.references
            .constructors
            .push(hir::ExportDefaultConstructorRef {
                target,
                witness: self.witness,
                origin,
            });
    }

    fn type_reference(&mut self, target: hir::TypeId, origin: hir::DefinitionOrigin) {
        self.references.types.push(hir::ExportDefaultTypeRef {
            target,
            witness: self.witness,
            origin,
        });
    }

    fn global(&mut self, target: hir::GlobalId, origin: hir::DefinitionOrigin) {
        self.references.globals.push(hir::ExportDefaultGlobalRef {
            target,
            witness: self.witness,
            origin,
        });
    }

    fn field(&mut self, target: hir::FieldRef, origin: hir::DefinitionOrigin) {
        self.references.fields.push(hir::ExportDefaultFieldRef {
            target,
            witness: self.witness,
            origin,
        });
    }

    fn at(&self, span: scoop_ast::Span) -> hir::DefinitionOrigin {
        hir::DefinitionOrigin {
            provider: self.fallback_origin.provider,
            file: self.fallback_origin.file,
            span,
            context: self.fallback_origin.context,
        }
    }
}
