use super::*;
use crate::NominalTarget;

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
mod receiver_calls;
mod resolution;
mod selection;

pub(in crate::expr) use property_invoke::{
    PropertyExtensionInvokeInput, PropertyExtensionInvokeOrigin, PropertyExtensionInvokeOutcome,
};

impl Lowerer {
    /// Resolve `super.name(...)` from the exact direct-base application. No
    /// extension, property-like, or interface layer participates, and the
    /// resulting HIR variant preserves the mandatory direct-dispatch proof.
    pub(super) fn lower_super_method_call(
        &mut self,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if self.initialization_context.is_some() {
            self.error(
                call.span,
                "initializing receiver cannot escape before construction completes".into(),
            );
            return None;
        }
        let Some(mut receiver) = self.lower_current_this(call.span) else {
            self.error(
                call.span,
                "`super` method calls are only allowed inside class member functions".into(),
            );
            return None;
        };
        let Type::Class(application) = self.types[receiver.ty] else {
            self.error(
                call.span,
                "`super` method calls require a class receiver".into(),
            );
            return None;
        };
        let current = self.class_applications[application].clone();
        let Some(base) = self.classes[current.template].base_class else {
            self.error(
                call.span,
                format!(
                    "class `{}` has no direct base for `super.{}`",
                    self.classes[current.template].name, name.text
                ),
            );
            return None;
        };
        let base = self.instantiate_ty(base, &current.arguments);
        let Type::Class(base_application) = self.types[base] else {
            unreachable!("a class direct base is a class application")
        };
        receiver.ty = base;
        let candidates = self.methods_by_name(base, &name.text);
        if candidates.is_empty() {
            let base_name = self.classes[self.class_applications[base_application].template]
                .name
                .clone();
            self.error(
                name.span,
                format!("base class `{base_name}` has no method `{}`", name.text),
            );
            return None;
        }
        self.finish_super_method_call(candidates, &name.text, receiver, call, sink, expected)
    }

