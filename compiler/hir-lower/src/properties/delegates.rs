use scoop_ast as ast;
use scoop_hir as hir;

use crate::overload::{LoweredOverloadCall, OverloadResolutionOutcome};
use crate::{
    ForbiddenSuspendContext, Lowerer, SuspensionContext, TypeId,
    properties::{PropertyAccessorKind, PropertyAccessorSource},
};

use super::{LocalDelegateAccessor, LocalDelegateDispatch, LocalDelegatePlan};

pub(crate) enum DelegateRoleCall {
    NoApplicable,
    Failed,
    Resolved(ResolvedDelegateRoleCall),
}

pub(crate) struct ResolvedDelegateRoleCall {
    pub(crate) expression: hir::Expr,
    pub(crate) effect: hir::Callable,
}

impl Lowerer {
    pub(crate) fn lower_local_delegated_property(
        &mut self,
        declaration: &ast::LocalDelegatedPropertyDecl,
        out: &mut Vec<hir::Statement>,
    ) {
        if self.scopes.is_declared_here(&declaration.name.text) {
            self.error(
                declaration.name.span,
                format!(
                    "`{}` is already declared in this scope",
                    declaration.name.text
                ),
            );
            return;
        }
        let annotation = match &declaration.ty {
            Some(ty) => match self.resolve_type_ref(ty) {
                Some(ty) => Some(ty),
                None => return,
            },
            None => None,
        };
        let mut sink = Vec::new();
        let Some(delegate) = self.lower_expr(&declaration.expression, &mut sink, None) else {
            return;
        };
        let effective = match self.resolve_delegate_role_call(
            delegate.clone(),
            hir::PropertyDelegateOperatorKind::ProvideDelegate,
            Vec::new(),
            declaration.span,
        ) {
            DelegateRoleCall::Resolved(resolved) => resolved.expression,
            DelegateRoleCall::NoApplicable => delegate,
            DelegateRoleCall::Failed => return,
        };
        let storage = self.alloc_hidden("delegate", effective.ty);
        let storage_binding = self.locals[storage].binding;
        let storage_read = hir::Expr {
            kind: hir::ExprKind::Local(storage),
            ty: effective.ty,
            span: declaration.span,
            origin: self.expression_origin(declaration.span),
        };

        let mut validation = self.clone();
        let Some(getter) = validation.require_delegate_role_resolution(
            storage_read.clone(),
            hir::PropertyDelegateOperatorKind::GetValue,
            vec![validation.unit_delegate_argument(declaration.span)],
            declaration.span,
        ) else {
            self.commit_layer_diagnostics(validation);
            return;
        };
        let property_ty = annotation.unwrap_or(getter.expression.ty);
        if !validation.validate_delegate_get_result(
            &declaration.name.text,
            getter.expression.ty,
            property_ty,
            declaration.span,
        ) {
            self.commit_layer_diagnostics(validation);
            return;
        }
        let getter = local_delegate_accessor(getter);
        let setter = if declaration.mutable {
            let property_value = hir::Expr {
                kind: hir::ExprKind::UnitLiteral,
                ty: property_ty,
                span: declaration.span,
                origin: validation.expression_origin(declaration.span),
            };
            let Some(setter) = validation.require_delegate_role_resolution(
                storage_read,
                hir::PropertyDelegateOperatorKind::SetValue,
                vec![
                    validation.unit_delegate_argument(declaration.span),
                    property_value,
                ],
                declaration.span,
            ) else {
                self.commit_layer_diagnostics(validation);
                return;
            };
            Some(local_delegate_accessor(setter))
        } else {
            None
        };

        *self = validation;
        self.local_delegate_plans.insert(
            storage_binding,
            LocalDelegatePlan {
                property_ty,
                mutable: declaration.mutable,
                getter,
                setter,
            },
        );
        self.scopes.declare(declaration.name.text.clone(), storage);
        out.extend(sink);
        out.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: storage },
                init: effective,
            },
            span: declaration.span,
        });
    }

    pub(crate) fn local_delegate_read(
        &mut self,
        storage: hir::Expr,
        binding: hir::BindingId,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        let plan = *self.local_delegate_plans.get(&binding)?;
        let unit = self.unit_delegate_argument(span);
        let value = self.materialize_local_delegate_call(plan.getter, storage, vec![unit], span);
        Some(self.adapt_to(value, plan.property_ty))
    }

    pub(crate) fn local_delegate_write(
        &mut self,
        storage: hir::Expr,
        binding: hir::BindingId,
        value: hir::Expr,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        let plan = *self.local_delegate_plans.get(&binding)?;
        let setter = plan.setter?;
        let unit = self.unit_delegate_argument(span);
        Some(self.materialize_local_delegate_call(setter, storage, vec![unit, value], span))
    }

    fn materialize_local_delegate_call(
        &mut self,
        accessor: LocalDelegateAccessor,
        receiver: hir::Expr,
        mut args: Vec<hir::Expr>,
        span: ast::Span,
    ) -> hir::Expr {
        self.check_call_effects(accessor.effect, span);
        let kind = match accessor.dispatch {
            LocalDelegateDispatch::Member(callee) => hir::ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee,
                args,
            },
            LocalDelegateDispatch::Extension(callee) => {
                args.insert(0, receiver);
                hir::ExprKind::Call { callee, args }
            }
        };
        hir::Expr {
            kind,
            ty: accessor.result_ty,
            span,
            origin: self.expression_origin(span),
        }
    }

    pub(super) fn lower_generated_delegate_accessor(
        &mut self,
        source: &PropertyAccessorSource,
    ) -> hir::Body {
        let mut declaration = source.declaration.clone();
        declaration.body = ast::FunctionBody::None;
        let mut body = self.lower_body(source.function, &declaration);

        let property = self.properties[source.property].clone();
        let hir::PropertyRepresentation::Delegated { storage } = property.representation else {
            return body;
        };
        let delegate = self.delegate_storages[storage].clone();
        let parameters = self.functions[source.function].params.clone();
        let this_ref = match property.owner {
            hir::PropertyOwner::TopLevel => self.unit_delegate_argument(property.span),
            hir::PropertyOwner::Extension(_)
            | hir::PropertyOwner::Class(_)
            | hir::PropertyOwner::Object(_) => {
                let Some(this_parameter) = parameters.first() else {
                    self.error(
                        property.span,
                        "a receiver delegated-property accessor requires an instance receiver"
                            .to_string(),
                    );
                    return body;
                };
                hir::Expr {
                    kind: hir::ExprKind::Local(this_parameter.local),
                    ty: this_parameter.ty,
                    span: property.span,
                    origin: self.expression_origin(property.span),
                }
            }
            hir::PropertyOwner::Struct(_)
            | hir::PropertyOwner::Enum(_)
            | hir::PropertyOwner::Interface(_) => {
                unreachable!("value types and interfaces cannot own delegated storage")
            }
        };
        let storage_read = match delegate.location {
            hir::DelegateStorageLocation::ClassField(field) => {
                let hir::Type::Class(application) = self.types[this_ref.ty] else {
                    self.error(
                        property.span,
                        "a class delegated-property accessor requires a class receiver".to_string(),
                    );
                    return body;
                };
                let application_value = self.class_applications[application].clone();
                let storage_ty = self.instantiate_ty(delegate.ty, &application_value.arguments);
                hir::Expr {
                    kind: hir::ExprKind::FieldAccess {
                        receiver: Box::new(this_ref.clone()),
                        field: hir::FieldRef::ClassField { application, field },
                    },
                    ty: storage_ty,
                    span: property.span,
                    origin: self.expression_origin(property.span),
                }
            }
            hir::DelegateStorageLocation::ManagedGlobal(global) => hir::Expr {
                kind: hir::ExprKind::GlobalRead(global),
                ty: delegate.ty,
                span: property.span,
                origin: self.expression_origin(property.span),
            },
        };

        let signature = self.signatures[&source.function].clone();
        let outer_type_params = std::mem::replace(
            &mut self.type_params_in_scope,
            signature.type_params.clone(),
        );
        let outer_return_ty = std::mem::replace(&mut self.current_return_ty, signature.return_ty);
        let accessor_name = self.functions[source.function].name.clone();
        let outer_fn_name = std::mem::replace(&mut self.current_fn_name, accessor_name);
        let outer_owner = std::mem::replace(&mut self.current_owner, source.owner);
        let outer_source_context = self.current_source_context;
        self.set_source_context(self.current_fn_name.clone());
        self.push_suspension_context(SuspensionContext::Forbidden(
            ForbiddenSuspendContext::Function,
        ));
        self.push_safety_context(self.functions[source.function].attributes.safety);

        let statements = match source.kind {
            PropertyAccessorKind::Getter => self
                .require_delegate_role_call(
                    storage_read,
                    hir::PropertyDelegateOperatorKind::GetValue,
                    vec![this_ref],
                    property.span,
                )
                .and_then(|value| {
                    self.validate_delegate_get_result(
                        &property.name,
                        value.ty,
                        signature.return_ty,
                        property.span,
                    )
                    .then(|| self.adapt_to(value, signature.return_ty))
                })
                .map_or_else(Vec::new, |value| {
                    if self.types_equal(signature.return_ty, self.unit) {
                        vec![
                            hir::Statement {
                                kind: hir::StatementKind::Expr(value),
                                span: property.span,
                            },
                            hir::Statement {
                                kind: hir::StatementKind::Return { value: None },
                                span: property.span,
                            },
                        ]
                    } else {
                        vec![hir::Statement {
                            kind: hir::StatementKind::Return { value: Some(value) },
                            span: property.span,
                        }]
                    }
                }),
            PropertyAccessorKind::Setter => {
                match if matches!(property.owner, hir::PropertyOwner::TopLevel) {
                    parameters.first()
                } else {
                    parameters.get(1)
                } {
                    Some(value_parameter) => {
                        let value = hir::Expr {
                            kind: hir::ExprKind::Local(value_parameter.local),
                            ty: value_parameter.ty,
                            span: property.span,
                            origin: self.expression_origin(property.span),
                        };
                        self.require_delegate_role_call(
                            storage_read,
                            hir::PropertyDelegateOperatorKind::SetValue,
                            vec![this_ref, value],
                            property.span,
                        )
                        .map_or_else(Vec::new, |call| {
                            vec![
                                hir::Statement {
                                    kind: hir::StatementKind::Expr(call),
                                    span: property.span,
                                },
                                hir::Statement {
                                    kind: hir::StatementKind::Return { value: None },
                                    span: property.span,
                                },
                            ]
                        })
                    }
                    None => {
                        self.error(
                            property.span,
                            "a mutable delegated-property accessor requires a value parameter"
                                .to_string(),
                        );
                        Vec::new()
                    }
                }
            }
        };

        self.pop_safety_context();
        self.pop_suspension_context();
        self.current_source_context = outer_source_context;
        self.current_owner = outer_owner;
        self.current_fn_name = outer_fn_name;
        self.current_return_ty = outer_return_ty;
        self.type_params_in_scope = outer_type_params;
        body.statements = statements;
        body
    }

    pub(crate) fn resolve_delegate_role_call(
        &mut self,
        receiver: hir::Expr,
        role: hir::PropertyDelegateOperatorKind,
        args: Vec<hir::Expr>,
        span: ast::Span,
    ) -> DelegateRoleCall {
        let name = delegate_role_name(role);
        let mut members = self.methods_by_name(receiver.ty, name);
        members.retain(|candidate| {
            self.signatures[&candidate.function]
                .modifiers
                .property_delegate_operator
                == Some(role)
        });
        if !members.is_empty() {
            let mut state = self.clone();
            let mut sink = Vec::new();
            match state.resolve_member_overload_lowered_outcome(
                name,
                &members,
                lowered_call(args.clone(), span),
                &mut sink,
            ) {
                OverloadResolutionOutcome::Resolved(resolved) => {
                    debug_assert!(sink.is_empty());
                    let callable = state.materialize_resolved_callee(&resolved);
                    state.check_call_effects(callable, span);
                    let callee = state.materialize_method_callee(
                        resolved.source,
                        callable,
                        &resolved.type_args,
                    );
                    let expression = hir::Expr {
                        kind: hir::ExprKind::MethodCall {
                            receiver: Box::new(receiver.clone()),
                            callee,
                            args: resolved.args,
                        },
                        ty: resolved.return_ty,
                        span,
                        origin: state.expression_origin(span),
                    };
                    *self = state;
                    return DelegateRoleCall::Resolved(ResolvedDelegateRoleCall {
                        expression,
                        effect: callable,
                    });
                }
                OverloadResolutionOutcome::Failed => {
                    self.commit_layer_diagnostics(state);
                    return DelegateRoleCall::Failed;
                }
                OverloadResolutionOutcome::NoApplicable => {}
            }
        }

        for extensions in self.extension_candidate_layers(name) {
            let extensions = extensions
                .into_iter()
                .filter(|function| {
                    self.signatures[function]
                        .modifiers
                        .property_delegate_operator
                        == Some(role)
                })
                .collect::<Vec<_>>();
            if extensions.is_empty() {
                continue;
            }
            let mut state = self.clone();
            let mut sink = Vec::new();
            match state.resolve_extension_overload_lowered_outcome(
                name,
                &extensions,
                receiver.clone(),
                lowered_call(args.clone(), span),
                &mut sink,
            ) {
                OverloadResolutionOutcome::Resolved(resolved) => {
                    debug_assert!(sink.is_empty());
                    let callable = state.materialize_resolved_callee(&resolved);
                    state.check_call_effects(callable, span);
                    let expression = hir::Expr {
                        kind: hir::ExprKind::Call {
                            callee: callable,
                            args: resolved.args,
                        },
                        ty: resolved.return_ty,
                        span,
                        origin: state.expression_origin(span),
                    };
                    *self = state;
                    return DelegateRoleCall::Resolved(ResolvedDelegateRoleCall {
                        expression,
                        effect: callable,
                    });
                }
                OverloadResolutionOutcome::Failed => {
                    self.commit_layer_diagnostics(state);
                    return DelegateRoleCall::Failed;
                }
                OverloadResolutionOutcome::NoApplicable => {}
            }
        }
        DelegateRoleCall::NoApplicable
    }

    pub(crate) fn require_delegate_role_call(
        &mut self,
        receiver: hir::Expr,
        role: hir::PropertyDelegateOperatorKind,
        args: Vec<hir::Expr>,
        span: ast::Span,
    ) -> Option<hir::Expr> {
        self.require_delegate_role_resolution(receiver, role, args, span)
            .map(|resolved| resolved.expression)
    }

    fn require_delegate_role_resolution(
        &mut self,
        receiver: hir::Expr,
        role: hir::PropertyDelegateOperatorKind,
        args: Vec<hir::Expr>,
        span: ast::Span,
    ) -> Option<ResolvedDelegateRoleCall> {
        let receiver_ty = receiver.ty;
        match self.resolve_delegate_role_call(receiver, role, args, span) {
            DelegateRoleCall::Resolved(resolved) => Some(resolved),
            DelegateRoleCall::Failed => None,
            DelegateRoleCall::NoApplicable => {
                let found = self.type_name(receiver_ty);
                self.error(
                    span,
                    format!(
                        "type `{found}` has no applicable property delegate operator `{}`",
                        delegate_role_name(role)
                    ),
                );
                None
            }
        }
    }

    pub(crate) fn unit_delegate_argument(&self, span: ast::Span) -> hir::Expr {
        hir::Expr {
            kind: hir::ExprKind::UnitLiteral,
            ty: self.unit,
            span,
            origin: self.expression_origin(span),
        }
    }

    pub(crate) fn validate_delegate_get_result(
        &mut self,
        property_name: &str,
        result: TypeId,
        expected: TypeId,
        span: ast::Span,
    ) -> bool {
        if self.is_subtype(result, expected) {
            return true;
        }
        self.error(
            span,
            format!(
                "property delegate getter for `{property_name}` returns {}, which is not assignable to {}",
                self.type_name(result),
                self.type_name(expected)
            ),
        );
        false
    }
}

fn local_delegate_accessor(resolved: ResolvedDelegateRoleCall) -> LocalDelegateAccessor {
    let result_ty = resolved.expression.ty;
    let dispatch = match resolved.expression.kind {
        hir::ExprKind::MethodCall { callee, .. } => LocalDelegateDispatch::Member(callee),
        hir::ExprKind::Call { callee, .. } => LocalDelegateDispatch::Extension(callee),
        _ => unreachable!("delegate role resolution produces a direct or member call"),
    };
    LocalDelegateAccessor {
        dispatch,
        effect: resolved.effect,
        result_ty,
    }
}

fn lowered_call(args: Vec<hir::Expr>, span: ast::Span) -> LoweredOverloadCall {
    LoweredOverloadCall {
        explicit_type_args: Vec::new(),
        args,
        span,
        expected_result: None,
    }
}

fn delegate_role_name(role: hir::PropertyDelegateOperatorKind) -> &'static str {
    match role {
        hir::PropertyDelegateOperatorKind::ProvideDelegate => "provideDelegate",
        hir::PropertyDelegateOperatorKind::GetValue => "getValue",
        hir::PropertyDelegateOperatorKind::SetValue => "setValue",
    }
}
