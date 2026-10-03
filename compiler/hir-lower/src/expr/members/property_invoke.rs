use super::*;
use crate::call_resolution::named::NamedFunctionLikeProbe;
use crate::imports::lookup::calls::{ExtensionCallTarget, ExtensionPropertyIdentity};
use crate::imports::lookup::values::ValueTarget;
use crate::overload::{CallArgumentProtocol, NamedCallReceiver, OverloadCall};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::expr) enum PropertyExtensionInvokeOrigin {
    Member(hir::PropertyId),
    NamedValue(ValueTarget),
    DependencyValue(hir::ImportedTarget),
    Extension(ExtensionPropertyIdentity),
}

pub(in crate::expr) struct PropertyExtensionInvokeInput {
    origin: PropertyExtensionInvokeOrigin,
    property: SuccessfulExprLayer,
    candidates: Vec<ExtensionCallTarget>,
}

impl PropertyExtensionInvokeInput {
    pub(in crate::expr) fn new(
        origin: PropertyExtensionInvokeOrigin,
        property: SuccessfulExprLayer,
        candidates: Vec<ExtensionCallTarget>,
    ) -> Self {
        Self {
            origin,
            property,
            candidates,
        }
    }
}

pub(in crate::expr) enum PropertyExtensionInvokeOutcome {
    NoApplicable(Option<Box<Lowerer>>),
    Blocked,
    Failed(Box<Lowerer>),
    Resolved(SuccessfulExprLayer),
}

impl Lowerer {
    pub(in crate::expr) fn probe_property_member_invoke_partition(
        &self,
        property: hir::Expr,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        require_infix: bool,
    ) -> PropertyExtensionInvokeOutcome {
        if matches!(self.types[property.ty], Type::Function(_)) {
            if require_infix {
                return PropertyExtensionInvokeOutcome::NoApplicable(None);
            }
            return match self.probe_expr_layer(|state, layer_sink| {
                state.lower_named_call_on_receiver(
                    property,
                    &ast::Ident {
                        text: "invoke".to_string(),
                        span: call.span,
                    },
                    call,
                    layer_sink,
                    expected,
                    RequiredCallableModifiers::default(),
                )
            }) {
                Ok(layer) => PropertyExtensionInvokeOutcome::Resolved(layer),
                Err(failure) => PropertyExtensionInvokeOutcome::NoApplicable(Some(failure)),
            };
        }
        let mut candidate_state = self.clone();
        let mut candidates = candidate_state.methods_by_name(property.ty, "invoke");
        candidates.retain(|candidate| {
            Self::matches_required_modifiers(
                candidate_state.signatures[&candidate.function].modifiers,
                RequiredCallableModifiers {
                    operator: Some(hir::OperatorKind::Invoke),
                    infix: require_infix,
                    ..Default::default()
                },
            )
        });
        candidate_state.probe_member_call_partition(
            candidates,
            &ast::Ident {
                text: "invoke".into(),
                span: call.span,
            },
            property,
            call,
            expected,
            RequiredCallableModifiers {
                operator: Some(hir::OperatorKind::Invoke),
                infix: require_infix,
                ..Default::default()
            },
        )
    }

