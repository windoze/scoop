//! Overload resolution (M7, docs/milestone7 DESIGN.md 1.2), aligned
//! with the Kotlin overload-resolution two-step: the caller picks the
//! candidate layer (host members → the call site's own side of the
//! core/user boundary → the other, implicitly imported side; the first
//! layer containing any candidate wins whole; an explicit-receiver
//! call has only the member layer), and this module selects the winner
//! inside one layer:
//!
//! 1. **Applicability.** Exact arity (no default arguments / varargs),
//!    type-argument inference for generic candidates (the M3 binding
//!    rules, run quietly — a conflict or an unbound parameter simply
//!    makes the candidate inapplicable), and every argument a subtype
//!    of its parameter (`is_subtype`, boxing included).
//! 2. **Most specific candidate (MSC).** Candidate A dominates B when
//!    every parameter type of A is a subtype of B's (generic candidates
//!    compare with their inferred type arguments — the simplification
//!    of Kotlin's fresh-variable constraint system documented in
//!    DESIGN.md 1.2). Exactly one dominator wins; on a tie (mutual or
//!    no dominance) non-generic candidates are preferred; anything
//!    still tied is an ambiguity diagnostic. Boxing needs no dedicated
//!    rule: `Any` is a supertype, so the unboxed candidate is naturally
//!    more specific.
//!
//! Single-candidate layers never reach here: the call sites keep the
//! pre-M7 code path so its diagnostics (arity and argument-type
//! messages naming the function, parameter types as expected-type
//! hints) stay exactly as they were.

use scoop_ast as ast;
use scoop_hir as hir;

use ast::Span;
use hir::{FunctionId, Type, TypeId};

use crate::{CallableCandidate, CallableCandidateSource, Lowerer};

mod diagnostics;
mod inference;
mod specificity;

/// The winner of overload resolution, ready to be wrapped in an
/// `ExprKind::Call` / `ExprKind::MethodCall` by the caller.
pub(crate) struct ResolvedCallee {
    pub(crate) callee: hir::Callable,
    pub(crate) source: CallableCandidateSource,
    pub(crate) type_args: Vec<TypeId>,
    /// The arguments, lowered once and adapted (boxed where needed) to
    /// the winner's parameter types.
    pub(crate) args: Vec<hir::Expr>,
    pub(crate) return_ty: TypeId,
}

#[derive(Clone, Copy)]
pub(crate) struct OverloadCall<'a> {
    pub(crate) explicit_type_args: &'a [TypeId],
    pub(crate) arg_exprs: &'a [ast::Expr],
    pub(crate) span: Span,
}

/// A member-overload call whose arguments have already been lowered in source
/// order. Operator resolution uses this form because both operands are
/// language-mandated single evaluations, while applicability and MSC must
/// remain exactly the ordinary member-call algorithm.
pub(crate) struct LoweredOverloadCall {
    pub(crate) explicit_type_args: Vec<TypeId>,
    pub(crate) args: Vec<hir::Expr>,
    pub(crate) span: Span,
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
}

/// A candidate prepared for resolution: parameter and return types
/// still use the function's combined type-parameter namespace. A generic
/// receiver pre-binds the owner prefix; applicability infers the remaining
/// method suffix and substitutes the complete vector.
struct Candidate {
    function: FunctionId,
    owner: crate::CallableCandidateOwner,
    source: CallableCandidateSource,
    params: Vec<TypeId>,
    return_ty: TypeId,
    /// Parameters declared by the function/method itself. Owner-only
    /// genericity does not make an otherwise concrete overload generic for
    /// MSC tie-breaking.
    own_type_param_count: usize,
    initial_bindings: Vec<Option<TypeId>>,
    explicit_arity_match: bool,
    /// Whether the candidate's declared (pre-instantiation) parameter
    /// types mention type parameters — its own or its host's. Such
    /// candidates lose MSC ties against fully concrete ones
    /// (DESIGN.md 1.2: non-parameterized candidates are preferred).
    parameterized: bool,
}

enum OverloadReceiver {
    Ordinary,
    Extension(hir::Expr),
}

