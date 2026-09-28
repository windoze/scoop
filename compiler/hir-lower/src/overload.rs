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

use crate::call_resolution::arguments::{ArgumentShapeFailure, CandidateArgumentMap};
use crate::call_resolution::candidates::CallableView;
use crate::expr::ResolvedCallTypeArgument;
use crate::{CallableCandidate, CallableCandidateSource, Lowerer};

mod diagnostics;
mod inference;
mod named;
mod probe;
mod specificity;

pub(crate) use named::{NamedCallReceiver, NamedCallableProbe};

/// The winner of overload resolution, ready to be wrapped in an
/// `ExprKind::Call` / `ExprKind::MethodCall` by the caller.
pub(crate) struct ResolvedCallee {
    target: CallableCandidate,
    pub(crate) source: CallableCandidateSource,
    pub(crate) type_args: Vec<TypeId>,
    /// The arguments, lowered once and adapted (boxed where needed) to
    /// the winner's parameter types.
    pub(crate) args: Vec<hir::Expr>,
    /// Materialized instance receiver. Extension receivers are normalized to
    /// the hidden first direct-call argument instead.
    pub(crate) receiver: Option<hir::Expr>,
    pub(crate) source_receiver: hir::SourceCallReceiver<TypeId>,
    pub(crate) return_ty: TypeId,
}

pub(crate) enum OverloadResolutionOutcome {
    NoApplicable,
    /// This layer owns the callable spelling, but every matching declaration
    /// is already rejected by the frozen declaration surface. The definition
    /// diagnostic is sufficient; callers must stop without committing a new
    /// layer diagnostic or probing a lower layer.
    Blocked,
    Failed,
    Resolved(Box<ResolvedCallee>),
}

impl OverloadResolutionOutcome {
    fn into_option(self) -> Option<ResolvedCallee> {
        match self {
            Self::Resolved(resolved) => Some(*resolved),
            Self::NoApplicable | Self::Blocked | Self::Failed => None,
        }
    }
}

impl ResolvedCallee {
    pub(crate) fn function(&self) -> FunctionId {
        self.target.function
    }
}

#[derive(Clone, Copy)]
pub(crate) struct OverloadCall<'a> {
    pub(crate) explicit_type_args: &'a [ResolvedCallTypeArgument],
    pub(crate) arg_exprs: &'a [ast::CallArgument],
    pub(crate) span: Span,
    pub(crate) expected_result: Option<TypeId>,
    pub(crate) argument_protocol: CallArgumentProtocol,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CallArgumentProtocol {
    Ordinary,
    OperatorSet,
}

/// A member-overload call whose arguments have already been lowered in source
/// order. Operator resolution uses this form because both operands are
/// language-mandated single evaluations, while applicability and MSC must
/// remain exactly the ordinary member-call algorithm.
pub(crate) struct LoweredOverloadCall {
    pub(crate) explicit_type_args: Vec<ResolvedCallTypeArgument>,
    pub(crate) args: Vec<hir::Expr>,
    pub(crate) span: Span,
    pub(crate) expected_result: Option<TypeId>,
}

enum OverloadArguments<'a> {
    Source(&'a [ast::CallArgument]),
    Lowered(Vec<hir::Expr>),
}

struct OverloadResolution<'a> {
    receiver: OverloadReceiver,
    explicit_type_args: &'a [ResolvedCallTypeArgument],
    arguments: OverloadArguments<'a>,
    span: Span,
    expected_result: Option<TypeId>,
    argument_protocol: CallArgumentProtocol,
}

