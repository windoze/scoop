use scoop_hir as hir;
use std::collections::HashMap;

use crate::defaults::DefaultExprTemplateRef;
use crate::{Lowerer, Type};

mod entities;

struct InstantiationContext {
    bindings: Vec<(hir::TypeParamId, hir::TypeId)>,
    locals: Vec<hir::Expr>,
    captures: HashMap<hir::BindingId, hir::Expr>,
    evaluation: InstantiationEvaluation,
}

#[derive(Clone, Copy)]
enum InstantiationEvaluation {
    Template,
    Concrete(hir::EvaluationOrigin),
}

impl Lowerer {
    pub(crate) fn instantiate_default(
        &mut self,
        template: DefaultExprTemplateRef,
        bindings: &[(hir::TypeParamId, hir::TypeId)],
        receiver: Option<&hir::Expr>,
        value_parameters: &[hir::Expr],
        call_span: scoop_ast::Span,
        sink: &mut Vec<hir::Statement>,
    ) -> hir::Expr {
        let (template, captures, template_bindings) = match template {
            DefaultExprTemplateRef::Local(template) => {
                let template = self.local_default_exprs[template].clone();
                (template.body, template.captures, bindings.to_vec())
            }
            DefaultExprTemplateRef::Export(source) => {
                let source = self.export_default_sources[source].clone();
                let template = self.export_default_exprs[source.expression].clone();
                assert_eq!(template.type_parameters.len(), source.type_arguments.len());
                let arguments = source
                    .type_arguments
                    .into_iter()
                    .map(|argument| self.instantiate_method_ty(argument, bindings))
                    .collect::<Vec<_>>();
                let template_bindings = template
                    .type_parameters
                    .iter()
                    .copied()
                    .zip(arguments)
                    .collect();
                (template, Vec::new(), template_bindings)
            }
        };
        let mut mapped = vec![None; template.locals.len()];
        if let Some(source) = template.receiver {
            mapped[arena_index(source.local)] = Some(
                receiver
                    .expect("a receiver default is instantiated with its receiver")
                    .clone(),
            );
        }
        for source in &template.value_parameters {
            mapped[arena_index(source.local)] =
                Some(value_parameters[source.position as usize].clone());
        }
        for (source_id, source) in template.locals.iter() {
            if mapped[arena_index(source_id)].is_some() {
                continue;
            }
            let ty = self.instantiate_method_ty(source.ty, &template_bindings);
            let local = self.alloc_local(source.name.clone(), ty, source.mutable);
            mapped[arena_index(source_id)] = Some(hir::Expr {
                kind: hir::ExprKind::Local(local),
                ty,
                span: template.origin.span,
                origin: hir::ExpressionOrigin::Definition(template.origin),
            });
        }
        let mut context = InstantiationContext {
            bindings: template_bindings,
            locals: mapped
                .into_iter()
                .map(|local| local.expect("every default-template local is mapped"))
                .collect(),
            captures: captures
                .into_iter()
                .map(|capture| {
                    let value = self
                        .lower_capture_binding(
                            capture.binding,
                            &capture.name,
                            capture.first_use_span,
                        )
                        .unwrap_or(capture.source);
                    (capture.binding, value)
                })
                .collect(),
            evaluation: if self.lowering_default_template {
                InstantiationEvaluation::Template
            } else {
                InstantiationEvaluation::Concrete(self.definition_origin(call_span).into())
            },
        };
        for statement in &template.statements {
            sink.push(self.instantiate_default_statement(statement, &mut context));
        }
        self.instantiate_default_expr(&template.value, &mut context)
    }

