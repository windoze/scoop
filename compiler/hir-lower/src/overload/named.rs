//! Candidate-local callable transactions for a mixed named-call partition.

use super::*;

#[derive(Clone)]
pub(crate) enum NamedCallReceiver {
    None,
    Member(hir::Expr),
    Extension(hir::Expr),
}

pub(crate) struct NamedCallableProbe {
    prepared: Candidate,
    transaction: probe::ApplicableCandidate,
    receiver: NamedCallReceiver,
}

impl NamedCallableProbe {
    pub(crate) fn forwarding(
        &self,
    ) -> crate::call_resolution::specificity::DeclarationForwardingView<'_> {
        crate::call_resolution::specificity::ForwardingDeclaration {
            view: &self.prepared.view,
            parameter_types: &self.prepared.params,
        }
        .into()
    }

    pub(crate) fn parameterized(&self) -> bool {
        self.prepared.own_type_param_count != 0
    }

    pub(crate) fn defaults(&self) -> usize {
        self.prepared
            .argument_map
            .as_ref()
            .expect("a successful probe has an argument map")
            .explicit_default_count()
    }

    pub(crate) fn vararg(&self) -> bool {
        self.prepared.view.value_parameters.iter().any(|parameter| {
            matches!(
                parameter.calling,
                crate::defaults::SourceParameterCalling::Vararg { .. }
            )
        })
    }

    pub(crate) fn source_argument_integer(&self, index: usize) -> Option<hir::IntegerKind> {
        let offset = usize::from(matches!(self.receiver, NamedCallReceiver::Extension(_)));
        match self.transaction.state.types[self.transaction.args[index + offset].ty] {
            hir::Type::Integer(kind) => Some(kind),
            _ => None,
        }
    }

    pub(crate) fn signature(&self, state: &Lowerer, name: &str) -> String {
        crate::call_resolution::diagnostics::callable_source_signature(
            state,
            name,
            &self.prepared.view,
        )
    }

    pub(crate) fn declaration_location(&self, state: &Lowerer) -> (usize, scoop_ast::Span) {
        let function = self.prepared.view.function();
        (
            state.function_files[&function],
            self.prepared.view.declaration_span,
        )
    }
}

impl Lowerer {
    pub(crate) fn probe_named_callable(
        &self,
        name: &str,
        target: CallableCandidate,
        receiver: NamedCallReceiver,
        call: OverloadCall<'_>,
    ) -> Result<NamedCallableProbe, Box<Lowerer>> {
        assert!(
            !self.declaration_surface.rejects_function(target.function),
            "rejected declarations never enter named callable probing"
        );
        let arguments = OverloadArguments::Source(call.arg_exprs);
        let extension = matches!(receiver, NamedCallReceiver::Extension(_));
        let prepared = self.prepare_overload_candidate(
            &target,
            call.explicit_type_args,
            &arguments,
            call.argument_protocol,
            call.span,
            extension,
        );
        let inference_receiver = match &receiver {
            NamedCallReceiver::Extension(receiver) => Some(receiver),
            NamedCallReceiver::None | NamedCallReceiver::Member(_) => None,
        };
        if prepared.explicit_arity_match && prepared.argument_map.is_ok() {
            if let Ok(transaction) = self.probe_overload_candidate(
                0,
                &prepared,
                inference_receiver,
                call.explicit_type_args,
                &arguments,
                call.expected_result,
            ) {
                return Ok(NamedCallableProbe {
                    prepared,
                    transaction,
                    receiver,
                });
            }
        }
        // The ordinary singleton resolver owns the established detailed
        // shape/constraint diagnostic. Its entire failure stays scratch.
        let mut failure = self.clone();
        let mut sink = Vec::new();
        failure.resolve_overload_with_receiver(
            name,
            &[target],
            OverloadResolution {
                receiver: match receiver {
                    NamedCallReceiver::None => OverloadReceiver::Ordinary,
                    NamedCallReceiver::Member(receiver) => OverloadReceiver::Instance(receiver),
                    NamedCallReceiver::Extension(receiver) => OverloadReceiver::Extension(receiver),
                },
                explicit_type_args: call.explicit_type_args,
                arguments,
                span: call.span,
                expected_result: call.expected_result,
                argument_protocol: call.argument_protocol,
            },
            &mut sink,
        );
        Err(Box::new(failure))
    }

