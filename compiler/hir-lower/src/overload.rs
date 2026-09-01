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

/// A candidate prepared for resolution: parameter and return types
/// still use the function's combined type-parameter namespace. A generic
/// receiver pre-binds the owner prefix; applicability infers the remaining
/// method suffix and substitutes the complete vector.
struct Candidate {
    function: FunctionId,
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
            .map(|function| CallableCandidate::direct(function, receiver_type_args.to_vec()))
            .collect::<Vec<_>>();
        self.resolve_overload_with_receiver(
            name,
            &candidates,
            OverloadReceiver::Ordinary,
            call,
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
            OverloadReceiver::Ordinary,
            call,
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
            .map(|function| CallableCandidate::direct(function, Vec::new()))
            .collect::<Vec<_>>();
        self.resolve_overload_with_receiver(
            name,
            &candidates,
            OverloadReceiver::Extension(receiver),
            call,
            sink,
        )
    }

    fn resolve_overload_with_receiver(
        &mut self,
        name: &str,
        candidates: &[CallableCandidate],
        receiver: OverloadReceiver,
        call: OverloadCall<'_>,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<ResolvedCallee> {
        let OverloadCall {
            explicit_type_args,
            arg_exprs,
            span,
        } = call;
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
        let mut lowered: Vec<Option<hir::Expr>> =
            Vec::with_capacity(receiver_offset + arg_exprs.len());
        if let Some(receiver) = receiver {
            lowered.push(Some(receiver));
        }
        lowered.extend((0..arg_exprs.len()).map(|_| None));
        let mut arg_sinks: Vec<Vec<hir::Statement>> =
            (0..arg_exprs.len()).map(|_| Vec::new()).collect();
        for (index, arg) in arg_exprs.iter().enumerate() {
            if self.expr_requires_expected_type(arg) {
                continue;
            }
            lowered[receiver_offset + index] =
                Some(self.lower_expr(arg, &mut arg_sinks[index], None)?);
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
                debug_assert_eq!(sig.owner_type_param_count, source.owner_arguments.len());
                let mut initial_bindings = vec![None; sig.type_params.len()];
                for (binding, &ty) in initial_bindings.iter_mut().zip(&source.owner_arguments) {
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
            if candidate.params.len() != receiver_offset + arg_exprs.len() {
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
                if let Err(reason) =
                    self.probe_contextual_expr(&arg_exprs[source_argument], expected)
                {
                    contextual_failures.push((source_argument, expected, reason));
                    contextual_args_match = false;
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
                if self.contextual_no_applicable_diagnostic(name, arg_exprs, &contextual_failures) {
                    return None;
                }
                self.no_applicable_diagnostic(
                    name,
                    &prepared,
                    &arg_tys[receiver_offset..],
                    arg_exprs,
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
                    arg_exprs,
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
        let callee = if !type_args.is_empty() {
            hir::Callable::Generic(self.record_instantiation(function, type_args.clone()))
        } else {
            hir::Callable::Function(function)
        };
        Some(ResolvedCallee {
            callee,
            source: candidate.source,
            type_args,
            args,
            return_ty,
        })
    }

    fn contextual_no_applicable_diagnostic(
        &mut self,
        name: &str,
        arg_exprs: &[ast::Expr],
        failures: &[(usize, TypeId, String)],
    ) -> bool {
        let Some(argument) = failures.iter().map(|failure| failure.0).min() else {
            return false;
        };
        let mut details = Vec::new();
        for (_, expected, reason) in failures.iter().filter(|failure| failure.0 == argument) {
            let detail = format!("{}: {reason}", self.type_name(*expected));
            if !details.contains(&detail) {
                details.push(detail);
            }
        }
        self.error(
            arg_exprs[argument].span(),
            format!(
                "{} does not match any overload of `{name}`; candidate expectations: {}",
                contextual_expr_name(&arg_exprs[argument]),
                details.join("; ")
            ),
        );
        true
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
        extension_receiver: Option<TypeId>,
        span: Span,
    ) {
        let hidden_argument_count = usize::from(extension_receiver.is_some());
        let supplied = arg_exprs.len();
        let uniform_arity = prepared[0].params.len() - hidden_argument_count;
        if prepared
            .iter()
            .all(|candidate| candidate.params.len() - hidden_argument_count == uniform_arity)
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
        let message = match extension_receiver {
            Some(receiver) => format!(
                "no overload of extension `{name}` matches receiver type {} and argument types ({})",
                self.type_name(receiver),
                found.join(", ")
            ),
            None => format!(
                "no overload of `{name}` matches argument types ({})",
                found.join(", ")
            ),
        };
        self.error(span, message);
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
            Type::Ptr(pointee) => self.mentions_type_param(pointee),
            Type::Enum(application) => self.enum_applications[application]
                .arguments
                .iter()
                .any(|&arg| self.mentions_type_param(arg)),
            Type::Struct(application) => self.struct_applications[application]
                .arguments
                .iter()
                .any(|&arg| self.mentions_type_param(arg)),
            Type::Class(application) => self.class_applications[application]
                .arguments
                .iter()
                .any(|&arg| self.mentions_type_param(arg)),
            Type::Interface(application) => self.interface_applications[application]
                .arguments
                .iter()
                .any(|&arg| self.mentions_type_param(arg)),
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
            Type::FunPtr(id) => {
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
    pub(crate) fn try_bind(
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
            (Type::Enum(param), Type::Enum(arg)) => {
                let param = self.enum_applications[param].clone();
                let arg = self.enum_applications[arg].clone();
                param.template == arg.template
                    && param.arguments.len() == arg.arguments.len()
                    && param
                        .arguments
                        .iter()
                        .zip(arg.arguments)
                        .all(|(param, arg)| self.try_bind(*param, arg, bindings))
            }
            (Type::Struct(param), Type::Struct(arg)) => {
                let param = self.struct_applications[param].clone();
                let arg = self.struct_applications[arg].clone();
                param.template == arg.template
                    && param.arguments.len() == arg.arguments.len()
                    && param
                        .arguments
                        .iter()
                        .zip(arg.arguments)
                        .all(|(param, arg)| self.try_bind(*param, arg, bindings))
            }
            (Type::Class(param), Type::Class(arg)) => {
                let param = self.class_applications[param].clone();
                let arg = self.class_applications[arg].clone();
                param.template == arg.template
                    && param.arguments.len() == arg.arguments.len()
                    && param
                        .arguments
                        .iter()
                        .zip(arg.arguments)
                        .all(|(param, arg)| self.try_bind(*param, arg, bindings))
            }
            (Type::Interface(param), Type::Interface(arg)) => {
                let param = self.interface_applications[param].clone();
                let arg = self.interface_applications[arg].clone();
                param.template == arg.template
                    && param.arguments.len() == arg.arguments.len()
                    && param
                        .arguments
                        .iter()
                        .zip(arg.arguments)
                        .all(|(param, arg)| self.try_bind(*param, arg, bindings))
            }
            (Type::Interface(param), _) => {
                let param = self.interface_applications[param].clone();
                let Some(arg_args) = self.implemented_interface_application(arg_ty, param.template)
                else {
                    return true;
                };
                param.arguments.len() == arg_args.len()
                    && param
                        .arguments
                        .iter()
                        .zip(arg_args)
                        .all(|(param, arg)| self.try_bind(*param, arg, bindings))
            }
            (Type::Array(param), Type::Array(arg))
            | (Type::MutableArray(param), Type::MutableArray(arg)) => {
                self.try_bind(param, arg, bindings)
            }
            (Type::Ptr(param), Type::Ptr(arg)) => self.try_bind(param, arg, bindings),
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
            (Type::FunPtr(param_id), Type::FunPtr(arg_id)) => {
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
        ast::Expr::Lambda { .. } => "lambda".to_string(),
        ast::Expr::AnonymousFunction { .. } => "anonymous function".to_string(),
        ast::Expr::CallableReference { .. } => "callable reference".to_string(),
        _ => "context-dependent expression".to_string(),
    }
}
