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

use crate::Lowerer;

/// The winner of overload resolution, ready to be wrapped in an
/// `ExprKind::Call` / `ExprKind::MethodCall` by the caller.
pub(crate) struct ResolvedCallee {
    pub(crate) function: FunctionId,
    /// Inferred call-level type arguments (empty unless the winner is
    /// a generic function).
    pub(crate) type_args: Vec<TypeId>,
    /// The arguments, lowered once and adapted (boxed where needed) to
    /// the winner's parameter types.
    pub(crate) args: Vec<hir::Expr>,
    pub(crate) return_ty: TypeId,
}

/// A candidate prepared for resolution: parameter and return types
/// already instantiated with the receiver's type arguments (enum
/// methods). A generic function's parameters still mention
/// `Type::Param`; they are instantiated with the inferred call-level
/// type arguments during applicability and dominance checks.
struct Candidate {
    function: FunctionId,
    params: Vec<TypeId>,
    return_ty: TypeId,
    type_param_count: usize,
    /// Whether the candidate's declared (pre-instantiation) parameter
    /// types mention type parameters — its own or its host's. Such
    /// candidates lose MSC ties against fully concrete ones
    /// (DESIGN.md 1.2: non-parameterized candidates are preferred).
    parameterized: bool,
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
        arg_exprs: &[ast::Expr],
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedCallee> {
        // The arguments are shared by every candidate, so they are
        // lowered once — without an expected-type hint, which only a
        // single known candidate could provide.
        let mut lowered = Vec::with_capacity(arg_exprs.len());
        for arg in arg_exprs {
            lowered.push(self.lower_expr(arg, sink, None)?);
        }
        let arg_tys: Vec<TypeId> = lowered.iter().map(|arg| arg.ty).collect();

        let prepared: Vec<Candidate> = candidates
            .iter()
            .map(|&function| {
                let sig = self.signatures[&function].clone();
                let parameterized = sig
                    .params
                    .iter()
                    .any(|param| self.mentions_type_param(param.ty));
                // The receiver substitution only applies to enum
                // methods; for everything else it is empty and the
                // declared types (possibly still mentioning the
                // function's own `Type::Param`s) are used as-is.
                let substitute = |this: &mut Self, ty: TypeId| {
                    if receiver_type_args.is_empty() {
                        ty
                    } else {
                        this.instantiate_ty(ty, receiver_type_args)
                    }
                };
                Candidate {
                    function,
                    params: sig
                        .params
                        .iter()
                        .map(|param| substitute(self, param.ty))
                        .collect(),
                    return_ty: substitute(self, sig.return_ty),
                    type_param_count: sig.type_params.len(),
                    parameterized,
                }
            })
            .collect();

        // Applicability (step 1): each entry pairs a prepared-candidate
        // index with its inferred call-level type arguments.
        let mut applicable: Vec<(usize, Vec<TypeId>)> = Vec::new();
        for (index, candidate) in prepared.iter().enumerate() {
            if candidate.params.len() != arg_exprs.len() {
                continue;
            }
            let Some(type_args) = self.try_infer_type_args(candidate, &arg_tys) else {
                continue;
            };
            let matches = candidate.params.iter().zip(&arg_tys).all(|(&param, &arg)| {
                let expected = self.substitute_call_level(param, &type_args);
                self.is_subtype(arg, expected)
            });
            if matches {
                applicable.push((index, type_args));
            }
        }

        let (winner, type_args) = match applicable.len() {
            0 => {
                self.no_applicable_diagnostic(name, &prepared, &arg_tys, arg_exprs.len(), span);
                return None;
            }
            1 => applicable.pop().expect("one applicable candidate"),
            _ => self.most_specific(name, &prepared, &applicable, span)?,
        };

        let candidate = &prepared[winner];
        let function = candidate.function;
        let mut args = Vec::with_capacity(lowered.len());
        for (&param, arg) in candidate.params.iter().zip(lowered) {
            let expected = self.substitute_call_level(param, &type_args);
            args.push(self.adapt_to(arg, expected));
        }
        let return_ty = self.substitute_call_level(candidate.return_ty, &type_args);
        // M9: the GC intrinsics constrain their type argument to
        // reference types (see `check_gc_ref_constraint`).
        if !self.check_gc_ref_constraint(function, &type_args, &args) {
            return None;
        }
        // Enum methods instantiate over the receiver's type arguments;
        // generic functions over the inferred call-level ones.
        if !receiver_type_args.is_empty() {
            self.record_instantiation(function, receiver_type_args.to_vec());
        }
        if !type_args.is_empty() {
            self.record_instantiation(function, type_args.clone());
        }
        Some(ResolvedCallee {
            function,
            type_args,
            args,
            return_ty,
        })
    }