    pub(in crate::expr) fn probe_property_extension_invoke_partition(
        &self,
        inputs: Vec<PropertyExtensionInvokeInput>,
        call: CallSite<'_>,
        expected: Option<TypeId>,
        layer_name: &str,
    ) -> PropertyExtensionInvokeOutcome {
        let mut probes = Vec::new();
        let mut setups = Vec::new();
        let mut first_failure = None;
        let mut seen = Vec::new();
        let mut suppressed = false;
        for input in inputs {
            for target in input.candidates {
                if seen.contains(&(input.origin, target.clone())) {
                    continue;
                }
                seen.push((input.origin, target.clone()));
                match target {
                    ExtensionCallTarget::Current(function) => {
                        if self.declaration_surface.rejects_function(function) {
                            suppressed = true;
                            continue;
                        }
                        let mut state = (*input.property.state).clone();
                        let Some(explicit_type_args) = state.resolve_call_type_args(call.type_args)
                        else {
                            first_failure.get_or_insert(Box::new(state));
                            continue;
                        };
                        let target = crate::CallableCandidate::function(function, Vec::new());
                        match state.probe_named_callable(
                            "invoke",
                            target,
                            NamedCallReceiver::Extension(input.property.expression.clone()),
                            OverloadCall {
                                explicit_type_args: &explicit_type_args,
                                arg_exprs: call.args,
                                span: call.span,
                                expected_result: expected,
                                argument_protocol: CallArgumentProtocol::Ordinary,
                            },
                        ) {
                            Ok(probe) => {
                                probes.push(NamedFunctionLikeProbe::Callable(Box::new(probe)));
                                setups.push(input.property.sink.clone());
                            }
                            Err(failure) => {
                                first_failure.get_or_insert(failure);
                            }
                        }
                    }
                    ExtensionCallTarget::Dependency(binding) => {
                        let state = &input.property.state;
                        match state.probe_imported_dependency_extension_callable(
                            &binding,
                            input.property.expression.clone(),
                            &ast::Ident {
                                text: "invoke".to_string(),
                                span: call.span,
                            },
                            call,
                            expected,
                            false,
                        ) {
                            Ok(probe) => {
                                probes.push(NamedFunctionLikeProbe::ImportedDependency(Box::new(
                                    probe,
                                )));
                                setups.push(input.property.sink.clone());
                            }
                            Err(failure) => {
                                first_failure.get_or_insert(failure);
                            }
                        }
                    }
                }
            }
        }
        if probes.is_empty() {
            return match (suppressed, first_failure) {
                (true, Some(failure)) => PropertyExtensionInvokeOutcome::Failed(failure),
                (true, None) => PropertyExtensionInvokeOutcome::Blocked,
                (false, failure) => PropertyExtensionInvokeOutcome::NoApplicable(failure),
            };
        }

        let mut state = self.clone();
        let Some(winner) =
            state.select_named_function_like("invoke", layer_name, &probes, call.args, call.span)
        else {
            return PropertyExtensionInvokeOutcome::Failed(Box::new(state));
        };
        let probe = probes.swap_remove(winner);
        let mut layer_sink = setups.swap_remove(winner);
        let expression = match probe {
            NamedFunctionLikeProbe::Callable(probe) => {
                let Some(resolved) = state.commit_named_callable(*probe, &mut layer_sink) else {
                    return PropertyExtensionInvokeOutcome::Failed(Box::new(state));
                };
                let callee = state.materialize_resolved_callee(&resolved);
                state.check_call_effects(callee, call.span);
                hir::Expr {
                    kind: hir::ExprKind::Call {
                        binding: None,
                        callee: callee.into(),
                        receiver: resolved.source_receiver,
                        args: resolved.args,
                    },
                    ty: resolved.return_ty,
                    span: call.span,
                    origin: state.expression_origin(call.span),
                }
            }
            NamedFunctionLikeProbe::ImportedDependency(probe) => {
                let Some(expression) =
                    state.commit_imported_dependency_callable(*probe, &mut layer_sink)
                else {
                    return PropertyExtensionInvokeOutcome::Failed(Box::new(state));
                };
                expression
            }
            NamedFunctionLikeProbe::ImportedDependencyProperty(_)
            | NamedFunctionLikeProbe::Nominal(_)
            | NamedFunctionLikeProbe::IntrinsicStruct(_) => {
                unreachable!("property extension invoke partitions contain callable candidates")
            }
        };
        PropertyExtensionInvokeOutcome::Resolved(SuccessfulExprLayer {
            state: Box::new(state),
            expression,
            sink: layer_sink,
        })
    }
}
