//! Unified declaration-call resolution (M16, `docs/milestone16/DESIGN.md`).
//! The caller probes lexical/member/import layers in priority order and stops
//! at the first layer containing at least one applicable candidate. This
//! module resolves one such layer:
//!
//! 1. **Applicability.** Every candidate owns its argument map, fresh
//!    inference session, constraints, postponed arguments and failure trace.
//!    Single-candidate layers use exactly this path too.
//! 2. **Most specific candidate (MSC).** Pairwise declaration forwarding uses
//!    fresh variables and declaration bounds; it never compares type arguments
//!    inferred from this call. A unique dominator wins, with the specified
//!    non-generic tie-break, otherwise the call is ambiguous.
//! 3. **Commit.** Candidate probes are scratch transactions. Only the winner's
//!    expressions, coercions, entities, captures and instantiations survive.

use scoop_ast as ast;
use scoop_hir as hir;

use ast::Span;
use hir::{FunctionId, TypeId};

use crate::call_resolution::arguments::CandidateArgumentMap;
use crate::call_resolution::candidates::CallableView;
use crate::{CallableCandidate, CallableCandidateSource, Lowerer};

mod diagnostics;
mod inference;
mod probe;
mod specificity;

/// The winner of overload resolution, ready to be wrapped in an
/// `ExprKind::Call` / `ExprKind::MethodCall` by the caller.
pub(crate) struct ResolvedCallee {
    target: CallableCandidate,
    pub(crate) source: CallableCandidateSource,
    pub(crate) type_args: Vec<TypeId>,
    /// The arguments, lowered once and adapted (boxed where needed) to
    /// the winner's parameter types.
    pub(crate) args: Vec<hir::Expr>,
    pub(crate) return_ty: TypeId,
}

impl ResolvedCallee {
    pub(crate) fn function(&self) -> FunctionId {
        self.target.function
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OverloadCall<'a> {
    pub(crate) explicit_type_args: &'a [TypeId],
    pub(crate) arg_exprs: &'a [ast::Expr],
    pub(crate) span: Span,
    pub(crate) expected_result: Option<TypeId>,
}

/// A member-overload call whose arguments have already been lowered in source
/// order. Operator resolution uses this form because both operands are
/// language-mandated single evaluations, while applicability and MSC must
/// remain exactly the ordinary member-call algorithm.
pub(crate) struct LoweredOverloadCall {
    pub(crate) explicit_type_args: Vec<TypeId>,
    pub(crate) args: Vec<hir::Expr>,
    pub(crate) span: Span,
    pub(crate) expected_result: Option<TypeId>,
}

enum OverloadArguments<'a> {
    Source(&'a [ast::Expr]),
    Lowered(Vec<hir::Expr>),
}

struct OverloadResolution<'a> {
    receiver: OverloadReceiver,
    explicit_type_args: &'a [TypeId],
    arguments: OverloadArguments<'a>,
    span: Span,
    expected_result: Option<TypeId>,
}

/// A candidate prepared for resolution: parameter and return types
/// still use the function's combined type-parameter namespace. A generic
/// receiver pre-binds the owner prefix; applicability infers the remaining
/// method suffix and substitutes the complete vector.
struct Candidate {
    view: CallableView,
    argument_map: Result<CandidateArgumentMap, crate::call_resolution::arguments::ArityMismatch>,
    function: FunctionId,
    owner: crate::CallableCandidateOwner,
    source: CallableCandidateSource,
    params: Vec<TypeId>,
    return_ty: TypeId,
    /// Parameters declared by the function/method itself. Owner-only
    /// genericity does not make an otherwise concrete overload generic for
    /// MSC tie-breaking.
    own_type_param_count: usize,
    owner_arguments: Vec<TypeId>,
    explicit_arity_match: bool,
    call_span: Span,
}

enum OverloadReceiver {
    Ordinary,
    Extension(hir::Expr),
}

impl Lowerer {
    /// Materialize the already selected declaration target. Callers invoke
    /// this only after any winner-specific intrinsic normalization has had a
    /// chance to consume the typed target directly.
    pub(crate) fn materialize_resolved_callee(
        &mut self,
        resolved: &ResolvedCallee,
    ) -> hir::Callable {
        self.materialize_candidate_callable(&resolved.target, &resolved.type_args)
    }

