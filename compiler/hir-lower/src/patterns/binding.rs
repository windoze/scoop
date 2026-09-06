//! Transactional planning for irrefutable binding patterns.
//!
//! The HIR-owned plan keeps declaration-order shape separate from
//! source-order runtime actions. `val` / `var` and lambda owners consume and
//! expand it before Export HIR is published; source `for` will retain the same
//! typed model until LocalConcrete HIR expansion.

use std::collections::{HashMap, HashSet};

use super::*;

/// A completed `val` / `var` plan plus its owner-specific initializer. The
/// shared HIR plan itself starts from an existing typed subject temporary so a
/// lambda parameter or future `for` payload can use it without a fake init.
pub(crate) struct LoweredIrrefutableBindingPlan {
    subject: BindingSubject,
    plan: hir::IrrefutableBindingPlan,
}

enum BindingSubject {
    Existing,
    Initialize { init: hir::Expr, span: Span },
}

impl LoweredIrrefutableBindingPlan {
    /// Expand the validated plan in runtime order. Every emitted declaration
    /// has a single binding target, so recursive source binding patterns do
    /// not cross the Export HIR boundary for this owner.
    pub(crate) fn into_statements(self) -> Vec<hir::Statement> {
        validate_leaf_schedule(&self.plan);
        let hir::IrrefutableBindingPlan {
            subject,
            shape: _,
            actions,
        } = self.plan;
        let mut statements = Vec::with_capacity(actions.len() + 1);
        if let BindingSubject::Initialize { init, span } = self.subject {
            statements.push(binding_statement(subject.local, init, span));
        }
        for action in actions {
            match action {
                hir::IrrefutableBindingAction::Project {
                    source,
                    result,
                    projection,
                    span,
                    origin,
                } => {
                    let field = match projection {
                        hir::BindingProjection::TupleIndex(index) => {
                            hir::FieldRef::TupleIndex(index)
                        }
                        hir::BindingProjection::StructField(field) => {
                            hir::FieldRef::StructField(field)
                        }
                    };
                    let init = hir::Expr {
                        kind: hir::ExprKind::FieldAccess {
                            receiver: Box::new(binding_local_expr(source, span, origin)),
                            field,
                        },
                        ty: result.ty,
                        span,
                        origin,
                    };
                    statements.push(binding_statement(result.local, init, span));
                }
                hir::IrrefutableBindingAction::Component {
                    source,
                    index,
                    result,
                    mut setup,
                    call,
                    span,
                } => {
                    assert_component_call_source(&setup, &call, source);
                    assert_ne!(index.get(), 0);
                    assert_eq!(call.ty, result.ty);
                    statements.append(&mut setup);
                    statements.push(binding_statement(result.local, call, span));
                }
                hir::IrrefutableBindingAction::Bind {
                    source,
                    target,
                    span,
                    origin,
                } => {
                    let init = binding_local_expr(source, span, origin);
                    statements.push(binding_statement(target.local, init, span));
                }
            }
        }
        statements
    }
}

fn binding_statement(local: hir::LocalId, init: hir::Expr, span: Span) -> hir::Statement {
    hir::Statement {
        kind: hir::StatementKind::ValDecl {
            pattern: hir::Pattern::Binding { local },
            init,
        },
        span,
    }
}

fn binding_local_expr(
    source: hir::BindingTemporary,
    span: Span,
    origin: hir::ExpressionOrigin,
) -> hir::Expr {
    hir::Expr {
        kind: hir::ExprKind::Local(source.local),
        ty: source.ty,
        span,
        origin,
    }
}

fn component_call_receiver(call: &hir::Expr) -> &hir::Expr {
    match &call.kind {
        hir::ExprKind::MethodCall { receiver, args, .. } => {
            assert!(
                args.is_empty(),
                "a component member has no explicit arguments"
            );
            receiver.as_ref()
        }
        hir::ExprKind::Call { args, .. } => {
            let [receiver] = args.as_slice() else {
                panic!("an extension component call has exactly its receiver argument")
            };
            receiver
        }
        _ => panic!("component resolution must produce a direct or method call"),
    }
}

