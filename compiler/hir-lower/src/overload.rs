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
    pub(crate) callee: hir::Callable,
    /// The arguments, lowered once and adapted (boxed where needed) to
    /// the winner's parameter types.
    pub(crate) args: Vec<hir::Expr>,
    pub(crate) return_ty: TypeId,
}

/// A candidate prepared for resolution: parameter and return types
/// still use the function's combined type-parameter namespace. A generic
/// receiver pre-binds the owner prefix; applicability infers the remaining
/// method suffix and substitutes the complete vector.
struct Candidate {
    function: FunctionId,
    params: Vec<TypeId>,
    return_ty: TypeId,
    /// Parameters declared by the function/method itself. Owner-only
    /// genericity does not make an otherwise concrete overload generic for
    /// MSC tie-breaking.
    own_type_param_count: usize,
    initial_bindings: Vec<Option<TypeId>>,
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
        // Context-independent arguments are shared by every candidate and
        // lowered once. `None`, empty arrays and context-dependent generic
        // constructors are postponed until inference provides a candidate
        // parameter type. Per-argument sinks preserve source evaluation
        // order even when later arguments are typed first.
        let mut lowered: Vec<Option<hir::Expr>> = (0..arg_exprs.len()).map(|_| None).collect();
        let mut arg_sinks: Vec<Vec<hir::Statement>> =
            (0..arg_exprs.len()).map(|_| Vec::new()).collect();
        for (index, arg) in arg_exprs.iter().enumerate() {
            if self.expr_requires_expected_type(arg) {
                continue;
            }
            lowered[index] = Some(self.lower_expr(arg, &mut arg_sinks[index], None)?);
        }
        let arg_tys: Vec<Option<TypeId>> = lowered
            .iter()
            .map(|arg| arg.as_ref().map(|arg| arg.ty))
            .collect();