    /// MSC selection (step 2) over two or more applicable candidates.
    /// Returns the winning `(prepared index, type arguments)` pair, or
    /// records the ambiguity diagnostic and returns `None`.
    fn most_specific(
        &mut self,
        name: &str,
        prepared: &[Candidate],
        applicable: &[(usize, Vec<TypeId>)],
        span: Span,
    ) -> Option<(usize, Vec<TypeId>)> {
        // Dominance compares the parameter types with each candidate's
        // own inferred type arguments applied.
        let inst_params: Vec<Vec<TypeId>> = applicable
            .iter()
            .map(|(index, type_args)| {
                prepared[*index]
                    .params
                    .iter()
                    .map(|&param| self.substitute_call_level(param, type_args))
                    .collect()
            })
            .collect();
        let mut dominators = Vec::new();
        for a in 0..applicable.len() {
            let dominates_all = (0..applicable.len()).all(|b| {
                b == a
                    || inst_params[a]
                        .iter()
                        .zip(&inst_params[b])
                        .all(|(&x, &y)| self.is_subtype(x, y))
            });
            if dominates_all {
                dominators.push(a);
            }
        }
        if dominators.len() == 1 {
            return Some(applicable[dominators[0]].clone());
        }
        // A tie (mutual or no dominance): concrete candidates win over
        // parameterized ones; anything still tied is ambiguous.
        let pool = if dominators.is_empty() {
            (0..applicable.len()).collect::<Vec<_>>()
        } else {
            dominators
        };
        let non_generic: Vec<usize> = pool
            .iter()
            .copied()
            .filter(|&a| {
                let candidate = &prepared[applicable[a].0];
                candidate.type_param_count == 0 && !candidate.parameterized
            })
            .collect();
        let pool = if non_generic.is_empty() {
            pool
        } else {
            non_generic
        };
        if pool.len() == 1 {
            Some(applicable[pool[0]].clone())
        } else {
            self.error(span, format!("call to `{name}` is ambiguous"));
            None
        }
    }

