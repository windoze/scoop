use super::*;

mod delegate_roles;
mod equality;
pub(crate) use equality::ImportedDerivedEqualityProbe;
mod explicit_calls;
mod extension_calls;
mod extensions;
mod imported_calls;
pub(in crate::expr) use imported_calls::ImportedMemberSelectionFailure;
mod interface_super;
mod pointers;
mod primitives;
mod property_invoke;
mod qualifiers;
pub(in crate::expr) use qualifiers::CurrentQualifiedCall;
mod receiver_calls;
mod resolution;
mod selection;

pub(in crate::expr) use property_invoke::{
    PropertyExtensionInvokeInput, PropertyExtensionInvokeOrigin, PropertyExtensionInvokeOutcome,
};

impl Lowerer {
    /// Resolve `super.name(...)` from the exact direct-base application. No
    /// extension, property-like, or interface layer participates, and the
    /// resulting HIR target retains direct dispatch.
    pub(super) fn lower_super_method_call(
        &mut self,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let receiver = self.resolve_super_call_receiver(name, call.span)?;
        let base = receiver.ty;
        let candidates = self.super_methods_by_name(base, &name.text);
        match self.probe_member_call_partition_with_kind(
            candidates,
            name,
            receiver,
            call,
            expected,
            RequiredCallableModifiers::default(),
            MemberCallKind::DirectSuper,
        ) {
            PropertyExtensionInvokeOutcome::Resolved(success) => {
                *self = *success.state;
                sink.extend(success.sink);
                Some(success.expression)
            }
            PropertyExtensionInvokeOutcome::Failed(failure)
            | PropertyExtensionInvokeOutcome::NoApplicable(Some(failure)) => {
                *self = *failure;
                None
            }
            PropertyExtensionInvokeOutcome::Blocked
            | PropertyExtensionInvokeOutcome::NoApplicable(None) => {
                self.error(
                    name.span,
                    format!(
                        "base class `{}` has no method `{}`",
                        self.type_name(base),
                        name.text
                    ),
                );
                None
            }
        }
    }

    pub(crate) fn resolve_super_call_receiver(
        &mut self,
        name: &ast::Ident,
        span: Span,
    ) -> Option<hir::Expr> {
        let Some(receiver) = self.lower_current_this(span) else {
            self.error(
                span,
                "`super` method calls are only allowed inside class member functions".into(),
            );
            return None;
        };
        let Type::Class(application) = self.types[receiver.ty] else {
            self.error(span, "`super` method calls require a class receiver".into());
            return None;
        };
        let current = self.class_applications[application].clone();
        let Some(base) = self.classes[self.class_id(current.template)].base_class else {
            self.error(
                span,
                format!(
                    "class `{}` has no direct base for `super.{}`",
                    self.classes[self.class_id(current.template)].name,
                    name.text
                ),
            );
            return None;
        };
        let base = self.instantiate_ty(base, &current.arguments);
        Some(self.adapt_to(receiver, base))
    }

    /// Read the ordinary receiver binding of a member or constructor body.
    pub(super) fn lower_this(
        &mut self,
        label: Option<&ast::Ident>,
        span: Span,
    ) -> Option<hir::Expr> {
        if let Some(label) = label {
            return self.lower_labelled_this(label, span);
        }
        let Some(this) = self.lower_current_this(span) else {
            self.error(
                span,
                "no `this` receiver is available in this context".to_string(),
            );
            return None;
        };
        Some(this)
    }