    /// `this` (M6): only inside member functions, where it is
    /// parameter 0 (`lower_body` registers it as a local).
    pub(super) fn lower_this(&mut self, span: Span) -> Option<hir::Expr> {
        if self.reject_initializing_this(span) {
            return None;
        }
        let Some(this) = self.lower_current_this(span) else {
            self.error(
                span,
                "`this` is only allowed inside member functions".to_string(),
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
        if matches!(receiver, ast::Expr::This { .. }) && self.initialization_context.is_some() {
            if !self.initializing_receiver_has_field(&name.text) {
                self.error(
                    call.span,
                    "initializing receiver cannot escape before construction completes".into(),
                );
                return None;
            }
            let property = self.initializing_field(name, call.span)?;
            return match self.probe_property_member_invoke_partition(
                property.read,
                call,
                expected,
                false,
            ) {
                PropertyExtensionInvokeOutcome::Resolved(layer) => {
                    Some(self.commit_expr_layer(layer, sink))
                }
                PropertyExtensionInvokeOutcome::Blocked => None,
                PropertyExtensionInvokeOutcome::Failed(failure)
                | PropertyExtensionInvokeOutcome::NoApplicable(Some(failure)) => {
                    self.commit_layer_diagnostics(*failure);
                    None
                }
                PropertyExtensionInvokeOutcome::NoApplicable(None) => {
                    self.error(
                        call.span,
                        "initializing receiver cannot escape before construction completes".into(),
                    );
                    None
                }
            };
        }
        if let Some(owner) = self.resolve_imported_nominal_qualifier(receiver).ok()? {
            return self.lower_imported_qualified_call(owner, name, call, sink, expected);
        }
        let direct_alias = match self.resolve_direct_alias_qualifier(receiver) {
            Ok(alias) => alias,
            Err(()) => return None,
        };
        let qualifier = direct_alias
            .as_ref()
            .map(|(_, target)| *target)
            .or_else(|| self.nominal_qualifier_target(receiver));
        if let Some(qualifier) = qualifier {
            if let Some(target) = self.nested_nominal_target(qualifier.owner(), &name.text) {
                return self.lower_static_nested_constructor(target, name, call, sink, expected);
            }
            if let NominalTarget::Enum(enum_id) = qualifier {
                if let Some(target) = self.find_variant_ref(enum_id, &name.text) {
                    let expected = self.alias_fixed_expected(
                        direct_alias.as_ref().map(|(alias, _)| alias),
                        call.type_args,
                        expected,
                    )?;
                    return self.lower_variant_construct(target, call, sink, expected);
                }
            }
            let forwarded = self.companion_forwarding_object(qualifier, &name.text);
            if let NominalTarget::Object(object) = qualifier
                && forwarded.is_none()
            {
                let receiver = self.lower_singleton_value(object, receiver.span())?;
                return self.lower_explicit_named_call(
                    receiver,
                    name,
                    call,
                    sink,
                    expected,
                    RequiredCallableModifiers::default(),
                );
            }
            if let Some(companion) = forwarded {
                let receiver = self.lower_singleton_value(companion, receiver.span())?;
                return self.lower_explicit_named_call(
                    receiver,
                    name,
                    call,
                    sink,
                    expected,
                    RequiredCallableModifiers::default(),
                );
            }
            if let NominalTarget::Enum(_) = qualifier {
                self.error(
                    name.span,
                    format!(
                        "enum `{}` has no variant `{}`",
                        qualifier.owner().describe_name(self),
                        name.text
                    ),
                );
                return None;
            }
            self.error(
                name.span,
                format!(
                    "type `{}` has no nested type `{}`",
                    qualifier.owner().describe_name(self),
                    name.text
                ),
            );
            return None;
        }
        if let Some(layer) = self.probe_integer_literal_receiver(
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

    fn matches_required_modifiers(
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
        name: &str,
        receiver: hir::Expr,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        operator_set: bool,
    ) -> PropertyExtensionInvokeOutcome {
        use crate::overload::{CallArgumentProtocol, OverloadCall, OverloadResolutionOutcome};

        if candidates.is_empty() {
            return PropertyExtensionInvokeOutcome::NoApplicable(None);
        }
        let mut state = self.clone();
        let Some(explicit_type_args) = state.resolve_call_type_args(call.type_args) else {
            return PropertyExtensionInvokeOutcome::NoApplicable(Some(Box::new(state)));
        };
        let mut layer_sink = Vec::new();
        match state.resolve_member_overload_outcome(
            name,
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
                PropertyExtensionInvokeOutcome::NoApplicable(Some(Box::new(state)))
            }
            OverloadResolutionOutcome::Blocked => PropertyExtensionInvokeOutcome::Blocked,
            OverloadResolutionOutcome::Failed => {
                PropertyExtensionInvokeOutcome::Failed(Box::new(state))
            }
            OverloadResolutionOutcome::Resolved(resolved) => {
                let expression = state
                    .finish_resolved_method_call(*resolved, call.span)
                    .expect("a resolved member call always materializes an expression");
                PropertyExtensionInvokeOutcome::Resolved(SuccessfulExprLayer {
                    state: Box::new(state),
                    expression,
                    sink: layer_sink,
                })
            }
        }
    }

    pub(in crate::expr) fn member_property_read(
        &mut self,
        receiver: hir::Expr,
        name: &ast::Ident,
    ) -> Option<hir::Expr> {
        let receiver_ty = receiver.ty;
        if let Some((property, owner, ty)) =
            self.find_accessible_nominal_property(receiver_ty, &name.text)
        {
            return self.lower_property_read(property, Some(owner), Some(receiver), ty, name.span);
        }
        let property = self
            .resolve_imported_member_property(receiver_ty, name)
            .ok()??;
        self.emit_imported_member_property_read(&property, receiver, name.span)
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
        let Some(inner) = self.as_option(receiver.ty) else {
            let found = self.type_name(receiver.ty);
            self.error(
                call.span,
                format!("`?.` requires an Option receiver, found {found}"),
            );
            return None;
        };
        let option_ty = receiver.ty;
        let origin = self.expression_origin(call.span);
        let receiver_local = self.alloc_hidden("opt", option_ty);
        sink.push(hir::Statement {
            kind: hir::StatementKind::ValDecl {
                pattern: hir::Pattern::Binding {
                    local: receiver_local,
                },
                init: receiver,
            },
            span: call.span,
        });
        let receiver_ref = hir::Expr {
            kind: ExprKind::Local(receiver_local),
            ty: option_ty,
            span: call.span,
            origin,
        };
        let payload = hir::Expr {
            kind: ExprKind::Unwrap {
                operand: Box::new(receiver_ref.clone()),
                trap_on_none: false,
            },
            ty: inner,
            span: call.span,
            origin,
        };
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