        let prepared: Vec<Candidate> = candidates
            .iter()
            .map(|&function| {
                let sig = self.signatures[&function].clone();
                let parameterized = sig
                    .params
                    .iter()
                    .any(|param| self.mentions_type_param(param.ty));
                debug_assert_eq!(sig.owner_type_param_count, receiver_type_args.len());
                let mut initial_bindings = vec![None; sig.type_params.len()];
                for (binding, &ty) in initial_bindings.iter_mut().zip(receiver_type_args) {
                    *binding = Some(ty);
                }
                Candidate {
                    function,
                    params: sig.params.iter().map(|param| param.ty).collect(),
                    return_ty: sig.return_ty,
                    own_type_param_count: sig.type_params.len() - sig.owner_type_param_count,
                    initial_bindings,
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
            let matches = candidate.params.iter().zip(&arg_tys).zip(arg_exprs).all(
                |((&param, arg), arg_expr)| {
                    let expected = self.substitute_call_level(param, &type_args);
                    match arg {
                        Some(arg) => self.is_subtype(*arg, expected),
                        None => self.contextual_expr_accepts(arg_expr, expected),
                    }
                },
            );
            if matches {
                applicable.push((index, type_args));
            }
        }

        let (winner, type_args) = match applicable.len() {
            0 => {
                self.no_applicable_diagnostic(
                    name,
                    &prepared,
                    &arg_tys,
                    arg_exprs,
                    arg_exprs.len(),
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
                    self.lower_expr(&arg_exprs[index], &mut arg_sinks[index], Some(expected))?
                }
            };
            if !self.is_subtype(arg.ty, expected) {
                self.no_applicable_diagnostic(
                    name,
                    &prepared,
                    &arg_tys,
                    arg_exprs,
                    arg_exprs.len(),
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
        // M9: the GC intrinsics constrain their type argument to
        // reference types (see `check_gc_ref_constraint`).
        if !self.check_gc_ref_constraint(function, &type_args, &args) {
            return None;
        }
        // The complete owner-prefix plus method-suffix vector identifies
        // the resolved generic entity stored on the HIR call.
        let callee = if !type_args.is_empty() {
            hir::Callable::Generic(self.record_instantiation(function, type_args.clone()))
        } else {
            hir::Callable::Function(function)
        };
        Some(ResolvedCallee {
            callee,
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
                candidate.own_type_param_count == 0 && !candidate.parameterized
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
        arg_tys: &[Option<TypeId>],
        arg_exprs: &[ast::Expr],
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
        let found: Vec<String> = arg_tys
            .iter()
            .zip(arg_exprs)
            .map(|(ty, expr)| match ty {
                Some(ty) => self.type_name(*ty),
                None => contextual_expr_name(expr),
            })
            .collect();
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
            Type::Enum(_, args) | Type::Struct(_, args) | Type::Interface(_, args) => {
                args.iter().any(|&arg| self.mentions_type_param(arg))
            }
            Type::Tuple(elements) => elements
                .iter()
                .any(|&element| self.mentions_type_param(element)),
            Type::Function(id) => {
                let function = &self.function_types[id];
                function
                    .parameter_types
                    .iter()
                    .any(|&parameter| self.mentions_type_param(parameter))
                    || self.mentions_type_param(function.return_type)
            }
            _ => false,
        }
    }

    /// Quiet type-argument inference for applicability checks: the M3/// binding rules (`bind_type_args`) without diagnostics — a
    /// conflict or an unbound parameter returns `None` and makes the
    /// candidate inapplicable. Non-generic candidates trivially
    /// "infer" to no type arguments.
    fn try_infer_type_args(
        &mut self,
        candidate: &Candidate,
        arg_tys: &[Option<TypeId>],
    ) -> Option<Vec<TypeId>> {
        if candidate.initial_bindings.is_empty() {
            return Some(Vec::new());
        }
        let mut bindings = candidate.initial_bindings.clone();
        for (&param, arg) in candidate.params.iter().zip(arg_tys) {
            let Some(arg) = *arg else {
                continue;
            };
            if !self.try_bind(param, arg, &mut bindings) {
                return None;
            }
        }
        bindings.into_iter().collect()
    }

    /// One binding step of `try_infer_type_args` (see
    /// `bind_type_args` for the rules this mirrors).
    fn try_bind(
        &mut self,
        param_ty: TypeId,
        arg_ty: TypeId,
        bindings: &mut [Option<TypeId>],
    ) -> bool {
        match (self.types[param_ty].clone(), self.types[arg_ty].clone()) {
            (Type::Param(index), _) => {
                let index = index.into_raw() as usize;
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
            (Type::Struct(param_id, param_args), Type::Struct(arg_id, arg_args))
                if param_id == arg_id && param_args.len() == arg_args.len() =>
            {
                param_args
                    .iter()
                    .zip(arg_args.iter())
                    .all(|(param, arg)| self.try_bind(*param, *arg, bindings))
            }
            (Type::Interface(param_id, param_args), Type::Interface(arg_id, arg_args))
                if param_id == arg_id && param_args.len() == arg_args.len() =>
            {
                param_args
                    .iter()
                    .zip(arg_args.iter())
                    .all(|(param, arg)| self.try_bind(*param, *arg, bindings))
            }
            (Type::Interface(param_id, param_args), _) => {
                let Some(arg_args) = self.implemented_interface_application(arg_ty, param_id)
                else {
                    return true;
                };
                param_args.len() == arg_args.len()
                    && param_args
                        .iter()
                        .zip(arg_args)
                        .all(|(param, arg)| self.try_bind(*param, arg, bindings))
            }
            (Type::Array(param), Type::Array(arg))
            | (Type::MutableArray(param), Type::MutableArray(arg)) => {
                self.try_bind(param, arg, bindings)
            }
            (Type::Tuple(params), Type::Tuple(args)) if params.len() == args.len() => params
                .iter()
                .zip(args)
                .all(|(param, arg)| self.try_bind(*param, arg, bindings)),
            (Type::Function(param_id), Type::Function(arg_id)) => {
                let param = self.function_types[param_id].clone();
                let arg = self.function_types[arg_id].clone();
                param.is_suspend == arg.is_suspend
                    && param.parameter_types.len() == arg.parameter_types.len()
                    && param
                        .parameter_types
                        .iter()
                        .zip(arg.parameter_types)
                        .all(|(param, arg)| self.try_bind(*param, arg, bindings))
                    && self.try_bind(param.return_type, arg.return_type, bindings)
            }
            _ => true,
        }
    }
}

fn contextual_expr_name(expr: &ast::Expr) -> String {
    match expr {
        ast::Expr::Var(name) if name.text == "None" => "None".to_string(),
        ast::Expr::ArrayLiteral { elements, .. } if elements.is_empty() => "[]".to_string(),
        _ => "context-dependent expression".to_string(),
    }
}
