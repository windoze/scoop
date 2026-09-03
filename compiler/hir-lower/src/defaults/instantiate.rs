use scoop_hir as hir;
use std::collections::HashMap;

use crate::defaults::DefaultExprTemplateRef;
use crate::{Lowerer, Type};

struct InstantiationContext {
    bindings: Vec<(hir::TypeParamId, hir::TypeId)>,
    locals: Vec<hir::Expr>,
    captures: HashMap<hir::BindingId, hir::Expr>,
    evaluation: Option<hir::EvaluationOrigin>,
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
            evaluation: (!self.lowering_default_template)
                .then(|| self.definition_origin(call_span).into()),
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
                        guard: arm
                            .guard
                            .as_ref()
                            .map(|guard| self.instantiate_default_expr(guard, context)),
                        body: arm
                            .body
                            .iter()
                            .map(|statement| self.instantiate_default_statement(statement, context))
                            .collect(),
                        span: arm.span,
                    })
                    .collect(),
                else_body: when.else_body.as_ref().map(|body| {
                    body.iter()
                        .map(|statement| self.instantiate_default_statement(statement, context))
                        .collect()
                }),
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
            hir::AssignTarget::Index { array, index } => hir::AssignTarget::Index {
                array: Box::new(self.instantiate_default_expr(array, context)),
                index: Box::new(self.instantiate_default_expr(index, context)),
            },
            hir::AssignTarget::Field { receiver, field } => hir::AssignTarget::Field {
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                field: self.instantiate_default_field(*field, context),
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
                equals,
                subject_ty,
            } => hir::Pattern::Literal {
                value: self.instantiate_default_expr(value, context),
                equals: self.instantiate_default_callable(*equals, context),
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
            value.origin = source.origin.instantiate(context.evaluation);
            return value;
        }
        if let hir::ExprKind::Capture(binding) = source.kind
            && let Some(value) = context.captures.get(&binding)
        {
            let mut value = value.clone();
            value.span = source.span;
            value.origin = source.origin.instantiate(context.evaluation);
            return value;
        }
        let kind = match &source.kind {
            hir::ExprKind::StringLiteral(value) => hir::ExprKind::StringLiteral(value.clone()),
            hir::ExprKind::IntLiteral(value) => hir::ExprKind::IntLiteral(*value),
            hir::ExprKind::BoolLiteral(value) => hir::ExprKind::BoolLiteral(*value),
            hir::ExprKind::UnitLiteral => hir::ExprKind::UnitLiteral,
            hir::ExprKind::TupleLiteral(elements) => hir::ExprKind::TupleLiteral(
                elements
                    .iter()
                    .map(|element| self.instantiate_default_expr(element, context))
                    .collect(),
            ),
            hir::ExprKind::StructInit { application, args } => hir::ExprKind::StructInit {
                application: self.instantiate_default_struct_application(*application, context),
                args: self.instantiate_default_exprs(args, context),
            },
            hir::ExprKind::ClassInit { application, args } => hir::ExprKind::ClassInit {
                application: self.instantiate_default_class_application(*application, context),
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
            hir::ExprKind::PtrFromUInt(value) => {
                hir::ExprKind::PtrFromUInt(Box::new(self.instantiate_default_expr(value, context)))
            }
            hir::ExprKind::PtrToUInt(value) => {
                hir::ExprKind::PtrToUInt(Box::new(self.instantiate_default_expr(value, context)))
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
            hir::ExprKind::FunPtrNull => hir::ExprKind::FunPtrNull,
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
            hir::ExprKind::MethodCall {
                receiver,
                callee,
                args,
            } => hir::ExprKind::MethodCall {
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
            hir::ExprKind::Index { receiver, index } => hir::ExprKind::Index {
                receiver: Box::new(self.instantiate_default_expr(receiver, context)),
                index: Box::new(self.instantiate_default_expr(index, context)),
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
            origin: source.origin.instantiate(context.evaluation),
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

    fn instantiate_default_callable(
        &mut self,
        source: hir::Callable,
        context: &InstantiationContext,
    ) -> hir::Callable {
        match source {
            hir::Callable::Function(function) => hir::Callable::Function(function),
            hir::Callable::Generic(application) => {
                let application = self.instantiations[application].clone();
                let function = self.generic_functions[application.generic].function;
                let arguments = application
                    .type_args
                    .into_iter()
                    .map(|ty| self.instantiate_method_ty(ty, &context.bindings))
                    .collect();
                hir::Callable::Generic(self.record_instantiation(function, arguments))
            }
            hir::Callable::Method(application) => {
                let application = self.method_applications[application].clone();
                let owner = self.instantiate_default_method_owner(application.owner, context);
                hir::Callable::Method(self.record_method_application(application.function, owner))
            }
            hir::Callable::GenericMethod(application) => {
                let application = self.generic_method_applications[application].clone();
                let function = self.generic_methods[application.method].function;
                let owner = match application.owner {
                    hir::GenericMethodOwner::Class(owner) => hir::GenericMethodOwner::Class(
                        self.instantiate_default_class_application(owner, context),
                    ),
                    hir::GenericMethodOwner::Struct(owner) => hir::GenericMethodOwner::Struct(
                        self.instantiate_default_struct_application(owner, context),
                    ),
                    hir::GenericMethodOwner::Enum(owner) => hir::GenericMethodOwner::Enum(
                        self.instantiate_default_enum_application(owner, context),
                    ),
                };
                let arguments = application
                    .method_arguments
                    .iter()
                    .map(|ty| self.instantiate_method_ty(*ty, &context.bindings))
                    .collect();
                hir::Callable::GenericMethod(
                    self.record_generic_method_application(function, owner, arguments),
                )
            }
        }
    }

    fn instantiate_default_method_callee(
        &mut self,
        source: hir::MethodCallee,
        context: &InstantiationContext,
    ) -> hir::MethodCallee {
        match source {
            hir::MethodCallee::Callable(callable) => {
                hir::MethodCallee::Callable(self.instantiate_default_callable(callable, context))
            }
            hir::MethodCallee::Bound(bound) => {
                let source = self.bound_callable_refs[bound].clone();
                let bound = self.instantiate_default_interface_application(source.bound, context);
                let instantiated_signature =
                    self.instantiate_default_function_type(source.instantiated_signature, context);
                let value = hir::BoundCallableRef {
                    receiver_parameter: source.receiver_parameter,
                    bound,
                    member: source.member,
                    instantiated_signature,
                };
                let existing = self
                    .bound_callable_refs
                    .iter()
                    .find_map(|(id, existing)| (existing == &value).then_some(id));
                let id = existing.unwrap_or_else(|| self.bound_callable_refs.alloc(value));
                hir::MethodCallee::Bound(id)
            }
            hir::MethodCallee::DerivedEquality(application) => {
                let source = self.derived_equality_applications[application].clone();
                let ty = self.instantiate_method_ty(source.owner_ty, &context.bindings);
                if ty == source.owner_ty {
                    hir::MethodCallee::DerivedEquality(application)
                } else {
                    let candidate = self
                        .derived_equality_candidate(ty, source.span)
                        .expect("a validated default keeps a valid equality derivation")
                        .expect("the original expression has a derived equality target");
                    let application = match candidate {
                        crate::derived::DerivedEqualityCandidate::Nominal {
                            application, ..
                        }
                        | crate::derived::DerivedEqualityCandidate::Structural {
                            application,
                            ..
                        } => application,
                    };
                    hir::MethodCallee::DerivedEquality(application)
                }
            }
        }
    }

    fn instantiate_default_method_owner(
        &mut self,
        source: hir::MethodOwnerApplication,
        context: &InstantiationContext,
    ) -> hir::MethodOwnerApplication {
        match source {
            hir::MethodOwnerApplication::Class(application) => hir::MethodOwnerApplication::Class(
                self.instantiate_default_class_application(application, context),
            ),
            hir::MethodOwnerApplication::Struct(application) => {
                hir::MethodOwnerApplication::Struct(
                    self.instantiate_default_struct_application(application, context),
                )
            }
            hir::MethodOwnerApplication::Enum(application) => hir::MethodOwnerApplication::Enum(
                self.instantiate_default_enum_application(application, context),
            ),
            hir::MethodOwnerApplication::Interface(application) => {
                hir::MethodOwnerApplication::Interface(
                    self.instantiate_default_interface_application(application, context),
                )
            }
        }
    }

    fn instantiate_default_struct_application(
        &mut self,
        source: hir::StructApplicationId,
        context: &InstantiationContext,
    ) -> hir::StructApplicationId {
        let source = self.struct_applications[source].clone();
        let arguments = source
            .arguments
            .into_iter()
            .map(|ty| self.instantiate_method_ty(ty, &context.bindings))
            .collect();
        self.struct_application_id(source.template, arguments)
    }

    fn instantiate_default_enum_application(
        &mut self,
        source: hir::EnumApplicationId,
        context: &InstantiationContext,
    ) -> hir::EnumApplicationId {
        let source = self.enum_applications[source].clone();
        let arguments = source
            .arguments
            .into_iter()
            .map(|ty| self.instantiate_method_ty(ty, &context.bindings))
            .collect();
        self.enum_application_id(source.template, arguments)
    }

    fn instantiate_default_class_application(
        &mut self,
        source: hir::ClassApplicationId,
        context: &InstantiationContext,
    ) -> hir::ClassApplicationId {
        let source = self.class_applications[source].clone();
        let arguments = source
            .arguments
            .into_iter()
            .map(|ty| self.instantiate_method_ty(ty, &context.bindings))
            .collect();
        self.class_application_id(source.template, arguments)
    }

    fn instantiate_default_interface_application(
        &mut self,
        source: hir::InterfaceApplicationId,
        context: &InstantiationContext,
    ) -> hir::InterfaceApplicationId {
        let source = self.interface_applications[source].clone();
        let arguments = source
            .arguments
            .into_iter()
            .map(|ty| self.instantiate_method_ty(ty, &context.bindings))
            .collect();
        self.interface_application_id(source.template, arguments)
    }

    fn instantiate_default_function_type(
        &mut self,
        source: hir::FunctionTypeId,
        context: &InstantiationContext,
    ) -> hir::FunctionTypeId {
        let canonical = self.function_types[source].canonical_type;
        let ty = self.instantiate_method_ty(canonical, &context.bindings);
        let Type::Function(function) = self.types[ty] else {
            unreachable!("function type substitution remains a function type")
        };
        function
    }

    fn instantiate_default_field(
        &mut self,
        source: hir::FieldRef,
        context: &InstantiationContext,
    ) -> hir::FieldRef {
        match source {
            hir::FieldRef::StructField { application, index } => hir::FieldRef::StructField {
                application: self.instantiate_default_struct_application(application, context),
                index,
            },
            hir::FieldRef::TupleIndex(index) => hir::FieldRef::TupleIndex(index),
            hir::FieldRef::ClassField { application, index } => hir::FieldRef::ClassField {
                application: self.instantiate_default_class_application(application, context),
                index,
            },
        }
    }

    fn instantiate_default_coercion(
        &mut self,
        source: hir::FunctionCoercionId,
        context: &InstantiationContext,
    ) -> hir::FunctionCoercionId {
        let value = self.function_coercions[source].clone();
        let source = self.instantiate_default_function_type(value.source, context);
        let target = self.instantiate_default_function_type(value.target, context);
        if let Some(&coercion) = self.function_coercion_by_types.get(&(source, target)) {
            return coercion;
        }
        let coercion = self
            .function_coercions
            .alloc(hir::FunctionCoercion { source, target });
        self.function_coercion_by_types
            .insert((source, target), coercion);
        coercion
    }

    fn instantiate_default_foreign_callback(
        &mut self,
        source: hir::ForeignCallbackRegistrationId,
        context: &InstantiationContext,
    ) -> hir::ForeignCallbackRegistrationId {
        let source = self.foreign_callback_registrations[source].clone();
        let native_function_type =
            self.instantiate_default_function_type(source.native_function_type, context);
        let managed_function_type =
            self.instantiate_default_function_type(source.managed_function_type, context);
        self.foreign_callback_registrations
            .alloc(hir::ForeignCallbackRegistration {
                native_function_type,
                managed_function_type,
                context_index: source.context_index,
                mode: source.mode,
            })
    }

    fn instantiate_default_lambda(
        &mut self,
        source: hir::LambdaId,
        context: &mut InstantiationContext,
    ) -> hir::LambdaId {
        let source = self.lambdas[source].clone();
        let body_type_arguments =
            self.instantiate_callable_body_arguments(source.function, context);
        let function_type = self.instantiate_default_function_type(source.function_type, context);
        let captures = source
            .captures
            .iter()
            .map(|capture| self.instantiate_default_capture(capture, context))
            .collect();
        self.lambdas.alloc(hir::Lambda {
            function: source.function,
            function_type,
            owner_type_param_count: source.owner_type_param_count,
            body_type_arguments,
            captures,
            span: source.span,
        })
    }

    fn instantiate_default_anonymous(
        &mut self,
        source: hir::AnonymousFunctionId,
        context: &mut InstantiationContext,
    ) -> hir::AnonymousFunctionId {
        let source = self.anonymous_functions[source].clone();
        let body_type_arguments =
            self.instantiate_callable_body_arguments(source.function, context);
        let function_type = self.instantiate_default_function_type(source.function_type, context);
        let captures = source
            .captures
            .iter()
            .map(|capture| self.instantiate_default_capture(capture, context))
            .collect();
        self.anonymous_functions.alloc(hir::AnonymousFunction {
            function: source.function,
            function_type,
            owner_type_param_count: source.owner_type_param_count,
            body_type_arguments,
            captures,
            span: source.span,
        })
    }

    fn instantiate_default_reference(
        &mut self,
        source: hir::CallableReferenceId,
        context: &mut InstantiationContext,
    ) -> hir::CallableReferenceId {
        let source = self.callable_references[source].clone();
        let target = match source.target {
            hir::CallableReferenceTarget::Named(callee) => hir::CallableReferenceTarget::Named(
                self.instantiate_default_callable(callee, context),
            ),
            hir::CallableReferenceTarget::Local {
                local_function,
                callee,
            } => hir::CallableReferenceTarget::Local {
                local_function,
                callee: self.instantiate_default_callable(callee, context),
            },
            hir::CallableReferenceTarget::BoundMember { receiver, callee } => {
                hir::CallableReferenceTarget::BoundMember {
                    receiver: Box::new(self.instantiate_default_expr(&receiver, context)),
                    callee: self.instantiate_default_method_callee(callee, context),
                }
            }
            hir::CallableReferenceTarget::BoundExtension { receiver, callee } => {
                hir::CallableReferenceTarget::BoundExtension {
                    receiver: Box::new(self.instantiate_default_expr(&receiver, context)),
                    callee: self.instantiate_default_callable(callee, context),
                }
            }
        };
        let function_type = self.instantiate_default_function_type(source.function_type, context);
        let captures = source
            .captures
            .iter()
            .map(|capture| self.instantiate_default_capture(capture, context))
            .collect();
        self.callable_references.alloc(hir::CallableReference {
            target,
            function_type,
            owner_type_param_count: source.owner_type_param_count,
            captures,
            span: source.span,
        })
    }

    fn instantiate_default_capture(
        &mut self,
        source: &hir::Capture,
        context: &mut InstantiationContext,
    ) -> hir::Capture {
        hir::Capture {
            binding: source.binding,
            name: source.name.clone(),
            ty: self.instantiate_method_ty(source.ty, &context.bindings),
            first_use_span: source.first_use_span,
            source: self.instantiate_default_expr(&source.source, context),
        }
    }

    fn instantiate_callable_body_arguments(
        &mut self,
        function: hir::FunctionId,
        context: &InstantiationContext,
    ) -> hir::CallableBodyTypeArguments {
        let parameters = self.functions[function]
            .type_params()
            .into_iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        let arguments = parameters
            .into_iter()
            .map(|parameter| {
                let ty = self.intern_type(Type::Param(parameter));
                self.instantiate_method_ty(ty, &context.bindings)
            })
            .collect();
        hir::CallableBodyTypeArguments::Explicit(arguments)
    }
}

fn mapped_local(context: &InstantiationContext, source: hir::LocalId) -> hir::LocalId {
    let hir::ExprKind::Local(local) = context.locals[arena_index(source)].kind else {
        unreachable!("default template places only refer to mapped local temporaries")
    };
    local
}

fn arena_index<T>(id: la_arena::Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}