    fn instantiate_default_statement(
        &mut self,
        source: &hir::Statement,
        context: &mut InstantiationContext,
    ) -> hir::Statement {
        let kind = match &source.kind {
            hir::StatementKind::InitializationEnsure(unit) => {
                hir::StatementKind::InitializationEnsure(*unit)
            }
            hir::StatementKind::Expr(value) => {
                hir::StatementKind::Expr(self.instantiate_default_expr(value, context))
            }
            hir::StatementKind::LocalFunction(function) => {
                hir::StatementKind::LocalFunction(*function)
            }
            hir::StatementKind::Return { value } => hir::StatementKind::Return {
                value: value
                    .as_ref()
                    .map(|value| self.instantiate_default_expr(value, context)),
            },
            hir::StatementKind::ValDecl { pattern, init } => hir::StatementKind::ValDecl {
                pattern: self.instantiate_default_pattern(pattern, context),
                init: self.instantiate_default_expr(init, context),
            },
            hir::StatementKind::Assign { target, value } => hir::StatementKind::Assign {
                target: self.instantiate_default_assign_target(target, context),
                value: self.instantiate_default_expr(value, context),
            },
            hir::StatementKind::If {
                cond,
                then_body,
                else_body,
            } => hir::StatementKind::If {
                cond: self.instantiate_default_expr(cond, context),
                then_body: then_body
                    .iter()
                    .map(|statement| self.instantiate_default_statement(statement, context))
                    .collect(),
                else_body: else_body.as_ref().map(|body| {
                    body.iter()
                        .map(|statement| self.instantiate_default_statement(statement, context))
                        .collect()
                }),
            },
            hir::StatementKind::While {
                condition_setup,
                cond,
                body,
            } => hir::StatementKind::While {
                condition_setup: condition_setup
                    .iter()
                    .map(|statement| self.instantiate_default_statement(statement, context))
                    .collect(),
                cond: self.instantiate_default_expr(cond, context),
                body: body
                    .iter()
                    .map(|statement| self.instantiate_default_statement(statement, context))
                    .collect(),
            },
            hir::StatementKind::When(when) => hir::StatementKind::When(hir::When {
                subject: self.instantiate_default_expr(&when.subject, context),
                arms: when
                    .arms
                    .iter()
                    .map(|arm| hir::WhenArm {
                        pattern: self.instantiate_default_pattern(&arm.pattern, context),
                        guard: arm.guard.as_ref().map(|guard| hir::WhenGuard {
                            setup: guard
                                .setup
                                .iter()
                                .map(|statement| {
                                    self.instantiate_default_statement(statement, context)
                                })
                                .collect(),
                            condition: self.instantiate_default_expr(&guard.condition, context),
                        }),
                        body: arm
                            .body
                            .iter()
                            .map(|statement| self.instantiate_default_statement(statement, context))
                            .collect(),
                        span: arm.span,
                    })
                    .collect(),
                fallback: match &when.fallback {
                    hir::WhenFallback::Else(body) => hir::WhenFallback::Else(
                        body.iter()
                            .map(|statement| self.instantiate_default_statement(statement, context))
                            .collect(),
                    ),
                    hir::WhenFallback::Impossible(proof) => {
                        let proof = match proof {
                            hir::ExhaustivenessProof::IrrefutableArm { subject_ty } => {
                                hir::ExhaustivenessProof::IrrefutableArm {
                                    subject_ty: self
                                        .instantiate_method_ty(*subject_ty, &context.bindings),
                                }
                            }
                            hir::ExhaustivenessProof::PatternMatrix { subject_ty } => {
                                hir::ExhaustivenessProof::PatternMatrix {
                                    subject_ty: self
                                        .instantiate_method_ty(*subject_ty, &context.bindings),
                                }
                            }
                            hir::ExhaustivenessProof::EnumPatternMatrix {
                                subject_ty,
                                application,
                            } => hir::ExhaustivenessProof::EnumPatternMatrix {
                                subject_ty: self
                                    .instantiate_method_ty(*subject_ty, &context.bindings),
                                application: self
                                    .instantiate_default_enum_application(*application, context),
                            },
                        };
                        hir::WhenFallback::Impossible(proof)
                    }
                },
            }),
            hir::StatementKind::Try(value) => hir::StatementKind::Try(hir::Try {
                body: value
                    .body
                    .iter()
                    .map(|statement| self.instantiate_default_statement(statement, context))
                    .collect(),
                catches: value
                    .catches
                    .iter()
                    .map(|catch| hir::CatchClause {
                        local: mapped_local(context, catch.local),
                        ty: self.instantiate_method_ty(catch.ty, &context.bindings),
                        body: catch
                            .body
                            .iter()
                            .map(|statement| self.instantiate_default_statement(statement, context))
                            .collect(),
                        span: catch.span,
                    })
                    .collect(),
                finally_body: value.finally_body.as_ref().map(|body| {
                    body.iter()
                        .map(|statement| self.instantiate_default_statement(statement, context))
                        .collect()
                }),
            }),
            hir::StatementKind::Throw(value) => {
                hir::StatementKind::Throw(self.instantiate_default_expr(value, context))
            }
        };
        hir::Statement {
            kind,
            span: source.span,
        }
    }

