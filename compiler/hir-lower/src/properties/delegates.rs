use scoop_ast as ast;
use scoop_hir as hir;

use crate::{
    ForbiddenSuspendContext, Lowerer, SuspensionContext, TypeId,
    properties::{PropertyAccessorKind, PropertyAccessorSource},
};

use super::{DelegateCallEffect, LocalDelegateAccessor, LocalDelegateDispatch, LocalDelegatePlan};

mod accessor;

pub(crate) enum DelegateRoleCall {
    NoApplicable,
    Failed,
    Resolved(ResolvedDelegateRoleCall),
}

pub(crate) struct ResolvedDelegateRoleCall {
    pub(crate) expression: hir::Expr,
    pub(crate) effect: DelegateCallEffect,
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
        let plan = self.local_delegate_plans.get(&binding)?.clone();
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
        let plan = self.local_delegate_plans.get(&binding)?.clone();
        let setter = plan.setter?;
        let unit = self.unit_delegate_argument(span);
        Some(self.materialize_local_delegate_call(setter, storage, vec![unit, value], span))
    }

    fn materialize_local_delegate_call(
        &mut self,
        accessor: LocalDelegateAccessor,
        receiver: hir::Expr,
        args: Vec<hir::Expr>,
        span: ast::Span,
    ) -> hir::Expr {
        match accessor.effect {
            DelegateCallEffect::Current(callable) => self.check_call_effects(callable, span),
            DelegateCallEffect::Imported(hir::CallableSafetyV1::Unsafe) => {
                self.require_unsafe_operation(span, "calling an unsafe dependency function");
            }
            DelegateCallEffect::Imported(hir::CallableSafetyV1::Safe) => {}
        }
        let receiver = self.adapt_to(receiver, accessor.receiver_ty);
        assert_eq!(args.len(), accessor.parameter_types.len());
        let mut args = args
            .into_iter()
            .zip(accessor.parameter_types)
            .map(|(value, ty)| self.adapt_to(value, ty))
            .collect::<Vec<_>>();
        let kind = match accessor.dispatch {
            LocalDelegateDispatch::Member(callee) => hir::ExprKind::MethodCall {
                receiver: Box::new(receiver),
                callee,
                args,
            },
            LocalDelegateDispatch::Call {
                callee,
                binding,
                receiver: source_receiver,
            } => {
                args.insert(0, receiver);
                hir::ExprKind::Call {
                    callee,
                    binding,
                    args,
                    receiver: source_receiver,
                }
            }
            LocalDelegateDispatch::ImportedGeneric {
                application,
                kind,
                binding,
                receiver: source_receiver,
            } => {
                args.insert(0, receiver);
                hir::ExprKind::ImportedGenericCall {
                    application,
                    kind,
                    binding,
                    args,
                    receiver: source_receiver,
                }
            }
        };
        hir::Expr {
            kind,
            ty: accessor.result_ty,
            span,
            origin: self.expression_origin(span),
        }
    }

    pub(crate) fn resolve_delegate_role_call(
        &mut self,
        receiver: hir::Expr,
        role: hir::PropertyDelegateOperatorKind,
        args: Vec<hir::Expr>,
        span: ast::Span,
    ) -> DelegateRoleCall {
        let name = ast::Ident {
            text: delegate_role_name(role).to_owned(),
            span,
        };
        match self.resolve_delegate_member_role(receiver.clone(), role, &name, &args) {
            DelegateRoleCall::NoApplicable => {}
            result => return result,
        }
        for layer in self.named_extension_delegate_operator_layers(role) {
            match self.resolve_delegate_extension_role(
                &layer.candidates,
                receiver.clone(),
                &name,
                &args,
            ) {
                DelegateRoleCall::NoApplicable => {}
                result => return result,
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
    let (dispatch, receiver_ty, args) = match resolved.expression.kind {
        hir::ExprKind::MethodCall {
            callee,
            receiver,
            args,
        } => (LocalDelegateDispatch::Member(callee), receiver.ty, args),
        hir::ExprKind::Call {
            callee,
            binding,
            receiver,
            mut args,
        } => {
            let value = args.remove(0);
            (
                LocalDelegateDispatch::Call {
                    callee,
                    binding,
                    receiver,
                },
                value.ty,
                args,
            )
        }
        hir::ExprKind::ImportedGenericCall {
            application,
            kind,
            binding,
            receiver,
            mut args,
        } => {
            let value = args.remove(0);
            (
                LocalDelegateDispatch::ImportedGeneric {
                    application,
                    kind,
                    binding,
                    receiver,
                },
                value.ty,
                args,
            )
        }
        _ => unreachable!("delegate role resolution produces a direct or member call"),
    };
    LocalDelegateAccessor {
        dispatch,
        effect: resolved.effect,
        receiver_ty,
        parameter_types: args.into_iter().map(|argument| argument.ty).collect(),
        result_ty,
    }
}

fn delegate_role_name(role: hir::PropertyDelegateOperatorKind) -> &'static str {
    match role {
        hir::PropertyDelegateOperatorKind::ProvideDelegate => "provideDelegate",
        hir::PropertyDelegateOperatorKind::GetValue => "getValue",
        hir::PropertyDelegateOperatorKind::SetValue => "setValue",
    }
}