/// A candidate prepared for resolution: parameter and return types
/// still use the function's combined type-parameter namespace. A generic
/// receiver pre-binds the owner prefix; applicability infers the remaining
/// method suffix and substitutes the complete vector.
struct Candidate {
    view: CallableView,
    argument_map: Result<CandidateArgumentMap, ArgumentShapeFailure>,
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
    Instance(hir::Expr),
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
        self.resolve_overload_outcome(name, candidates, receiver_type_args, call, sink)
            .into_option()
    }

    pub(crate) fn resolve_overload_outcome(
        &mut self,
        name: &str,
        candidates: &[FunctionId],
        receiver_type_args: &[TypeId],
        call: OverloadCall<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> OverloadResolutionOutcome {
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
                argument_protocol: call.argument_protocol,
            },
            sink,
        )
    }

    pub(crate) fn resolve_member_overload_outcome(
        &mut self,
        name: &str,
        candidates: &[CallableCandidate],
        receiver: hir::Expr,
        call: OverloadCall<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> OverloadResolutionOutcome {
        self.resolve_overload_with_receiver(
            name,
            candidates,
            OverloadResolution {
                receiver: OverloadReceiver::Instance(receiver),
                explicit_type_args: call.explicit_type_args,
                arguments: OverloadArguments::Source(call.arg_exprs),
                span: call.span,
                expected_result: call.expected_result,
                argument_protocol: call.argument_protocol,
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
                argument_protocol: CallArgumentProtocol::Ordinary,
            },
            sink,
        )
        .into_option()
    }

    fn resolve_overload_with_receiver(
        &mut self,
        name: &str,
        candidates: &[CallableCandidate],
        resolution: OverloadResolution<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> OverloadResolutionOutcome {
        let suppressed = candidates.iter().any(|candidate| {
            self.declaration_surface
                .rejects_function(candidate.function)
        });
        let candidates = candidates
            .iter()
            .filter(|candidate| {
                !self
                    .declaration_surface
                    .rejects_function(candidate.function)
            })
            .cloned()
            .collect::<Vec<_>>();
        if candidates.is_empty() && suppressed {
            // The duplicate-signature diagnostic is already attached to the
            // declaration. This diagnostic-only blocker must stop a caller
            // from probing a lower member/import layer, without manufacturing
            // a semantic candidate from either rejected declaration.
            return OverloadResolutionOutcome::Blocked;
        }
        let OverloadResolution {
            receiver,
            explicit_type_args,
            arguments,
            span,
            expected_result,
            argument_protocol,
        } = resolution;
        let (evaluation_receiver, extension) = match receiver {
            OverloadReceiver::Ordinary => (None, false),
            OverloadReceiver::Instance(receiver) => (Some(receiver), false),
            OverloadReceiver::Extension(receiver) => (Some(receiver), true),
        };
        let inference_receiver = if extension {
            Some(
                evaluation_receiver
                    .as_ref()
                    .expect("an extension call has a receiver"),
            )
        } else {
            None
        };
        let receiver_offset = usize::from(extension);
        let prepared: Vec<Candidate> = candidates
            .iter()
            .map(|source| {
                self.prepare_overload_candidate(
                    source,
                    explicit_type_args,
                    &arguments,
                    argument_protocol,
                    span,
                    extension,
                )
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
            if let Err(failure) = &candidate.argument_map {
                failures.push(probe::CandidateProbeFailure {
                    candidate: index,
                    state: Box::new(self.clone()),
                    arguments: Vec::new(),
                    kind: probe::CandidateProbeFailureKind::Shape(
                        probe::CandidateShapeFailure::Argument(failure.clone()),
                    ),
                });
                continue;
            }
            match self.probe_overload_candidate(
                index,
                candidate,
                inference_receiver,
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
                    diagnostics::CandidateFailureContext {
                        arguments: &arguments,
                        extension_receiver: inference_receiver,
                        explicit_type_args,
                        span,
                    },
                );
                return if suppressed {
                    OverloadResolutionOutcome::Failed
                } else {
                    OverloadResolutionOutcome::NoApplicable
                };
            }
            1 => applicable[0].candidate,
            _ => {
                let indices = applicable
                    .iter()
                    .map(|candidate| candidate.candidate)
                    .collect::<Vec<_>>();
                let mut tied = self.most_specific_candidates(&prepared, &indices);
                if tied.len() > 1
                    && let OverloadArguments::Source(source_arguments) = &arguments
                {
                    tied = literal_default_pareto(
                        &tied,
                        &applicable,
                        source_arguments,
                        usize::from(inference_receiver.is_some()),
                    );
                }
                if tied.len() != 1 {
                    self.ambiguity_diagnostic(
                        name,
                        &prepared,
                        &tied,
                        diagnostics::AmbiguityContext {
                            applicable: &applicable,
                            arguments: &arguments,
                            receiver_offset,
                            span,
                        },
                    );
                    return OverloadResolutionOutcome::Failed;
                }
                tied[0]
            }
        };

        let candidate = &prepared[winner];
        let transaction_index = applicable
            .iter()
            .position(|candidate| candidate.candidate == winner)
            .expect("MSC winner has an applicability transaction");
        let transaction = applicable.swap_remove(transaction_index);
        match self.commit_overload_candidate(
            candidate,
            transaction,
            evaluation_receiver,
            extension,
            matches!(arguments, OverloadArguments::Source(_)),
            sink,
        ) {
            Some(resolved) => OverloadResolutionOutcome::Resolved(Box::new(resolved)),
            None => OverloadResolutionOutcome::Failed,
        }
    }
}

fn literal_default_pareto(
    candidates: &[usize],
    applicable: &[probe::ApplicableCandidate],
    arguments: &[ast::CallArgument],
    receiver_offset: usize,
) -> Vec<usize> {
    candidates
        .iter()
        .copied()
        .filter(|candidate| {
            !candidates.iter().copied().any(|other| {
                other != *candidate
                    && literal_default_dominates(
                        applicable
                            .iter()
                            .find(|transaction| transaction.candidate == other)
                            .expect("every tied candidate has an applicability transaction"),
                        applicable
                            .iter()
                            .find(|transaction| transaction.candidate == *candidate)
                            .expect("every tied candidate has an applicability transaction"),
                        arguments,
                        receiver_offset,
                    )
            })
        })
        .collect()
}

fn literal_default_dominates(
    preferred: &probe::ApplicableCandidate,
    other: &probe::ApplicableCandidate,
    arguments: &[ast::CallArgument],
    receiver_offset: usize,
) -> bool {
    let mut strictly_better = false;
    for (source_index, argument) in arguments.iter().enumerate() {
        let Some(default_kind) = crate::expr::integer_literal_default_kind(&argument.expression)
        else {
            continue;
        };
        let preferred_ty = preferred.args[receiver_offset + source_index].ty;
        let other_ty = other.args[receiver_offset + source_index].ty;
        let (hir::Type::Integer(preferred_kind), hir::Type::Integer(other_kind)) = (
            &preferred.state.types[preferred_ty],
            &other.state.types[other_ty],
        ) else {
            continue;
        };
        if preferred_kind == other_kind {
            continue;
        }
        match (*preferred_kind == default_kind, *other_kind == default_kind) {
            (true, false) => strictly_better = true,
            (false, true) => return false,
            (true, true) | (false, false) => {}
        }
    }
    strictly_better
}