    fn instantiate_default_assign_target(
        &mut self,
        source: &hir::AssignTarget,
        context: &mut InstantiationContext,
    ) -> hir::AssignTarget {
        match source {
            hir::AssignTarget::Local(local) => {
                hir::AssignTarget::Local(mapped_local(context, *local))
            }
            hir::AssignTarget::Global(global) => hir::AssignTarget::Global(*global),
            hir::AssignTarget::SingletonPublishedRoot(root) => {
                hir::AssignTarget::SingletonPublishedRoot(*root)
            }
            hir::AssignTarget::Index { array, index } => hir::AssignTarget::Index {
                array: Box::new(self.instantiate_default_expr(array, context)),
                index: Box::new(self.instantiate_default_expr(index, context)),
            },
            hir::AssignTarget::Field { receiver, field } => hir::AssignTarget::Field {
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                field: self.instantiate_default_field(*field, context),
            },
            hir::AssignTarget::InitializingClassField {
                application,
                field,
                origin,
            } => hir::AssignTarget::InitializingClassField {
                application: self.instantiate_default_class_application(*application, context),
                field: *field,
                origin: instantiate_origin(*origin, context.evaluation),
            },
        }
    }

    fn instantiate_default_pattern(
        &mut self,
        source: &hir::Pattern,
        context: &mut InstantiationContext,
    ) -> hir::Pattern {
        match source {
            hir::Pattern::Binding { local } => hir::Pattern::Binding {
                local: mapped_local(context, *local),
            },
            hir::Pattern::Wildcard => hir::Pattern::Wildcard,
            hir::Pattern::Literal {
                value,
                equality,
                subject_ty,
            } => hir::Pattern::Literal {
                value: self.instantiate_default_expr(value, context),
                equality: match *equality {
                    hir::LiteralPatternEquality::Integer { kind, target } => {
                        hir::LiteralPatternEquality::Integer { kind, target }
                    }
                    hir::LiteralPatternEquality::Ordinary { equals } => {
                        hir::LiteralPatternEquality::Ordinary {
                            equals: self.instantiate_default_callable(equals, context),
                        }
                    }
                },
                subject_ty: self.instantiate_method_ty(*subject_ty, &context.bindings),
            },
            hir::Pattern::Variant {
                application,
                variant,
                fields,
            } => hir::Pattern::Variant {
                application: self.instantiate_default_enum_application(*application, context),
                variant: *variant,
                fields: fields
                    .iter()
                    .map(|(index, pattern)| {
                        (*index, self.instantiate_default_pattern(pattern, context))
                    })
                    .collect(),
            },
            hir::Pattern::Tuple(elements) => hir::Pattern::Tuple(
                elements
                    .iter()
                    .map(|element| self.instantiate_default_pattern(element, context))
                    .collect(),
            ),
            hir::Pattern::Struct {
                application,
                fields,
            } => hir::Pattern::Struct {
                application: self.instantiate_default_struct_application(*application, context),
                fields: fields
                    .iter()
                    .map(|(index, pattern)| {
                        (*index, self.instantiate_default_pattern(pattern, context))
                    })
                    .collect(),
            },
        }
    }

