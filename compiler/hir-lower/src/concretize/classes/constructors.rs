//! Constructor executable bodies retain their typed evaluation context.

use super::*;

impl Concretizer<'_> {
    pub(in crate::concretize) fn lower_class_constructor(
        &mut self,
        source_id: export::ClassConstructorDefinition,
        class: concrete::ClassId,
        substitution: &[concrete::TypeId],
    ) -> PendingClassConstructor {
        let source = self.class_constructor_definition(source_id);
        let mut expression_origin = export::ExpressionOrigin::Definition(source.origin).concrete();
        expression_origin.evaluation.context = source.evaluation_context;
        let parameters = self.lower_constructor_parameters(source.parameters, substitution);
        let mut body = concrete::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        };
        let class_ty = self.class_type[&class];
        let kind = match source.kind {
            export::ClassConstructorKind::Primary {
                base,
                primary_stores,
                common_initialization,
            } => {
                self.append_base_initialization(
                    &mut body,
                    base,
                    substitution,
                    source.span,
                    expression_origin,
                );
                for store in primary_stores {
                    let (receiver_ty, field) =
                        self.lower_initializing_class_field(store.field, substitution);
                    let receiver =
                        self.constructor_receiver(receiver_ty, store.span, expression_origin);
                    let parameter = parameters
                        .iter()
                        .find(|parameter| parameter.id.into_raw() == store.parameter.into_raw())
                        .expect("a primary store retains its constructor parameter");
                    let value = concrete::Expr {
                        kind: concrete::ExprKind::ConstructorParam(parameter.id),
                        ty: parameter.ty,
                        span: store.span,
                        origin: expression_origin,
                    };
                    body.statements.push(concrete::Statement {
                        kind: concrete::StatementKind::Assign {
                            target: concrete::AssignTarget::Field {
                                receiver: Box::new(receiver),
                                field,
                            },
                            value,
                        },
                        span: store.span,
                    });
                }
                self.append_common_initialization(
                    &mut body,
                    common_initialization,
                    substitution,
                    expression_origin,
                );
                concrete::ClassConstructorKind::Terminal { body }
            }
            export::ClassConstructorKind::Secondary {
                delegation,
                body: secondary_body,
            } => match delegation {
                export::ClassSecondaryDelegation::This { target, arguments } => {
                    let target_id = self.lower_class_constructor_application(*target, substitution);
                    let args =
                        self.append_constructor_arguments(&mut body, arguments, substitution);
                    let receiver =
                        self.constructor_receiver(class_ty, source.span, expression_origin);
                    body.statements.push(concrete::Statement {
                        kind: concrete::StatementKind::Expr(concrete::Expr {
                            kind: concrete::ExprKind::ClassInitializerCall {
                                receiver: Box::new(receiver),
                                initializer: concrete::ClassInitializerTarget::Local(target_id),
                                args,
                            },
                            ty: self.lower_type(self.source.unit, &[]),
                            span: source.span,
                            origin: expression_origin,
                        }),
                        span: source.span,
                    });
                    self.append_source_body(&mut body, secondary_body, substitution);
                    concrete::ClassConstructorKind::This {
                        target: target_id,
                        body,
                    }
                }
                export::ClassSecondaryDelegation::Terminal {
                    base,
                    common_initialization,
                } => {
                    self.append_base_initialization(
                        &mut body,
                        base,
                        substitution,
                        source.span,
                        expression_origin,
                    );
                    self.append_common_initialization(
                        &mut body,
                        common_initialization,
                        substitution,
                        expression_origin,
                    );
                    self.append_source_body(&mut body, secondary_body, substitution);
                    concrete::ClassConstructorKind::Terminal { body }
                }
            },
        };
        PendingClassConstructor {
            class,
            source_discriminator: source.discriminator,
            safety: source.safety,
            origin: source.origin,
            parameters,
            kind,
        }
    }

    pub(in crate::concretize) fn lower_class_constructor_application(
        &mut self,
        source: export::ClassConstructorApplicationId,
        substitution: &[concrete::TypeId],
    ) -> concrete::ClassConstructorId {
        let application = &self.source.class_constructor_applications[source];
        let class = self.lower_class_application(application.owner, substitution);
        self.request_class_constructor_source(application.constructor, class)
    }

    pub(in crate::concretize) fn append_base_initialization(
        &mut self,
        body: &mut concrete::Body,
        base: &export::BaseInitialization,
        substitution: &[concrete::TypeId],
        span: scoop_ast::Span,
        origin: export::ConcreteExpressionOrigin,
    ) {
        let export::BaseInitialization::Super { target, arguments } = base else {
            return;
        };
        let (target, target_ty) = match target {
            export::BaseInitializerTarget::Local(target) => {
                let target = self.lower_class_constructor_application(*target, substitution);
                let (_, class) = self.class_constructor_keys[target.into_raw().into_u32() as usize];
                (
                    concrete::ClassInitializerTarget::Local(target),
                    self.class_type[&class],
                )
            }
            export::BaseInitializerTarget::Imported { owner, callable } => (
                concrete::ClassInitializerTarget::Imported(
                    self.imported_dependency_callable_map[callable],
                ),
                self.lower_type(*owner, substitution),
            ),
        };
        let args = self.append_constructor_arguments(body, arguments, substitution);
        let receiver = self.constructor_receiver(target_ty, span, origin);
        body.statements.push(concrete::Statement {
            kind: concrete::StatementKind::Expr(concrete::Expr {
                kind: concrete::ExprKind::ClassInitializerCall {
                    receiver: Box::new(receiver),
                    initializer: target,
                    args,
                },
                ty: self.lower_type(self.source.unit, &[]),
                span,
                origin,
            }),
            span,
        });
    }

    fn append_common_initialization(
        &mut self,
        body: &mut concrete::Body,
        common: &[export::ClassInitializationStep],
        substitution: &[concrete::TypeId],
        origin: export::ConcreteExpressionOrigin,
    ) {
        for step in common {
            match step {
                export::ClassInitializationStep::Field {
                    field,
                    initializer,
                    span,
                } => {
                    let locals = self.append_source_locals(body, &initializer.locals, substitution);
                    body.statements.extend(self.lower_statement_region(
                        &initializer.statements,
                        substitution,
                        &locals,
                    ));
                    let value = self.lower_expr(&initializer.value, substitution, &locals);
                    let (receiver_ty, field) =
                        self.lower_initializing_class_field(*field, substitution);
                    let receiver = self.constructor_receiver(receiver_ty, *span, origin);
                    body.statements.push(concrete::Statement {
                        kind: concrete::StatementKind::Assign {
                            target: concrete::AssignTarget::Field {
                                receiver: Box::new(receiver),
                                field,
                            },
                            value,
                        },
                        span: *span,
                    });
                }
                export::ClassInitializationStep::InitBlock {
                    body: source_body, ..
                } => self.append_source_body(body, source_body, substitution),
            }
        }
    }

    pub(in crate::concretize) fn append_constructor_arguments(
        &mut self,
        body: &mut concrete::Body,
        arguments: &export::ConstructorArguments,
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::Expr> {
        let locals = self.append_source_locals(body, &arguments.locals, substitution);
        body.statements.extend(self.lower_statement_region(
            &arguments.statements,
            substitution,
            &locals,
        ));
        arguments
            .args
            .iter()
            .map(|argument| self.lower_expr(argument, substitution, &locals))
            .collect()
    }

    pub(in crate::concretize) fn append_source_body(
        &mut self,
        body: &mut concrete::Body,
        source: &export::Body,
        substitution: &[concrete::TypeId],
    ) {
        let locals = self.append_source_locals(body, &source.locals, substitution);
        body.statements.extend(self.lower_statement_region(
            &source.statements,
            substitution,
            &locals,
        ));
    }

    pub(in crate::concretize) fn append_source_locals(
        &mut self,
        body: &mut concrete::Body,
        source: &Arena<export::Local>,
        substitution: &[concrete::TypeId],
    ) -> Vec<concrete::LocalId> {
        source
            .iter()
            .map(|(_, local)| {
                let ty = self.lower_type(local.ty, substitution);
                body.locals.alloc(concrete::Local {
                    binding: concrete::BindingId::from_raw(local.binding.into_raw()),
                    selector: local.selector.clone(),
                    definition: local.definition,
                    name: local.name.clone(),
                    ty,
                    mutable: local.mutable,
                })
            })
            .collect()
    }

    pub(in crate::concretize) fn constructor_receiver(
        &self,
        ty: concrete::TypeId,
        span: scoop_ast::Span,
        origin: export::ConcreteExpressionOrigin,
    ) -> concrete::Expr {
        concrete::Expr {
            kind: concrete::ExprKind::ConstructorReceiver,
            ty,
            span,
            origin,
        }
    }
}