impl Lowerer {
    /// Resolve a call over one candidate layer. `receiver_type_args`
    /// are the receiver's enum type arguments for method calls (empty
    /// for top-level functions and non-enum receivers). Records the
    /// winner's instantiation request and returns it; on failure the
    /// diagnostic is recorded and `None` comes back.
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
        } = call;
        self.resolve_overload_with_receiver(
            name,
            candidates,
            OverloadResolution {
                receiver: OverloadReceiver::Ordinary,
                explicit_type_args: &explicit_type_args,
                arguments: OverloadArguments::Lowered(args),
                span,
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
        } = resolution;
        let arg_count = match &arguments {
            OverloadArguments::Source(args) => args.len(),
            OverloadArguments::Lowered(args) => args.len(),
        };
        // Context-independent arguments are shared by every candidate and
        // lowered once. `None`, empty arrays and context-dependent generic
        // constructors are postponed until inference provides a candidate
        // parameter type. Per-argument sinks preserve source evaluation
        // order even when later arguments are typed first.
        let receiver = match receiver {
            OverloadReceiver::Ordinary => None,
            OverloadReceiver::Extension(receiver) => Some(receiver),
        };
        let receiver_offset = usize::from(receiver.is_some());
        let mut lowered: Vec<Option<hir::Expr>> = Vec::with_capacity(receiver_offset + arg_count);
        if let Some(receiver) = receiver {
            lowered.push(Some(receiver));
        }
        let mut arg_sinks: Vec<Vec<hir::Statement>> = (0..arg_count).map(|_| Vec::new()).collect();
        match &arguments {
            OverloadArguments::Source(arg_exprs) => {
                lowered.extend((0..arg_exprs.len()).map(|_| None));
                for (index, arg) in arg_exprs.iter().enumerate() {
                    if self.expr_requires_expected_type(arg) {
                        continue;
                    }
                    lowered[receiver_offset + index] =
                        Some(self.lower_expr(arg, &mut arg_sinks[index], None)?);
                }
            }
            OverloadArguments::Lowered(args) => {
                lowered.extend(args.iter().cloned().map(Some));
            }
        }
        let arg_tys: Vec<Option<TypeId>> = lowered
            .iter()
            .map(|arg| arg.as_ref().map(|arg| arg.ty))
            .collect();

        let prepared: Vec<Candidate> = candidates
            .iter()
            .map(|source| {
                let function = source.function;
                let sig = self.signatures[&function].clone();
                let mut params: Vec<_> = sig.params.iter().map(|param| param.ty).collect();
                if receiver_offset != 0 {
                    params.insert(
                        0,
                        *self
                            .extension_receivers
                            .get(&function)
                            .expect("extension candidate has a receiver type"),
                    );
                }
                let parameterized = params.iter().any(|&ty| self.mentions_type_param(ty));
                let owner_arguments = self.callable_candidate_owner_arguments(source);
                debug_assert_eq!(sig.owner_type_param_count, owner_arguments.len());
                let mut initial_bindings = vec![None; sig.type_params.len()];
                for (binding, &ty) in initial_bindings.iter_mut().zip(&owner_arguments) {
                    *binding = Some(ty);
                }
                let own_type_param_count = sig.type_params.len() - sig.owner_type_param_count;
                let explicit_arity_match = explicit_type_args.is_empty()
                    || explicit_type_args.len() == own_type_param_count;
                if explicit_arity_match && !explicit_type_args.is_empty() {
                    for (binding, &ty) in initial_bindings[sig.owner_type_param_count..]
                        .iter_mut()
                        .zip(explicit_type_args)
                    {
                        *binding = Some(ty);
                    }
                }
                Candidate {
                    function,
                    owner: source.owner.clone(),
                    source: source.source,
                    params,
                    return_ty: sig.return_ty,
                    own_type_param_count,
                    initial_bindings,
                    explicit_arity_match,
                    parameterized,
                }
            })
            .collect();

        if !explicit_type_args.is_empty()
            && prepared
                .iter()
                .all(|candidate| !candidate.explicit_arity_match)
        {
            let supplied = explicit_type_args.len();
            let expected = prepared[0].own_type_param_count;
            if prepared
                .iter()
                .all(|candidate| candidate.own_type_param_count == expected)
            {
                self.error(
                    span,
                    format!(
                        "`{name}` takes exactly {expected} type argument(s), but {supplied} were supplied"
                    ),
                );
            } else {
                self.error(
                    span,
                    format!("no overload of `{name}` accepts {supplied} explicit type argument(s)"),
                );
            }
            return None;
        }

        // Applicability (step 1): each entry pairs a prepared-candidate
        // index with its inferred call-level type arguments.
        let mut applicable: Vec<(usize, Vec<TypeId>)> = Vec::new();
        let mut kind_failures: Vec<(usize, Vec<TypeId>)> = Vec::new();
        let mut contextual_failures: Vec<(usize, TypeId, String)> = Vec::new();
        for (index, candidate) in prepared.iter().enumerate() {
            if !candidate.explicit_arity_match {
                continue;
            }
            if candidate.params.len() != receiver_offset + arg_count {
                continue;
            }
            let Some(type_args) = self.try_infer_type_args(candidate, &arg_tys) else {
                continue;
            };
            let ordinary_args_match = candidate.params.iter().zip(&arg_tys).all(|(&param, arg)| {
                let expected = self.substitute_call_level(param, &type_args);
                match arg {
                    Some(arg) => self.is_subtype(*arg, expected),
                    None => true,
                }
            });
            if !ordinary_args_match {
                continue;
            }
            let mut contextual_args_match = true;
            for (argument, (&param, arg)) in candidate.params.iter().zip(&arg_tys).enumerate() {
                if argument < receiver_offset {
                    continue;
                }
                if arg.is_some() {
                    continue;
                }
                let expected = self.substitute_call_level(param, &type_args);
                let source_argument = argument - receiver_offset;
                if let OverloadArguments::Source(arg_exprs) = &arguments {
                    if let Err(reason) =
                        self.probe_contextual_expr(&arg_exprs[source_argument], expected)
                    {
                        contextual_failures.push((source_argument, expected, reason));
                        contextual_args_match = false;
                    }
                }
            }
            if contextual_args_match {
                let type_params = self.signatures[&candidate.function].type_params.clone();
                if self.type_arguments_satisfy_kinds(&type_params, &type_args) {
                    applicable.push((index, type_args));
                } else {
                    kind_failures.push((index, type_args));
                }
            }
        }

        let (winner, type_args) = match applicable.len() {
            0 => {
                if kind_failures.len() == 1 {
                    let (index, type_args) = kind_failures.pop().expect("one kind failure");
                    let function = prepared[index].function;
                    let type_params = self.signatures[&function].type_params.clone();
                    self.check_type_argument_kinds(
                        &type_params,
                        &type_args,
                        span,
                        &format!("function `{}`", self.functions[function].name),
                    );
                    return None;
                }
                if let OverloadArguments::Source(arg_exprs) = &arguments {
                    if self.contextual_no_applicable_diagnostic(
                        name,
                        arg_exprs,
                        &contextual_failures,
                    ) {
                        return None;
                    }
                }
                self.no_applicable_diagnostic(
                    name,
                    &prepared,
                    &arg_tys[receiver_offset..],
                    match &arguments {
                        OverloadArguments::Source(args) => Some(args),
                        OverloadArguments::Lowered(_) => None,
                    },
                    (receiver_offset != 0).then(|| arg_tys[0]).flatten(),
                    span,
                );
                return None;
            }
            1 => applicable.pop().expect("one applicable candidate"),
            _ => self.most_specific(name, &prepared, &applicable, span)?,
        };

        let candidate = &prepared[winner];
        let function = candidate.function;
        let mut args = Vec::with_capacity(lowered.len());
        for (index, &param) in candidate.params.iter().enumerate() {
            let expected = self.substitute_call_level(param, &type_args);
            let arg = match lowered[index].take() {
                Some(arg) => arg,
                None => {
                    let source_index = index - receiver_offset;
                    let OverloadArguments::Source(arg_exprs) = &arguments else {
                        unreachable!("pre-lowered overload arguments are all present")
                    };
                    self.lower_expr(
                        &arg_exprs[source_index],
                        &mut arg_sinks[source_index],
                        Some(expected),
                    )?
                }
            };
            if !self.is_subtype(arg.ty, expected) {
                self.no_applicable_diagnostic(
                    name,
                    &prepared,
                    &arg_tys[receiver_offset..],
                    match &arguments {
                        OverloadArguments::Source(args) => Some(args),
                        OverloadArguments::Lowered(_) => None,
                    },
                    (receiver_offset != 0).then(|| arg_tys[0]).flatten(),
                    span,
                );
                return None;
            }
            args.push(self.adapt_to(arg, expected));
        }
        for mut arg_sink in arg_sinks {
            sink.append(&mut arg_sink);
        }
        let return_ty = self.substitute_call_level(candidate.return_ty, &type_args);
        // The complete owner-prefix plus method-suffix vector identifies
        // the resolved generic entity stored on the HIR call.
        let resolved_candidate = CallableCandidate {
            function,
            owner: candidate.owner.clone(),
            source: candidate.source,
        };
        let callee = self.materialize_candidate_callable(&resolved_candidate, &type_args);
        Some(ResolvedCallee {
            callee,
            source: candidate.source,
            type_args,
            args,
            return_ty,
        })
    }
}