    /// `receiver.name(args)` (M6/M7): the method overloads are
    /// collected from the receiver's static type — class members (base
    /// chain included), interface methods, or struct / enum methods — and
    /// resolved by the unified M16 algorithm. If that layer has no applicable
    /// candidate, visible extensions are probed in import priority order.
    /// Single and multiple candidates use the same entry.
    /// The dispatch kind (direct / virtual / interface) is decided at
    /// MIR from the receiver's static type (hir docs).
    ///
    /// One receiver shape is not a method call: the M6 parser folds a
    /// qualified enum variant construction `E.V(args)` into this
    /// syntax (`MethodCall { receiver: Var("E"), ... }`). When the
    /// receiver is a bare name that is no in-scope variable and no
    /// property of the current host — but names an enum — it is a
    /// variant path and goes through variant construction (M4 rules:
    /// variant existence, per-field argument checks, type-argument
    /// inference, constructor-style defaults). Variables and host
    /// properties shadow enum names. Core array conversion methods enter the
    /// ordinary member candidate layer and are normalized only after their
    /// typed intrinsic target wins (spec 10.4).
    pub(super) fn lower_method_call(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if let Some(parameter) = self.companion_parameter_qualifier(receiver) {
            let receiver = self.lower_parameter_companion(parameter, receiver.span())?;
            return self.lower_explicit_named_call(
                receiver,
                name,
                call,
                sink,
                expected,
                RequiredCallableModifiers::default(),
            );
        }
        if let Some((alias, index)) = self.resolve_alias_variant(receiver, name).ok()? {
            return self.lower_alias_variant_construct(&alias, index, name, call, sink, expected);
        }
        if let Some(alias) = self.nested_constructor_alias(receiver, name).ok()? {
            return self.lower_alias_construct(alias, name, call, sink, expected);
        }
        if let Some(owner) = self.resolve_imported_nominal_qualifier(receiver).ok()? {
            return self.lower_imported_qualified_call(owner, name, call, sink, expected);
        }
        if let Some(target) = self
            .resolve_current_qualified_call(receiver, name, call.type_args, expected)
            .ok()?
        {
            return match target {
                CurrentQualifiedCall::Struct { id, ty } => {
                    self.lower_struct_init(id, ty, call, sink, expected)
                }
                CurrentQualifiedCall::Class(id) => {
                    self.lower_class_construct(id, call, sink, expected)
                }
                CurrentQualifiedCall::Variant { target, expected } => {
                    self.lower_variant_construct(target, call, sink, expected)
                }
                CurrentQualifiedCall::Singleton(receiver) => self.lower_explicit_named_call(
                    receiver,
                    name,
                    call,
                    sink,
                    expected,
                    RequiredCallableModifiers::default(),
                ),
            };
        }
        if let Some(layer) = self.probe_numeric_literal_receiver(
            receiver,
            expected,
            |state, receiver, layer_sink| {
                state.lower_explicit_named_call(
                    receiver,
                    name,
                    call,
                    layer_sink,
                    expected,
                    RequiredCallableModifiers::default(),
                )
            },
        ) {
            return Some(self.commit_expr_layer(layer, sink));
        }
        let receiver = self.lower_expr(receiver, sink, None)?;
        self.lower_explicit_named_call(
            receiver,
            name,
            call,
            sink,
            expected,
            RequiredCallableModifiers::default(),
        )
    }

    pub(crate) fn matches_required_modifiers(
        modifiers: hir::CallableModifiers,
        required: RequiredCallableModifiers,
    ) -> bool {
        required
            .operator
            .is_none_or(|operator| modifiers.operator == Some(operator))
            && required
                .property_delegate_operator
                .is_none_or(|operator| modifiers.property_delegate_operator == Some(operator))
            && (!required.infix || modifiers.is_infix)
    }