    pub(crate) fn commit_named_callable(
        &mut self,
        probe: NamedCallableProbe,
        sink: &mut Vec<hir::Statement>,
    ) -> ResolvedCallee {
        let NamedCallableProbe {
            prepared,
            transaction,
            receiver,
        } = probe;
        let (evaluation_receiver, extension) = match receiver {
            NamedCallReceiver::None => (None, false),
            NamedCallReceiver::Member(receiver) => (Some(receiver), false),
            NamedCallReceiver::Extension(receiver) => (Some(receiver), true),
        };
        self.commit_overload_candidate(
            &prepared,
            transaction,
            evaluation_receiver,
            extension,
            true,
            sink,
        )
    }

    pub(super) fn prepare_overload_candidate(
        &self,
        source: &CallableCandidate,
        explicit_type_args: &[ResolvedCallTypeArgument],
        arguments: &OverloadArguments<'_>,
        protocol: CallArgumentProtocol,
        span: Span,
        extension: bool,
    ) -> Candidate {
        let view = self.callable_view(source, extension);
        let function = view.function();
        let signature = &self.signatures[&function];
        debug_assert_eq!(view.effects.is_suspend, signature.is_suspend);
        debug_assert_eq!(view.effects.attributes, signature.attributes);
        debug_assert_eq!(view.declaration_span, self.functions[function].span);
        let argument_map = match arguments {
            OverloadArguments::Source(arguments) => match protocol {
                CallArgumentProtocol::Ordinary => CandidateArgumentMap::source(&view, arguments),
                CallArgumentProtocol::OperatorSet => {
                    CandidateArgumentMap::source_operator_set(&view, arguments)
                }
            },
            OverloadArguments::Lowered(arguments) => {
                CandidateArgumentMap::exact_lowered(&view, arguments.len())
            }
        };
        let mut params = match &argument_map {
            Ok(argument_map) => argument_map.forwarding_parameter_types(&view.value_parameters),
            Err(_) => view
                .value_parameters
                .iter()
                .map(|parameter| parameter.ty)
                .collect(),
        };
        if extension {
            let crate::call_resolution::candidates::ReceiverShape::Extension(receiver) =
                view.receiver
            else {
                unreachable!("extension resolution builds extension callable views")
            };
            params.insert(0, receiver);
        }
        let owner_arguments = self.callable_candidate_owner_arguments(source);
        debug_assert_eq!(view.owner_parameters.len(), owner_arguments.len());
        let own_type_param_count = view.callable_parameters.len();
        Candidate {
            argument_map,
            function,
            owner: source.owner.clone(),
            source: view.dispatch,
            access: source.access.clone(),
            params,
            return_ty: view.return_type,
            own_type_param_count,
            owner_arguments,
            explicit_arity_match: explicit_type_args.is_empty()
                || explicit_type_args.len() == own_type_param_count,
            call_span: span,
            view,
        }
    }

    pub(super) fn commit_overload_candidate(
        &mut self,
        candidate: &Candidate,
        transaction: probe::ApplicableCandidate,
        evaluation_receiver: Option<hir::Expr>,
        extension: bool,
        source_arguments: bool,
        sink: &mut Vec<hir::Statement>,
    ) -> ResolvedCallee {
        let probe::ApplicableCandidate {
            state,
            type_args,
            mut args,
            argument_sinks,
            return_ty,
            ..
        } = transaction;
        *self = *state;
        let argument_map = candidate
            .argument_map
            .as_ref()
            .expect("the winner has a complete argument map");
        let (instance_receiver, args) = if source_arguments {
            let receiver = if extension {
                Some(args.remove(0))
            } else {
                evaluation_receiver
            };
            let (materialized_receiver, mut args) = self.materialize_callable_arguments(
                crate::argument_materialization::CallableArgumentMaterialization {
                    function: candidate.function,
                    argument_map,
                    type_args: &type_args,
                    receiver,
                    source_args: args,
                    argument_sinks,
                    call_span: candidate.call_span,
                },
                sink,
            );
            if extension {
                args.insert(
                    0,
                    materialized_receiver.expect("an extension receiver is materialized"),
                );
                (None, args)
            } else {
                (materialized_receiver, args)
            }
        } else {
            for mut argument_sink in argument_sinks {
                sink.append(&mut argument_sink);
            }
            if extension {
                (None, args)
            } else {
                (evaluation_receiver, args)
            }
        };
        ResolvedCallee {
            target: CallableCandidate {
                function: candidate.function,
                owner: candidate.owner.clone(),
                source: candidate.source,
                access: candidate.access.clone(),
            },
            source: candidate.source,
            type_args,
            args,
            receiver: instance_receiver,
            return_ty,
        }
    }
}