    /// No applicable candidate: when every overload shares one arity
    /// and the call supplies a different count, report it as an arity
    /// error against the name (the shape arity diagnostics had before
    /// overloading — this keeps the `print` / `println` arity messages
    /// intact); otherwise report the unmatched argument types.
    fn no_applicable_diagnostic(
        &mut self,
        name: &str,
        prepared: &[Candidate],
        arg_tys: &[TypeId],
        supplied: usize,
        span: Span,
    ) {
        let uniform_arity = prepared[0].params.len();
        if prepared
            .iter()
            .all(|candidate| candidate.params.len() == uniform_arity)
            && uniform_arity != supplied
        {
            let noun = if uniform_arity == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                span,
                format!(
                    "`{name}` takes exactly {uniform_arity} {noun}, but {supplied} were supplied"
                ),
            );
            return;
        }
        let found: Vec<String> = arg_tys.iter().map(|&ty| self.type_name(ty)).collect();
        self.error(
            span,
            format!(
                "no overload of `{name}` matches argument types ({})",
                found.join(", ")
            ),
        );
    }

    /// Substitute inferred call-level type arguments into a prepared
    /// type. The empty substitution (non-generic winner, or an enum
    /// method still mentioning its host's `Type::Param`s) is the
    /// identity — `instantiate_ty` requires every parameter to be
    /// bound and would reject those.
    fn substitute_call_level(&mut self, ty: TypeId, type_args: &[TypeId]) -> TypeId {
        if type_args.is_empty() {
            ty
        } else {
            self.instantiate_ty(ty, type_args)
        }
    }

    /// Whether a (declared, pre-instantiation) type mentions any
    /// `Type::Param`, recursively.
    fn mentions_type_param(&self, ty: TypeId) -> bool {
        match self.types[ty].clone() {
            Type::Param(_) => true,
            Type::Array(element) | Type::MutableArray(element) => self.mentions_type_param(element),
            Type::Enum(_, args) => args.iter().any(|&arg| self.mentions_type_param(arg)),
            Type::Tuple(elements) => elements
                .iter()
                .any(|&element| self.mentions_type_param(element)),
            _ => false,
        }
    }

    /// Quiet type-argument inference for applicability checks: the M3/// binding rules (`bind_type_args`) without diagnostics — a
    /// conflict or an unbound parameter returns `None` and makes the
    /// candidate inapplicable. Non-generic candidates trivially
    /// "infer" to no type arguments.
    fn try_infer_type_args(
        &self,
        candidate: &Candidate,
        arg_tys: &[TypeId],
    ) -> Option<Vec<TypeId>> {
        if candidate.type_param_count == 0 {
            return Some(Vec::new());
        }
        let mut bindings = vec![None; candidate.type_param_count];
        for (&param, &arg) in candidate.params.iter().zip(arg_tys) {
            if !self.try_bind(param, arg, &mut bindings) {
                return None;
            }
        }
        bindings.into_iter().collect()
    }

    /// One binding step of `try_infer_type_args` (see
    /// `bind_type_args` for the rules this mirrors).
    fn try_bind(&self, param_ty: TypeId, arg_ty: TypeId, bindings: &mut [Option<TypeId>]) -> bool {
        match (self.types[param_ty].clone(), self.types[arg_ty].clone()) {
            (Type::Param(index), _) => {
                let index = index as usize;
                match bindings[index] {
                    Some(existing) => self.types_equal(existing, arg_ty),
                    None => {
                        bindings[index] = Some(arg_ty);
                        true
                    }
                }
            }
            (Type::Enum(param_id, param_args), Type::Enum(arg_id, arg_args))
                if param_id == arg_id && param_args.len() == arg_args.len() =>
            {
                param_args
                    .iter()
                    .zip(arg_args)
                    .all(|(param, arg)| self.try_bind(*param, arg, bindings))
            }
            // Generic struct applications (M9): see `bind_type_args`.
            (Type::Struct(param_id), Type::Struct(arg_id)) if param_id == arg_id => {
                match (
                    self.generic_struct_args.get(&param_ty),
                    self.generic_struct_args.get(&arg_ty),
                ) {
                    (Some((_, param_args)), Some((_, arg_args)))
                        if param_args.len() == arg_args.len() =>
                    {
                        param_args
                            .iter()
                            .zip(arg_args.iter())
                            .all(|(param, arg)| self.try_bind(*param, *arg, bindings))
                    }
                    _ => true,
                }
            }
            (Type::Array(param), Type::Array(arg))
            | (Type::MutableArray(param), Type::MutableArray(arg)) => {
                self.try_bind(param, arg, bindings)
            }
            (Type::Tuple(params), Type::Tuple(args)) if params.len() == args.len() => params
                .iter()
                .zip(args)
                .all(|(param, arg)| self.try_bind(*param, arg, bindings)),
            _ => true,
        }
    }
}