    /// Resolve one candidate layer. `receiver_type_args` are the already fixed
    /// owner arguments (empty for top-level functions). On success the unique
    /// winner is committed into this layer transaction; on failure a stable
    /// diagnostic is recorded for the caller to retain or discard with it.
    pub(crate) fn resolve_overload(
        &mut self,
        name: &str,
        candidates: &[FunctionId],
        receiver_type_args: &[TypeId],
        call: OverloadCall<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedCallee> {
        let candidates = candidates
            .iter()
            .copied()
            .map(|function| CallableCandidate::function(function, receiver_type_args.to_vec()))
            .collect::<Vec<_>>();
        self.resolve_overload_with_receiver(
            name,
            &candidates,
            OverloadResolution {
                receiver: OverloadReceiver::Ordinary,
                explicit_type_args: call.explicit_type_args,
                arguments: OverloadArguments::Source(call.arg_exprs),
                span: call.span,
                expected_result: call.expected_result,
            },
            sink,
        )
    }

    pub(crate) fn resolve_member_overload(
        &mut self,
        name: &str,
        candidates: &[CallableCandidate],
        call: OverloadCall<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedCallee> {
        self.resolve_overload_with_receiver(
            name,
            candidates,
            OverloadResolution {
                receiver: OverloadReceiver::Ordinary,
                explicit_type_args: call.explicit_type_args,
                arguments: OverloadArguments::Source(call.arg_exprs),
                span: call.span,
                expected_result: call.expected_result,
            },
            sink,
        )
    }

    pub(crate) fn resolve_member_overload_lowered(
        &mut self,
        name: &str,
        candidates: &[CallableCandidate],
        call: LoweredOverloadCall,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedCallee> {
        let LoweredOverloadCall {
            explicit_type_args,
            args,
            span,
            expected_result,
        } = call;
        self.resolve_overload_with_receiver(
            name,
            candidates,
            OverloadResolution {
                receiver: OverloadReceiver::Ordinary,
                explicit_type_args: &explicit_type_args,
                arguments: OverloadArguments::Lowered(args),
                span,
                expected_result,
            },
            sink,
        )
    }

    /// Resolve an extension candidate layer. The already-lowered receiver is
    /// the first inference argument and, for the selected extension, the first
    /// direct-call argument. It is not part of the source argument count.
    pub(crate) fn resolve_extension_overload(
        &mut self,
        name: &str,
        candidates: &[FunctionId],
        receiver: hir::Expr,
        call: OverloadCall<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedCallee> {
        let candidates = candidates
            .iter()
            .copied()
            .map(|function| CallableCandidate::function(function, Vec::new()))
            .collect::<Vec<_>>();
        self.resolve_overload_with_receiver(
            name,
            &candidates,
            OverloadResolution {
                receiver: OverloadReceiver::Extension(receiver),
                explicit_type_args: call.explicit_type_args,
                arguments: OverloadArguments::Source(call.arg_exprs),
                span: call.span,
                expected_result: call.expected_result,
            },
            sink,
        )
    }

    fn resolve_overload_with_receiver(
        &mut self,
        name: &str,
        candidates: &[CallableCandidate],
        resolution: OverloadResolution<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedCallee> {
        let OverloadResolution {
            receiver,
            explicit_type_args,
            arguments,
            span,
            expected_result,
        } = resolution;
        let arg_count = match &arguments {
            OverloadArguments::Source(args) => args.len(),
            OverloadArguments::Lowered(args) => args.len(),
        };
        let receiver = match receiver {
            OverloadReceiver::Ordinary => None,
            OverloadReceiver::Extension(receiver) => Some(receiver),
        };
        let receiver_offset = usize::from(receiver.is_some());
        let prepared: Vec<Candidate> = candidates
            .iter()
            .map(|source| {
                let view = self.callable_view(source, receiver_offset != 0);
                let function = view.function();
                let argument_map = CandidateArgumentMap::exact(&view, arg_count);
                let mut params: Vec<_> = match &argument_map {
                    Ok(argument_map) => argument_map
                        .parameters
                        .iter()
                        .map(|input| {
                            let parameter = &view.value_parameters[input.parameter.index()];
                            debug_assert_eq!(input.input.index(), input.parameter.index());
                            parameter.ty
                        })
                        .collect(),
                    Err(_) => view
                        .value_parameters
                        .iter()
                        .map(|parameter| parameter.ty)
                        .collect(),
                };
                let signature = &self.signatures[&function];
                debug_assert_eq!(view.effects.is_suspend, signature.is_suspend);
                debug_assert_eq!(view.effects.attributes, signature.attributes);
                debug_assert_eq!(view.declaration_span, self.functions[function].span);
                debug_assert!(
                    view.value_parameters
                        .iter()
                        .zip(&signature.params)
                        .all(|(view, declaration)| view.name == declaration.name.text)
                );
                if receiver_offset != 0 {
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
                let explicit_arity_match = explicit_type_args.is_empty()
                    || explicit_type_args.len() == own_type_param_count;
                Candidate {
                    argument_map,
                    function,
                    owner: source.owner.clone(),
                    source: view.dispatch,
                    params,
                    return_ty: view.return_type,
                    own_type_param_count,
                    owner_arguments,
                    explicit_arity_match,
                    call_span: span,
                    view,
                }
            })
            .collect();

        // Every candidate starts from the exact same semantic state. Its
        // lowered expressions, generated callable entities, coercions and
        // diagnostics remain inside that transaction until MSC chooses it.
        let mut applicable = Vec::new();
        let mut failures = Vec::new();
        for (index, candidate) in prepared.iter().enumerate() {
            if !candidate.explicit_arity_match {
                failures.push(probe::CandidateProbeFailure {
                    candidate: index,
                    state: Box::new(self.clone()),
                    arguments: Vec::new(),
                    kind: probe::CandidateProbeFailureKind::Shape(
                        probe::CandidateShapeFailure::TypeArgumentArity {
                            expected: candidate.own_type_param_count,
                            supplied: explicit_type_args.len(),
                        },
                    ),
                });
                continue;
            }
            if let Err(mismatch) = &candidate.argument_map {
                failures.push(probe::CandidateProbeFailure {
                    candidate: index,
                    state: Box::new(self.clone()),
                    arguments: Vec::new(),
                    kind: probe::CandidateProbeFailureKind::Shape(
                        probe::CandidateShapeFailure::ArgumentArity {
                            expected: mismatch.expected,
                            supplied: mismatch.supplied,
                        },
                    ),
                });
                continue;
            }
            match self.probe_overload_candidate(
                index,
                candidate,
                receiver.as_ref(),
                explicit_type_args,
                &arguments,
                expected_result,
            ) {
                Ok(candidate) => applicable.push(candidate),
                Err(failure) => failures.push(*failure),
            }
        }

        let winner = match applicable.len() {
            0 => {
                self.candidate_failures_diagnostic(
                    name,
                    &prepared,
                    &mut failures,
                    &arguments,
                    receiver.as_ref(),
                    span,
                );
                return None;
            }
            1 => applicable[0].candidate,
            _ => {
                let indices = applicable
                    .iter()
                    .map(|candidate| candidate.candidate)
                    .collect::<Vec<_>>();
                self.most_specific(name, &prepared, &indices, span)?
            }
        };

        let candidate = &prepared[winner];
        let function = candidate.function;
        let owner = candidate.owner.clone();
        let source = candidate.source;
        let transaction_index = applicable
            .iter()
            .position(|candidate| candidate.candidate == winner)
            .expect("MSC winner has an applicability transaction");
        let transaction = applicable.swap_remove(transaction_index);
        let probe::ApplicableCandidate {
            state,
            type_args,
            args,
            argument_sinks,
            return_ty,
            ..
        } = transaction;
        *self = *state;
        for mut arg_sink in argument_sinks {
            sink.append(&mut arg_sink);
        }
        // The complete owner-prefix plus method-suffix vector identifies
        // the resolved generic entity stored on the HIR call.
        let resolved_candidate = CallableCandidate {
            function,
            owner,
            source,
        };
        Some(ResolvedCallee {
            target: resolved_candidate,
            source,
            type_args,
            args,
            return_ty,
        })
    }
}