    #[allow(clippy::too_many_arguments)]
    fn probe_local_member_call_partition(
        &self,
        candidates: Vec<crate::CallableCandidate>,
        name: &ast::Ident,
        receiver: hir::Expr,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        operator_set: bool,
    ) -> PropertyExtensionInvokeOutcome {
        use crate::overload::{CallArgumentProtocol, OverloadCall, OverloadResolutionOutcome};

        if candidates.is_empty() {
            return PropertyExtensionInvokeOutcome::NoApplicable(None);
        }
        let mut state = self.probe_state();
        let Some(explicit_type_args) = state.resolve_call_type_args(call.type_args) else {
            return PropertyExtensionInvokeOutcome::NoApplicable(Some(state));
        };
        let mut layer_sink = Vec::new();
        match state.resolve_member_overload_outcome(
            &name.text,
            &candidates,
            receiver,
            OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: call.args,
                span: call.span,
                expected_result: expected,
                argument_protocol: if operator_set {
                    CallArgumentProtocol::OperatorSet
                } else {
                    CallArgumentProtocol::Ordinary
                },
            },
            &mut layer_sink,
        ) {
            OverloadResolutionOutcome::NoApplicable => {
                PropertyExtensionInvokeOutcome::NoApplicable(Some(state))
            }
            OverloadResolutionOutcome::Blocked => PropertyExtensionInvokeOutcome::Blocked,
            OverloadResolutionOutcome::Failed => PropertyExtensionInvokeOutcome::Failed(state),
            OverloadResolutionOutcome::Resolved(resolved) => {
                state.observe_callable_reference(
                    name.span,
                    hir::SemanticCallable::Current(resolved.function()),
                );
                let Some(expression) =
                    state.finish_resolved_method_call(*resolved, call.span, &mut layer_sink)
                else {
                    return PropertyExtensionInvokeOutcome::Failed(state);
                };
                PropertyExtensionInvokeOutcome::Resolved(SuccessfulExprLayer {
                    state,
                    expression,
                    sink: layer_sink,
                })
            }
        }
    }

    pub(crate) fn member_property_read(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
    ) -> Option<hir::Expr> {
        self.read_member_property_views(receiver, name, name.span, None)
            .ok()
            .flatten()
    }

    pub(crate) fn prepare_safe_call_receiver(
        &mut self,
        receiver: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<(hir::Expr, hir::Expr)> {
        let Some(inner) = self.as_option(receiver.ty) else {
            let found = self.type_name(receiver.ty);
            self.error(
                span,
                format!("`?.` requires an Option receiver, found {found}"),
            );
            return None;
        };
        let option_ty = receiver.ty;
        let origin = self.expression_origin(span);
        let receiver_local = self.alloc_hidden("opt", option_ty);
        sink.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding {
                    local: receiver_local,
                },
                init: receiver,
            },
            span,
        });
        let receiver_ref = hir::Expr {
            kind: ExprKind::Local(receiver_local),
            ty: option_ty,
            span,
            origin,
        };
        let payload = hir::Expr {
            kind: ExprKind::Unwrap {
                operand: Box::new(receiver_ref.clone()),
                trap_on_none: false,
            },
            ty: inner,
            span,
            origin,
        };
        Some((receiver_ref, payload))
    }

    pub(super) fn lower_safe_method_call(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let receiver = self.lower_expr(receiver, sink, None)?;
        let (receiver_ref, payload) = self.prepare_safe_call_receiver(receiver, call.span, sink)?;
        let origin = self.expression_origin(call.span);
        let mut then_body = Vec::new();
        let value = self.lower_explicit_named_call(
            payload,
            name,
            call,
            &mut then_body,
            expected.and_then(|ty| self.as_option(ty)),
            RequiredCallableModifiers::default(),
        )?;
        let result_ty = self.option_type(value.ty);
        let result = self.alloc_hidden("res", result_ty);
        then_body.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: hir::Expr {
                    kind: ExprKind::SomeWrap(Box::new(value)),
                    ty: result_ty,
                    span: call.span,
                    origin,
                },
            },
            span: call.span,
        });
        let else_body = vec![hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding { local: result },
                init: hir::Expr {
                    kind: ExprKind::NoneLiteral,
                    ty: result_ty,
                    span: call.span,
                    origin,
                },
            },
            span: call.span,
        }];
        sink.push(hir::Statement {
            kind: hir::StatementKind::If {
                cond: hir::Expr {
                    kind: ExprKind::IsSome(Box::new(receiver_ref)),
                    ty: self.boolean,
                    span: call.span,
                    origin,
                },
                then_body,
                else_body: Some(else_body),
            },
            span: call.span,
        });
        Some(hir::Expr {
            kind: ExprKind::Local(result),
            ty: result_ty,
            span: call.span,
            origin,
        })
    }
}