fn component_setup_binding(setup: &[hir::Statement]) -> (hir::LocalId, &hir::Expr) {
    let [statement] = setup else {
        panic!("component setup must materialize exactly one receiver temporary")
    };
    let hir::StatementKind::ValDecl {
        pattern: hir::Pattern::Binding { local },
        init,
    } = &statement.kind
    else {
        panic!("component setup must be one immutable receiver binding")
    };
    (*local, init)
}

fn assert_component_call_source(
    setup: &[hir::Statement],
    call: &hir::Expr,
    source: hir::BindingTemporary,
) {
    let (receiver_local, init) = component_setup_binding(setup);
    let hir::ExprKind::Local(initializer_source) = init.kind else {
        panic!("a component receiver temporary must read its source directly")
    };
    assert_eq!(initializer_source, source.local);
    let receiver = component_call_receiver(call);
    let hir::ExprKind::Local(call_source) = receiver.kind else {
        panic!("a component call must read its materialized receiver directly")
    };
    assert_eq!(call_source, receiver_local);
    assert_eq!(receiver.ty, init.ty);
}

fn validate_leaf_schedule(plan: &hir::IrrefutableBindingPlan) {
    let mut temporaries = HashMap::new();
    assert!(
        temporaries
            .insert(plan.subject.local, plan.subject.ty)
            .is_none()
    );
    for action in &plan.actions {
        match action {
            hir::IrrefutableBindingAction::Project { source, result, .. } => {
                assert_eq!(temporaries.get(&source.local), Some(&source.ty));
                assert!(temporaries.insert(result.local, result.ty).is_none());
            }
            hir::IrrefutableBindingAction::Component {
                source,
                index,
                result,
                setup,
                call,
                ..
            } => {
                assert_eq!(temporaries.get(&source.local), Some(&source.ty));
                assert_component_call_source(setup, call, *source);
                assert_ne!(index.get(), 0);
                assert_eq!(call.ty, result.ty);
                assert!(temporaries.insert(result.local, result.ty).is_none());
            }
            hir::IrrefutableBindingAction::Bind { source, target, .. } => {
                assert_eq!(temporaries.get(&source.local), Some(&source.ty));
                assert_eq!(source.ty, target.ty);
                assert!(!temporaries.contains_key(&target.local));
            }
        }
    }
    let mut leaves = Vec::new();
    collect_shape_leaves(&plan.shape, &mut leaves);
    let scheduled = plan
        .actions
        .iter()
        .filter_map(|action| match action {
            hir::IrrefutableBindingAction::Bind { target, .. } => Some(*target),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(leaves.len(), scheduled.len());
    for leaf in leaves {
        assert_eq!(
            scheduled
                .iter()
                .filter(|scheduled| **scheduled == leaf)
                .count(),
            1,
            "every shape leaf must have exactly one runtime bind action",
        );
    }
}

fn collect_shape_leaves(shape: &hir::IrrefutableBindingShape, out: &mut Vec<hir::BindingLeaf>) {
    match shape {
        hir::IrrefutableBindingShape::Binding(leaf) => out.push(*leaf),
        hir::IrrefutableBindingShape::Wildcard => {}
        hir::IrrefutableBindingShape::Tuple(elements) => {
            for element in elements {
                collect_shape_leaves(element, out);
            }
        }
        hir::IrrefutableBindingShape::Struct {
            application,
            fields,
        } => {
            for (index, (field, shape)) in fields.iter().enumerate() {
                assert_eq!(field.application(), *application);
                assert_eq!(field.local_index(), index as u32);
                collect_shape_leaves(shape, out);
            }
        }
        hir::IrrefutableBindingShape::Class { components, .. } => {
            for (_, shape) in components {
                collect_shape_leaves(shape, out);
            }
        }
    }
}

impl Lowerer {
    /// Build one owner-scoped plan on a cloned lowering state. A late error
    /// publishes only new fatal diagnostics and discards locals, scopes,
    /// warnings, counters, instantiated entities, and the partial schedule.
    pub(crate) fn lower_irrefutable_binding_plan(
        &mut self,
        pattern: &ast::Pattern,
        subject_init: hir::Expr,
        mutable: bool,
        span: Span,
    ) -> Option<LoweredIrrefutableBindingPlan> {
        self.with_pattern_transaction(|state| {
            let subject = hir::BindingTemporary {
                local: state.alloc_hidden("binding.subject", subject_init.ty),
                ty: subject_init.ty,
            };
            state.build_irrefutable_binding_plan(
                pattern,
                subject,
                BindingSubject::Initialize {
                    init: subject_init,
                    span,
                },
                mutable,
            )
        })
    }

    /// Build a plan whose immutable subject storage already exists inside the
    /// surrounding lambda-owner transaction. Expansion emits only the action
    /// prefix and never creates a second source parameter or subject copy.
    pub(crate) fn lower_irrefutable_binding_plan_from_subject(
        &mut self,
        pattern: &ast::Pattern,
        subject: hir::BindingTemporary,
        mutable: bool,
    ) -> Option<LoweredIrrefutableBindingPlan> {
        self.build_irrefutable_binding_plan(pattern, subject, BindingSubject::Existing, mutable)
    }

    fn build_irrefutable_binding_plan(
        &mut self,
        pattern: &ast::Pattern,
        subject: hir::BindingTemporary,
        subject_storage: BindingSubject,
        mutable: bool,
    ) -> Option<LoweredIrrefutableBindingPlan> {
        let mut actions = Vec::new();
        let shape = self.plan_irrefutable_pattern(pattern, subject, mutable, &mut actions)?;
        Some(LoweredIrrefutableBindingPlan {
            subject: subject_storage,
            plan: hir::IrrefutableBindingPlan {
                subject,
                shape,
                actions,
            },
        })
    }

    fn plan_irrefutable_pattern(
        &mut self,
        pattern: &ast::Pattern,
        subject: hir::BindingTemporary,
        mutable: bool,
        actions: &mut Vec<hir::IrrefutableBindingAction>,
    ) -> Option<hir::IrrefutableBindingShape> {
        match pattern {
            ast::Pattern::Binding(name) => {
                let local = self.bind_local(name, subject.ty, mutable)?;
                let target = hir::BindingLeaf {
                    local,
                    ty: subject.ty,
                    mutability: if mutable {
                        hir::BindingMutability::Mutable
                    } else {
                        hir::BindingMutability::Immutable
                    },
                };
                actions.push(hir::IrrefutableBindingAction::Bind {
                    source: subject,
                    target,
                    span: name.span,
                    origin: self.expression_origin(name.span),
                });
                Some(hir::IrrefutableBindingShape::Binding(target))
            }
            ast::Pattern::Wildcard { .. } => Some(hir::IrrefutableBindingShape::Wildcard),
            ast::Pattern::Literal { span, .. } => {
                self.error(
                    *span,
                    "refutable patterns are only allowed in `when`".to_string(),
                );
                None
            }
            ast::Pattern::Tuple {
                elements,
                rest,
                span,
            } => match self.types[subject.ty].clone() {
                Type::Tuple(element_types) => {
                    let owner = format!("tuple of type {}", self.type_name(subject.ty));
                    self.plan_tuple_binding(
                        elements,
                        *rest,
                        *span,
                        subject,
                        &element_types,
                        &owner,
                        mutable,
                        actions,
                    )
                }
                Type::Struct(application) => self.plan_struct_positional_binding(
                    application,
                    elements,
                    *rest,
                    *span,
                    subject,
                    mutable,
                    actions,
                ),
                Type::Class(application) => {
                    self.plan_class_binding(application, elements, *rest, subject, mutable, actions)
                }
                _ => {
                    let found = self.type_name(subject.ty);
                    self.error(
                        *span,
                        format!("tuple pattern does not match a subject of type {found}"),
                    );
                    None
                }
            },
            ast::Pattern::Positional {
                path,
                elements,
                rest,
                span,
            } => match self.resolve_pattern_path(path, subject.ty, *span)? {
                PatternTarget::Variant(..) => {
                    self.error(
                        *span,
                        "refutable patterns are only allowed in `when`".to_string(),
                    );
                    None
                }
                PatternTarget::Struct(application) => self.plan_struct_positional_binding(
                    application,
                    elements,
                    *rest,
                    *span,
                    subject,
                    mutable,
                    actions,
                ),
            },
            ast::Pattern::Named {
                path,
                fields,
                rest,
                span,
            } => match self.resolve_pattern_path(path, subject.ty, *span)? {
                PatternTarget::Variant(..) => {
                    self.error(
                        *span,
                        "refutable patterns are only allowed in `when`".to_string(),
                    );
                    None
                }
                PatternTarget::Struct(application) => self.plan_struct_named_binding(
                    application,
                    fields,
                    *rest,
                    *span,
                    subject,
                    mutable,
                    actions,
                ),
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn plan_class_binding(
        &mut self,
        application: hir::ClassApplicationId,
        elements: &[ast::Pattern],
        rest: Option<Span>,
        subject: hir::BindingTemporary,
        mutable: bool,
        actions: &mut Vec<hir::IrrefutableBindingAction>,
    ) -> Option<hir::IrrefutableBindingShape> {
        if let Some(rest) = rest {
            self.error(
                rest,
                "a class component pattern cannot contain `..`".to_string(),
            );
            return None;
        }

        let mut components = Vec::with_capacity(elements.len());
        for (position, element) in elements.iter().enumerate() {
            let index = std::num::NonZeroU32::new(
                u32::try_from(position)
                    .expect("a source pattern cannot contain more than u32::MAX elements")
                    .checked_add(1)
                    .expect("class component indices cannot overflow"),
            )
            .expect("class component indices start at one");
            let at = binding_pattern_span(element);
            let mut setup = Vec::new();
            let diagnostics_before = self.diagnostics.len();
            let call = self.lower_component_call(
                binding_local_expr(subject, at, self.expression_origin(at)),
                index,
                at,
                &mut setup,
            )?;
            if self.diagnostics.len() != diagnostics_before {
                return None;
            }
            let winner = self.component_call_function(&call);
            let result = hir::BindingTemporary {
                local: self.alloc_hidden("binding.component", call.ty),
                ty: call.ty,
            };
            self.validate_component_action(
                application,
                subject,
                index,
                result,
                &setup,
                &call,
                winner,
            );
            actions.push(hir::IrrefutableBindingAction::Component {
                source: subject,
                index,
                result,
                setup,
                call,
                span: at,
            });
            let shape = self.plan_irrefutable_pattern(element, result, mutable, actions)?;
            components.push((index, shape));
        }
        Some(hir::IrrefutableBindingShape::Class {
            application,
            components,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_component_action(
        &mut self,
        application: hir::ClassApplicationId,
        source: hir::BindingTemporary,
        index: std::num::NonZeroU32,
        result: hir::BindingTemporary,
        setup: &[hir::Statement],
        call: &hir::Expr,
        winner: hir::FunctionId,
    ) {
        let Type::Class(source_application) = self.types[source.ty] else {
            panic!("only a class subject can own a component action")
        };
        assert_eq!(source_application, application);
        assert_eq!(
            self.functions[winner].modifiers.operator,
            Some(hir::OperatorKind::Component { index }),
            "a planned component action must retain its exact typed operator winner",
        );

        let (receiver_local, init) = component_setup_binding(setup);
        assert_component_call_source(setup, call, source);
        assert!(
            self.is_subtype(source.ty, init.ty),
            "a component receiver setup must preserve a valid source adaptation",
        );
        assert_ne!(receiver_local, source.local);
        assert_eq!(self.locals[receiver_local].ty, init.ty);
        assert!(!self.locals[receiver_local].mutable);
        assert_eq!(self.locals[result.local].ty, result.ty);
        assert!(!self.locals[result.local].mutable);

        let extension = self.extension_receivers.contains_key(&winner);
        assert_eq!(matches!(call.kind, hir::ExprKind::Call { .. }), extension);
        assert_eq!(
            matches!(call.kind, hir::ExprKind::MethodCall { .. }),
            !extension
        );
    }

    fn component_call_function(&self, call: &hir::Expr) -> hir::FunctionId {
        let callee = match &call.kind {
            hir::ExprKind::Call { callee, .. } => return self.callable_function_id(*callee),
            hir::ExprKind::MethodCall { callee, .. } => *callee,
            _ => panic!("component resolution must produce a direct or method call"),
        };
        match callee {
            hir::MethodCallee::Callable(callable) => self.callable_function_id(callable),
            hir::MethodCallee::Bound(bound) => match self.bound_callable_refs[bound].source {
                hir::BoundCallableSource::Class { callable, .. } => {
                    self.callable_function_id(callable)
                }
                hir::BoundCallableSource::Interface { member, .. } => {
                    self.interface_method_entities[member].function
                }
            },
            hir::MethodCallee::DerivedEquality(_) => {
                panic!("component resolution cannot produce derived equality")
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn plan_tuple_binding(
        &mut self,
        elements: &[ast::Pattern],
        rest: Option<Span>,
        span: Span,
        subject: hir::BindingTemporary,
        element_types: &[TypeId],
        owner: &str,
        mutable: bool,
        actions: &mut Vec<hir::IrrefutableBindingAction>,
    ) -> Option<hir::IrrefutableBindingShape> {
        let indices =
            self.positional_pattern_indices(elements, rest, element_types.len(), owner, span)?;
        let mut shapes: Vec<Option<hir::IrrefutableBindingShape>> =
            (0..element_types.len()).map(|_| None).collect();
        for (element, index) in elements.iter().zip(indices) {
            if matches!(element, ast::Pattern::Wildcard { .. }) {
                shapes[index] = Some(hir::IrrefutableBindingShape::Wildcard);
                continue;
            }
            let at = binding_pattern_span(element);
            let projected = self.plan_binding_projection(
                subject,
                hir::BindingProjection::TupleIndex(index as u32),
                element_types[index],
                at,
                actions,
            );
            shapes[index] =
                Some(self.plan_irrefutable_pattern(element, projected, mutable, actions)?);
        }
        Some(hir::IrrefutableBindingShape::Tuple(
            shapes
                .into_iter()
                .map(|shape| shape.unwrap_or(hir::IrrefutableBindingShape::Wildcard))
                .collect(),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn plan_struct_positional_binding(
        &mut self,
        application: hir::StructApplicationId,
        elements: &[ast::Pattern],
        rest: Option<Span>,
        span: Span,
        subject: hir::BindingTemporary,
        mutable: bool,
        actions: &mut Vec<hir::IrrefutableBindingAction>,
    ) -> Option<hir::IrrefutableBindingShape> {
        let application_value = self.struct_applications[application].clone();
        let struct_id = application_value.template;
        let owner = format!("struct `{}`", self.structs[struct_id].name);
        let declared_types: Vec<TypeId> = self.structs[struct_id]
            .semantic_fields()
            .iter()
            .map(|field| field.ty)
            .collect();
        let field_types: Vec<TypeId> = declared_types
            .into_iter()
            .map(|ty| self.instantiate_ty(ty, &application_value.arguments))
            .collect();
        let indices =
            self.positional_pattern_indices(elements, rest, field_types.len(), &owner, span)?;
        self.plan_struct_binding_fields(
            application,
            elements
                .iter()
                .zip(indices)
                .map(|(pattern, index)| (pattern, index, binding_pattern_span(pattern))),
            subject,
            &field_types,
            mutable,
            actions,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn plan_struct_named_binding(
        &mut self,
        application: hir::StructApplicationId,
        fields: &[ast::FieldPattern],
        rest: Option<Span>,
        span: Span,
        subject: hir::BindingTemporary,
        mutable: bool,
        actions: &mut Vec<hir::IrrefutableBindingAction>,
    ) -> Option<hir::IrrefutableBindingShape> {
        let application_value = self.struct_applications[application].clone();
        let struct_id = application_value.template;
        let owner = format!("struct `{}`", self.structs[struct_id].name);
        let declared: Vec<(String, TypeId)> = self.structs[struct_id]
            .semantic_fields()
            .iter()
            .map(|field| (field.name.clone(), field.ty))
            .collect();
        let field_types: Vec<(String, TypeId)> = declared
            .into_iter()
            .map(|(name, ty)| (name, self.instantiate_ty(ty, &application_value.arguments)))
            .collect();
        if rest.is_none() && fields.len() < field_types.len() {
            self.error(
                span,
                format!("pattern does not list all fields of {owner}; add `..` to ignore the rest"),
            );
            return None;
        }

        let mut seen = HashSet::new();
        let mut indexed = Vec::with_capacity(fields.len());
        for field in fields {
            if field.field.text == "_" {
                self.error(
                    field.field.span,
                    "`_` is not allowed in a field pattern".to_string(),
                );
                return None;
            }
            if !seen.insert(field.field.text.clone()) {
                self.error(
                    field.field.span,
                    format!("duplicate field `{}` in pattern", field.field.text),
                );
                return None;
            }
            let Some(index) = field_types
                .iter()
                .position(|(name, _)| name == &field.field.text)
            else {
                self.error(
                    field.field.span,
                    format!("{owner} has no field `{}`", field.field.text),
                );
                return None;
            };
            indexed.push((&*field.subpattern, index, field.field.span));
        }

        let field_types = field_types.iter().map(|(_, ty)| *ty).collect::<Vec<_>>();
        self.plan_struct_binding_fields(
            application,
            indexed.into_iter(),
            subject,
            &field_types,
            mutable,
            actions,
        )
    }

    fn plan_struct_binding_fields<'pattern>(
        &mut self,
        application: hir::StructApplicationId,
        fields: impl Iterator<Item = (&'pattern ast::Pattern, usize, Span)>,
        subject: hir::BindingTemporary,
        field_types: &[TypeId],
        mutable: bool,
        actions: &mut Vec<hir::IrrefutableBindingAction>,
    ) -> Option<hir::IrrefutableBindingShape> {
        let mut shapes: Vec<Option<hir::IrrefutableBindingShape>> =
            (0..field_types.len()).map(|_| None).collect();
        for (pattern, index, span) in fields {
            let field = hir::AppliedStructFieldRef::checked(
                &self.structs,
                &self.struct_applications,
                application,
                index as u32,
            )
            .expect("a resolved struct pattern field has a checked identity");
            if matches!(pattern, ast::Pattern::Wildcard { .. }) {
                shapes[index] = Some(hir::IrrefutableBindingShape::Wildcard);
                continue;
            }
            let projected = self.plan_binding_projection(
                subject,
                hir::BindingProjection::StructField(field),
                field_types[index],
                span,
                actions,
            );
            shapes[index] =
                Some(self.plan_irrefutable_pattern(pattern, projected, mutable, actions)?);
        }
        let fields = shapes
            .into_iter()
            .enumerate()
            .map(|(index, shape)| {
                let field = hir::AppliedStructFieldRef::checked(
                    &self.structs,
                    &self.struct_applications,
                    application,
                    index as u32,
                )
                .expect("every declaration-order field has a checked identity");
                (
                    field,
                    shape.unwrap_or(hir::IrrefutableBindingShape::Wildcard),
                )
            })
            .collect();
        Some(hir::IrrefutableBindingShape::Struct {
            application,
            fields,
        })
    }

    fn plan_binding_projection(
        &mut self,
        source: hir::BindingTemporary,
        projection: hir::BindingProjection,
        ty: TypeId,
        span: Span,
        actions: &mut Vec<hir::IrrefutableBindingAction>,
    ) -> hir::BindingTemporary {
        let result = hir::BindingTemporary {
            local: self.alloc_hidden("binding.projection", ty),
            ty,
        };
        actions.push(hir::IrrefutableBindingAction::Project {
            source,
            result,
            projection,
            span,
            origin: self.expression_origin(span),
        });
        result
    }
}

fn binding_pattern_span(pattern: &ast::Pattern) -> Span {
    match pattern {
        ast::Pattern::Binding(name) => name.span,
        ast::Pattern::Wildcard { span }
        | ast::Pattern::Literal { span, .. }
        | ast::Pattern::Positional { span, .. }
        | ast::Pattern::Named { span, .. }
        | ast::Pattern::Tuple { span, .. } => *span,
    }
}