    fn instantiate_default_expr(
        &mut self,
        source: &hir::Expr,
        context: &mut InstantiationContext,
    ) -> hir::Expr {
        if let hir::ExprKind::Local(local) = source.kind {
            let mut value = context.locals[arena_index(local)].clone();
            value.span = source.span;
            value.origin = instantiate_origin(source.origin, context.evaluation);
            return value;
        }
        if let hir::ExprKind::Capture(binding) = source.kind
            && let Some(value) = context.captures.get(&binding)
        {
            let mut value = value.clone();
            value.span = source.span;
            value.origin = instantiate_origin(source.origin, context.evaluation);
            return value;
        }
        let kind = match &source.kind {
            hir::ExprKind::StringLiteral(value) => hir::ExprKind::StringLiteral(value.clone()),
            hir::ExprKind::IntegerLiteral(value) => hir::ExprKind::IntegerLiteral(*value),
            hir::ExprKind::BoolLiteral(value) => hir::ExprKind::BoolLiteral(*value),
            hir::ExprKind::UnitLiteral => hir::ExprKind::UnitLiteral,
            hir::ExprKind::TupleLiteral(elements) => hir::ExprKind::TupleLiteral(
                elements
                    .iter()
                    .map(|element| self.instantiate_default_expr(element, context))
                    .collect(),
            ),
            hir::ExprKind::StructInit { constructor, args } => hir::ExprKind::StructInit {
                constructor: self.instantiate_default_struct_constructor(*constructor, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::ClassInit { constructor, args } => hir::ExprKind::ClassInit {
                constructor: self.instantiate_default_class_constructor(*constructor, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::ConstructorParam(parameter) => {
                hir::ExprKind::ConstructorParam(*parameter)
            }
            hir::ExprKind::VariantConstruct {
                application,
                variant,
                args,
            } => hir::ExprKind::VariantConstruct {
                application: self.instantiate_default_enum_application(*application, context),
                variant: *variant,
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::Local(_) => unreachable!("local reads return before kind cloning"),
            hir::ExprKind::GlobalRead(global) => hir::ExprKind::GlobalRead(*global),
            hir::ExprKind::SingletonValue(value) => hir::ExprKind::SingletonValue(*value),
            hir::ExprKind::Capture(binding) => hir::ExprKind::Capture(*binding),
            hir::ExprKind::Lambda(lambda) => {
                hir::ExprKind::Lambda(self.instantiate_default_lambda(*lambda, context))
            }
            hir::ExprKind::AnonymousFunction(function) => hir::ExprKind::AnonymousFunction(
                self.instantiate_default_anonymous(*function, context),
            ),
            hir::ExprKind::CallableReference(reference) => hir::ExprKind::CallableReference(
                self.instantiate_default_reference(*reference, context),
            ),
            hir::ExprKind::FunctionCoercion {
                source,
                coercion,
                target_type,
            } => hir::ExprKind::FunctionCoercion {
                source: Box::new(self.instantiate_default_expr(source, context)),
                coercion: self.instantiate_default_coercion(*coercion, context),
                target_type: self.instantiate_default_function_type(*target_type, context),
            },
            hir::ExprKind::PtrFromNonZeroULong(value) => hir::ExprKind::PtrFromNonZeroULong(
                Box::new(self.instantiate_default_expr(value, context)),
            ),
            hir::ExprKind::PtrToULong(value) => {
                hir::ExprKind::PtrToULong(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::PtrCast(value) => {
                hir::ExprKind::PtrCast(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::PtrLoad { pointer, offset } => hir::ExprKind::PtrLoad {
                pointer: Box::new(self.instantiate_default_expr(pointer, context)),
                offset: offset
                    .as_ref()
                    .map(|value| Box::new(self.instantiate_default_expr(value, context))),
            },
            hir::ExprKind::PtrStore {
                pointer,
                offset,
                value,
            } => hir::ExprKind::PtrStore {
                pointer: Box::new(self.instantiate_default_expr(pointer, context)),
                offset: offset
                    .as_ref()
                    .map(|value| Box::new(self.instantiate_default_expr(value, context))),
                value: Box::new(self.instantiate_default_expr(value, context)),
            },
            hir::ExprKind::PtrOffset {
                pointer,
                offset,
                subtract,
            } => hir::ExprKind::PtrOffset {
                pointer: Box::new(self.instantiate_default_expr(pointer, context)),
                offset: Box::new(self.instantiate_default_expr(offset, context)),
                subtract: *subtract,
            },
            hir::ExprKind::AddressOf(place) => hir::ExprKind::AddressOf(match place {
                hir::Place::Local(local) => hir::Place::Local(mapped_local(context, *local)),
                hir::Place::Global(global) => hir::Place::Global(*global),
            }),
            hir::ExprKind::SizeOf(ty) => {
                hir::ExprKind::SizeOf(self.instantiate_method_ty(*ty, &context.bindings))
            }
            hir::ExprKind::AlignOf(ty) => {
                hir::ExprKind::AlignOf(self.instantiate_method_ty(*ty, &context.bindings))
            }
            hir::ExprKind::FunctionAddress(function) => hir::ExprKind::FunctionAddress(*function),
            hir::ExprKind::ForeignCallbackRegister {
                registration,
                closure,
            } => hir::ExprKind::ForeignCallbackRegister {
                registration: self.instantiate_default_foreign_callback(*registration, context),
                closure: Box::new(self.instantiate_default_expr(closure, context)),
            },
            hir::ExprKind::ForeignCallbackOperation {
                operation,
                callback,
            } => hir::ExprKind::ForeignCallbackOperation {
                operation: *operation,
                callback: Box::new(self.instantiate_default_expr(callback, context)),
            },
            hir::ExprKind::FieldAccess { receiver, field } => hir::ExprKind::FieldAccess {
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                field: self.instantiate_default_field(*field, context),
            },
            hir::ExprKind::InitializingClassFieldAccess { application, field } => {
                hir::ExprKind::InitializingClassFieldAccess {
                    application: self.instantiate_default_class_application(*application, context),
                    field: *field,
                }
            }
            hir::ExprKind::InitializingStructFieldAccess { application, index } => {
                hir::ExprKind::InitializingStructFieldAccess {
                    application: self.instantiate_default_struct_application(*application, context),
                    index: *index,
                }
            }
            hir::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => hir::ExprKind::MethodCall {
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                callee: self.instantiate_default_method_callee(*callee, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::DirectSuperMethodCall {
                receiver,
                callee,
                args,
            } => hir::ExprKind::DirectSuperMethodCall {
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                callee: self.instantiate_default_method_callee(*callee, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::Box(value) => {
                hir::ExprKind::Box(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::Unbox(value) => {
                hir::ExprKind::Unbox(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::IsInstance { operand, check_ty } => hir::ExprKind::IsInstance {
                operand: Box::new(self.instantiate_default_expr(operand, context)),
                check_ty: self.instantiate_method_ty(*check_ty, &context.bindings),
            },
            hir::ExprKind::Cast { operand, optional } => hir::ExprKind::Cast {
                operand: Box::new(self.instantiate_default_expr(operand, context)),
                optional: *optional,
            },
            hir::ExprKind::ArrayLiteral(elements) => {
                hir::ExprKind::ArrayLiteral(self.instantiate_default_exprs(elements, context))
            }
            hir::ExprKind::ArrayAssembly(assembly) => {
                hir::ExprKind::ArrayAssembly(hir::ArrayAssembly {
                    element_type: self
                        .instantiate_method_ty(assembly.element_type, &context.bindings),
                    parts: assembly
                        .parts
                        .iter()
                        .map(|part| match part {
                            hir::ArrayAssemblyPart::Element(value) => {
                                hir::ArrayAssemblyPart::Element(
                                    self.instantiate_default_expr(value, context),
                                )
                            }
                            hir::ArrayAssemblyPart::CopyArray(value) => {
                                hir::ArrayAssemblyPart::CopyArray(
                                    self.instantiate_default_expr(value, context),
                                )
                            }
                        })
                        .collect(),
                    result_type: self
                        .instantiate_default_class_application(assembly.result_type, context),
                })
            }
            hir::ExprKind::Index {
                access,
                receiver,
                index,
            } => hir::ExprKind::Index {
                access: *access,
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                index: Box::new(self.instantiate_default_expr(index, context)),
            },
            hir::ExprKind::ArraySet {
                access,
                receiver,
                index,
                value,
            } => hir::ExprKind::ArraySet {
                access: *access,
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                index: Box::new(self.instantiate_default_expr(index, context)),
                value: Box::new(self.instantiate_default_expr(value, context)),
            },
            hir::ExprKind::ArrayLen(value) => {
                hir::ExprKind::ArrayLen(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::ArrayClone(value) => {
                hir::ExprKind::ArrayClone(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::Call { callee, args } => hir::ExprKind::Call {
                callee: self.instantiate_default_callable(*callee, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::LocalFunctionCall {
                local_function,
                callee,
                captures,
                args,
            } => hir::ExprKind::LocalFunctionCall {
                local_function: *local_function,
                callee: self.instantiate_default_callable(*callee, context),
                captures: self.instantiate_default_exprs(captures, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::CallableCall {
                callee,
                function_type,
                args,
            } => hir::ExprKind::CallableCall {
                callee: Box::new(self.instantiate_default_expr(callee, context)),
                function_type: self.instantiate_default_function_type(*function_type, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::PrimitiveBinary { kind, lhs, rhs } => hir::ExprKind::PrimitiveBinary {
                kind: *kind,
                lhs: Box::new(self.instantiate_default_expr(lhs, context)),
                rhs: Box::new(self.instantiate_default_expr(rhs, context)),
            },
            hir::ExprKind::PrimitiveUnary { kind, operand } => hir::ExprKind::PrimitiveUnary {
                kind: *kind,
                operand: Box::new(self.instantiate_default_expr(operand, context)),
            },
            hir::ExprKind::IntegerOperation {
                operation,
                arguments,
            } => hir::ExprKind::IntegerOperation {
                operation: *operation,
                arguments: match arguments {
                    hir::HirIntegerOperationArguments::Unary(operand) => {
                        hir::HirIntegerOperationArguments::Unary(Box::new(
                            self.instantiate_default_expr(operand, context),
                        ))
                    }
                    hir::HirIntegerOperationArguments::Binary { lhs, rhs } => {
                        hir::HirIntegerOperationArguments::Binary {
                            lhs: Box::new(self.instantiate_default_expr(lhs, context)),
                            rhs: Box::new(self.instantiate_default_expr(rhs, context)),
                        }
                    }
                },
            },
            hir::ExprKind::IntegerConversion {
                conversion,
                operand,
            } => hir::ExprKind::IntegerConversion {
                conversion: *conversion,
                operand: Box::new(self.instantiate_default_expr(operand, context)),
            },
            hir::ExprKind::Binary { op, lhs, rhs } => hir::ExprKind::Binary {
                op: *op,
                lhs: Box::new(self.instantiate_default_expr(lhs, context)),
                rhs: Box::new(self.instantiate_default_expr(rhs, context)),
            },
            hir::ExprKind::Unary { op, operand } => hir::ExprKind::Unary {
                op: *op,
                operand: Box::new(self.instantiate_default_expr(operand, context)),
            },
            hir::ExprKind::SomeWrap(value) => {
                hir::ExprKind::SomeWrap(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::NoneLiteral => hir::ExprKind::NoneLiteral,
            hir::ExprKind::IsSome(value) => {
                hir::ExprKind::IsSome(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::Unwrap {
                operand,
                trap_on_none,
            } => hir::ExprKind::Unwrap {
                operand: Box::new(self.instantiate_default_expr(operand, context)),
                trap_on_none: *trap_on_none,
            },
        };
        hir::Expr {
            kind,
            ty: self.instantiate_method_ty(source.ty, &context.bindings),
            span: source.span,
            origin: instantiate_origin(source.origin, context.evaluation),
        }
    }

    fn instantiate_default_exprs(
        &mut self,
        source: &[hir::Expr],
        context: &mut InstantiationContext,
    ) -> Vec<hir::Expr> {
        source
            .iter()
            .map(|value| self.instantiate_default_expr(value, context))
            .collect()
    }
}

fn mapped_local(context: &InstantiationContext, source: hir::LocalId) -> hir::LocalId {
    let hir::ExprKind::Local(local) = context.locals[arena_index(source)].kind else {
        unreachable!("default template places only refer to mapped local temporaries")
    };
    local
}

fn instantiate_origin(
    source: hir::ExpressionOrigin,
    evaluation: InstantiationEvaluation,
) -> hir::ExpressionOrigin {
    match evaluation {
        InstantiationEvaluation::Template => hir::ExpressionOrigin::Definition(source.definition()),
        InstantiationEvaluation::Concrete(evaluation) => source.instantiate(evaluation),
    }
}

fn arena_index<T>(id: la_arena::Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
