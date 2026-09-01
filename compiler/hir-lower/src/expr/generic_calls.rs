//! Generic call resolution, argument inference and concrete callable binding.

use super::*;

impl Lowerer {
    /// A bare call `f(args)` (M7): the candidate layers are, in order,
    /// the current host's methods (inside a member function, where
    /// `f(...)` means `this.f(...)`), the top-level functions declared
    /// on the call site's own side of the core/user boundary (its
    /// "same package" layer), and the other side (the implicitly
    /// imported layer) — the first layer containing any candidate wins
    /// whole (milestone7 DESIGN.md 1.2). The layering is relative to
    /// the call site's file: for a user-file call that is user
    /// top-level → core, for a core-file call core → user, so a user
    /// declaration shadows core overloads for user code without
    /// breaking the core library's own internal calls. (M13's
    /// multi-Cone package system will redefine these layers per
    /// package/Cone.) A single candidate keeps the pre-M7 path
    /// (`finish_single_function_call` / `finish_method_call`) so its
    /// diagnostics stay intact; several candidates go through
    /// `resolve_overload`.
    pub(super) fn lower_function_call(
        &mut self,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let name = call.callee.text.clone();

        // Layer 1: the nearest lexical block containing local functions of
        // this name. The whole overload set shadows members and top-level
        // functions, and declarations enter it only as they are encountered.
        let local_candidates = self.local_function_scopes.lookup(&name);
        if !local_candidates.is_empty() {
            if local_candidates.len() == 1 {
                return self.finish_local_function_call(local_candidates[0], call, sink);
            }
            let functions: Vec<_> = local_candidates
                .iter()
                .map(|id| self.local_functions[*id].function)
                .collect();
            let owner_count = self.local_functions[local_candidates[0]].owner_type_param_count;
            let owner_type_args = self.ambient_type_args(owner_count);
            let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
            let resolved = self.resolve_overload(
                &name,
                &functions,
                &owner_type_args,
                crate::overload::OverloadCall {
                    explicit_type_args: &explicit_type_args,
                    arg_exprs: &call.args,
                    span: call.span,
                },
                sink,
            )?;
            let function = self.callable_function_id(resolved.callee);
            let local_function = self.local_function_by_function[&function];
            let captures = self.local_call_capture_args(local_function, call.span)?;
            self.check_call_effects(resolved.callee, call.span);
            return Some(hir::Expr {
                kind: ExprKind::LocalFunctionCall {
                    local_function,
                    callee: resolved.callee,
                    captures,
                    args: resolved.args,
                },
                ty: resolved.return_ty,
                span: call.span,
            });
        }

        // Layer 2: members of the current host.
        let mut members = self
            .current_this_ty()
            .map(|host_ty| self.methods_by_name(host_ty, &name))
            .unwrap_or_default();
        if !members.is_empty() {
            let receiver = self
                .lower_current_this(call.callee.span)
                .expect("a member callable body always has a lexical `this`");
            if members.len() == 1 {
                return self.finish_method_call(
                    members.remove(0),
                    receiver,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    sink,
                );
            }
            return self.finish_overloaded_method_call(
                members,
                &name,
                receiver,
                CallSite {
                    type_args: &call.type_args,
                    args: &call.args,
                    span: call.span,
                },
                sink,
            );
        }

        // An extension body has a lexical `this` just like a member body.
        // If no real member wins, another visible extension may use it as
        // the implicit receiver before ordinary top-level functions.
        if self.current_this_ty().is_some() {
            let extensions = self.extension_candidate_layer(&name);
            if !extensions.is_empty() {
                let receiver = self
                    .lower_current_this(call.callee.span)
                    .expect("a lexical receiver has a `this` value");
                return self.finish_extension_call(
                    &extensions,
                    &name,
                    receiver,
                    CallSite {
                        type_args: &call.type_args,
                        args: &call.args,
                        span: call.span,
                    },
                    sink,
                );
            }
        }

        // Layers 2 and 3, relative to the call site's file: the
        // declarations on the call site's own side of the core/user
        // boundary come first, the other side is the implicitly
        // imported layer.
        let candidates = self.top_level_candidate_layer(&name);
        if candidates.is_empty() {
            self.error(
                call.callee.span,
                format!("unknown function `{}`", call.callee.text),
            );
            return None;
        }
        if candidates.len() == 1 {
            return self.finish_single_function_call(candidates[0], call, sink);
        }
        let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
        let resolved = self.resolve_overload(
            &name,
            &candidates,
            &[],
            crate::overload::OverloadCall {
                explicit_type_args: &explicit_type_args,
                arg_exprs: &call.args,
                span: call.span,
            },
            sink,
        )?;
        let ty = resolved.return_ty;
        self.check_call_effects(resolved.callee, call.span);
        Some(hir::Expr {
            kind: ExprKind::Call {
                callee: resolved.callee,
                args: resolved.args,
            },
            ty,
            span: call.span,
        })
    }

    pub(super) fn ambient_type_args(&mut self, count: usize) -> Vec<TypeId> {
        self.type_params_in_scope[..count]
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>()
            .into_iter()
            .map(|parameter| self.intern_type(Type::Param(parameter)))
            .collect()
    }

    pub(super) fn resolve_call_type_args(&mut self, refs: &[ast::TypeRef]) -> Option<Vec<TypeId>> {
        refs.iter()
            .map(|type_ref| self.resolve_type_ref(type_ref))
            .collect()
    }

    /// Seed the call's own generic suffix. Receiver/lexical-owner type
    /// parameters occupy the prefix and are never repeated at the call site.
    /// Scoop requires either no explicit arguments (infer the whole suffix)
    /// or the complete suffix; partial explicit lists are intentionally not
    /// ambiguous with inference.
    pub(super) fn bind_explicit_type_args(
        &mut self,
        bindings: &mut [Option<TypeId>],
        owner_type_param_count: usize,
        explicit: &[TypeId],
        span: Span,
        target: &str,
    ) -> bool {
        if explicit.is_empty() {
            return true;
        }
        let expected = bindings.len() - owner_type_param_count;
        if explicit.len() != expected {
            self.error(
                span,
                format!(
                    "{target} takes exactly {expected} type argument(s), but {} were supplied",
                    explicit.len()
                ),
            );
            return false;
        }
        for (binding, &ty) in bindings[owner_type_param_count..].iter_mut().zip(explicit) {
            *binding = Some(ty);
        }
        true
    }

    pub(super) fn local_call_capture_args(
        &mut self,
        local_function: hir::LocalFunctionId,
        span: Span,
    ) -> Option<Vec<hir::Expr>> {
        let captures: Vec<_> = self.local_functions[local_function]
            .captures
            .iter()
            .map(|capture| (capture.binding, capture.name.clone(), capture.ty))
            .collect();
        let mut args = Vec::with_capacity(captures.len());
        for (binding, name, ty) in captures {
            if let Some((local, _)) = self
                .locals
                .iter()
                .find(|(_, candidate)| candidate.binding == binding)
            {
                args.push(hir::Expr {
                    kind: ExprKind::Local(local),
                    ty,
                    span,
                });
                continue;
            }
            args.push(self.lower_capture_binding(binding, &name, span)?);
        }
        Some(args)
    }

    pub(super) fn finish_local_function_call(
        &mut self,
        local_function: hir::LocalFunctionId,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let function = self.local_functions[local_function].function;
        let sig = self.signatures[&function].clone();
        if sig.params.len() != call.args.len() {
            let expected = sig.params.len();
            let supplied = call.args.len();
            let noun = if expected == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                call.span,
                format!(
                    "local function `{}` takes exactly {expected} {noun}, but {supplied} were supplied",
                    call.callee.text
                ),
            );
            return None;
        }
        let owner_type_args = self.ambient_type_args(sig.owner_type_param_count);
        let mut bindings = vec![None; sig.type_params.len()];
        for (binding, ty) in bindings.iter_mut().zip(owner_type_args) {
            *binding = Some(ty);
        }
        let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
        if !self.bind_explicit_type_args(
            &mut bindings,
            sig.owner_type_param_count,
            &explicit_type_args,
            call.span,
            &format!("local function `{}`", call.callee.text),
        ) {
            return None;
        }
        let param_tys: Vec<_> = sig.params.iter().map(|param| param.ty).collect();
        let inferred =
            self.lower_inference_args(&call.args, &param_tys, bindings, &sig.type_params)?;
        let mut type_args = Vec::with_capacity(inferred.bindings.len());
        for (binding, param) in inferred.bindings.iter().copied().zip(&sig.type_params) {
            let Some(ty) = binding else {
                self.error(
                    call.span,
                    format!(
                        "cannot infer type argument `{}` for local function `{}`",
                        param.name, call.callee.text
                    ),
                );
                return None;
            };
            type_args.push(ty);
        }
        if !self.check_type_argument_kinds(
            &sig.type_params,
            &type_args,
            call.span,
            &format!("local function `{}`", call.callee.text),
        ) {
            return None;
        }
        let args = inferred.finish(sink);
        let mut adapted = Vec::with_capacity(args.len());
        for (param, arg) in sig.params.iter().zip(args) {
            let expected = self.instantiate_ty(param.ty, &type_args);
            if !self.is_subtype(arg.ty, expected) {
                let expected_name = self.type_name(expected);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for parameter `{}` of local function `{}` must be of type {expected_name}, found {found}",
                        param.name.text, call.callee.text
                    ),
                );
                return None;
            }
            adapted.push(self.adapt_to(arg, expected));
        }
        let return_ty = self.instantiate_ty(sig.return_ty, &type_args);
        let callee = if type_args.is_empty() {
            hir::Callable::Function(function)
        } else {
            hir::Callable::Generic(self.record_instantiation(function, type_args))
        };
        let captures = self.local_call_capture_args(local_function, call.span)?;
        self.check_call_effects(callee, call.span);
        Some(hir::Expr {
            kind: ExprKind::LocalFunctionCall {
                local_function,
                callee,
                captures,
                args: adapted,
            },
            ty: return_ty,
            span: call.span,
        })
    }

    /// A call to the single candidate of its layer: arity and
    /// argument-type diagnostics name the function directly, and the
    /// parameter types serve as expected-type hints for the arguments
    /// (this is what types `None` in argument position). Type-argument
    /// inference for generic callees follows the M3 rules.
    pub(super) fn finish_single_function_call(
        &mut self,
        function: hir::FunctionId,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        if self.ffi_core.is_some_and(|core| {
            function == core.address_of || function == core.size_of || function == core.align_of
        }) {
            return self.lower_pointer_top_level_intrinsic(function, call);
        }
        let name = self.functions[function].name.clone();

        let sig = self.signatures[&function].clone();
        if sig.params.len() != call.args.len() {
            let expected = sig.params.len();
            let supplied = call.args.len();
            let noun = if expected == 1 {
                "argument"
            } else {
                "arguments"
            };
            self.error(
                call.span,
                format!(
                    "function `{name}` takes exactly {expected} {noun}, but {supplied} were supplied"
                ),
            );
            return None;
        }

        // Infer to a fixed point: context-independent arguments may bind
        // parameters needed to type earlier `None` / empty-array arguments.
        // Each argument owns a temporary desugaring sink; the sinks are
        // concatenated in source order after inference, preserving runtime
        // evaluation order even when typing happens in a different order.
        let param_tys: Vec<TypeId> = sig.params.iter().map(|param| param.ty).collect();
        let mut bindings = vec![None; sig.type_params.len()];
        let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
        if !self.bind_explicit_type_args(
            &mut bindings,
            sig.owner_type_param_count,
            &explicit_type_args,
            call.span,
            &format!("function `{name}`"),
        ) {
            return None;
        }
        let inferred =
            self.lower_inference_args(&call.args, &param_tys, bindings, &sig.type_params)?;
        let mut type_args = Vec::with_capacity(inferred.bindings.len());
        for (binding, param) in inferred.bindings.iter().copied().zip(&sig.type_params) {
            match binding {
                Some(ty) => type_args.push(ty),
                None => {
                    self.error(
                        call.span,
                        format!("cannot infer type argument `{}` for `{name}`", param.name),
                    );
                    return None;
                }
            }
        }
        if !self.check_type_argument_kinds(
            &sig.type_params,
            &type_args,
            call.span,
            &format!("function `{name}`"),
        ) {
            return None;
        }
        let args = inferred.finish(sink);

        // Argument types must be subtypes of the (instantiated)
        // parameter types; the adaptation boxes value types crossing
        // into `Any` / an interface (M6).
        let mut adapted_args = Vec::with_capacity(args.len());
        for (param, arg) in sig.params.iter().zip(args) {
            let expected = self.instantiate_ty(param.ty, &type_args);
            if !self.is_subtype(arg.ty, expected) {
                let param_name = param.name.text.clone();
                let expected_name = self.type_name(expected);
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!(
                        "argument for parameter `{param_name}` of `{name}` must be of type {expected_name}, found {found}"
                    ),
                );
                return None;
            }
            adapted_args.push(self.adapt_to(arg, expected));
        }
        let ty = self.instantiate_ty(sig.return_ty, &type_args);

        // Every successful generic call (including calls inside generic
        // function bodies, whose type arguments may still mention
        // `Type::Param`) requests an instantiation; mir-lower
        // materializes them.
        let callee = if type_args.is_empty() {
            hir::Callable::Function(function)
        } else {
            hir::Callable::Generic(self.record_instantiation(function, type_args))
        };

        self.check_call_effects(callee, call.span);

        Some(hir::Expr {
            kind: ExprKind::Call {
                callee,
                args: adapted_args,
            },
            ty,
            span: call.span,
        })
    }

    pub(super) fn lower_pointer_top_level_intrinsic(
        &mut self,
        function: hir::FunctionId,
        call: &ast::CallExpr,
    ) -> Option<hir::Expr> {
        let core = self
            .ffi_core
            .expect("pointer core is validated before bodies are lowered");
        if function == core.address_of {
            if call.args.len() != 1 {
                self.error(
                    call.span,
                    format!(
                        "function `addressOf` takes exactly 1 argument, but {} were supplied",
                        call.args.len()
                    ),
                );
                return None;
            }
            let place = match &call.args[0] {
                ast::Expr::Var(name) => self
                    .scopes
                    .lookup(&name.text)
                    .map(|local| (hir::Place::Local(local), self.locals[local].ty, name.span))
                    .or_else(|| {
                        self.globals_by_name.get(&name.text).copied().map(|global| {
                            (
                                hir::Place::Global(global),
                                self.globals[global].ty,
                                name.span,
                            )
                        })
                    }),
                ast::Expr::This { span } => self
                    .current_this
                    .map(|(local, ty)| (hir::Place::Local(local), ty, *span)),
                expression => {
                    self.error(
                        expression.span(),
                        "`addressOf` argument must be an addressable local, parameter, global, or value-type `this`"
                            .to_string(),
                    );
                    return None;
                }
            };
            let Some((place, place_ty, place_span)) = place else {
                self.error(
                    call.args[0].span(),
                    "`addressOf` argument must be an addressable local, parameter, global, or value-type `this`"
                        .to_string(),
                );
                return None;
            };
            if let hir::Place::Global(global) = place
                && matches!(
                    self.globals[global].storage,
                    hir::GlobalStorage::Extern { .. }
                )
            {
                self.require_unsafe_operation(place_span, "taking the address of an extern global");
            }
            let explicit = self.resolve_call_type_args(&call.type_args)?;
            if explicit.len() > 1 {
                self.error(
                    call.span,
                    format!(
                        "function `addressOf` takes exactly 1 type argument, but {} were supplied",
                        explicit.len()
                    ),
                );
                return None;
            }
            if explicit
                .first()
                .is_some_and(|explicit| !self.types_equal(*explicit, place_ty))
            {
                self.error(
                    place_span,
                    format!(
                        "`addressOf` type argument must match the place type {}, found {}",
                        self.type_name(place_ty),
                        self.type_name(explicit[0])
                    ),
                );
                return None;
            }
            if !self.is_value_ty(place_ty)
                || self.type_contains_param(place_ty)
                || !self.is_gc_free(place_ty)
            {
                self.error(
                    place_span,
                    format!(
                        "`addressOf` requires a concrete GC-free value type, found {}",
                        self.type_name(place_ty)
                    ),
                );
                return None;
            }
            self.check_call_effects(hir::Callable::Function(function), call.span);
            let ty = self.intern_type(Type::Ptr(place_ty));
            return Some(hir::Expr {
                kind: ExprKind::AddressOf(place),
                ty,
                span: call.span,
            });
        }

        if !call.args.is_empty() {
            let name = if function == core.size_of {
                "sizeOf"
            } else {
                "alignOf"
            };
            self.error(
                call.span,
                format!(
                    "function `{name}` takes exactly 0 arguments, but {} were supplied",
                    call.args.len()
                ),
            );
            return None;
        }
        let explicit = self.resolve_call_type_args(&call.type_args)?;
        if explicit.len() != 1 {
            let name = if function == core.size_of {
                "sizeOf"
            } else {
                "alignOf"
            };
            self.error(
                call.span,
                format!(
                    "function `{name}` requires exactly 1 explicit type argument, but {} were supplied",
                    explicit.len()
                ),
            );
            return None;
        }
        let target = explicit[0];
        if !self.is_value_ty(target) || self.type_contains_param(target) || !self.is_gc_free(target)
        {
            self.error(
                call.span,
                format!(
                    "memory layout requires a concrete GC-free value type, found {}",
                    self.type_name(target)
                ),
            );
            return None;
        }
        let kind = if function == core.size_of {
            ExprKind::SizeOf(target)
        } else {
            ExprKind::AlignOf(target)
        };
        Some(hir::Expr {
            kind,
            ty: self.uint,
            span: call.span,
        })
    }

    /// Lower generic-call/constructor arguments to a fixed point. An
    /// expression that intrinsically needs an expected type is postponed
    /// while its parameter still contains an unbound type variable; other
    /// arguments can then add bindings independently of their source order.
    pub(super) fn lower_inference_args(
        &mut self,
        arg_exprs: &[ast::Expr],
        param_tys: &[TypeId],
        mut bindings: Vec<Option<TypeId>>,
        type_params: &[hir::TypeParamDecl],
    ) -> Option<InferredArguments> {
        let mut args: Vec<Option<hir::Expr>> = (0..arg_exprs.len()).map(|_| None).collect();
        let mut sinks: Vec<Vec<hir::Statement>> =
            (0..arg_exprs.len()).map(|_| Vec::new()).collect();
        loop {
            let mut progress = false;
            for index in 0..arg_exprs.len() {
                if args[index].is_some() {
                    continue;
                }
                let hint = self.try_substitute(param_tys[index], &bindings);
                if hint.is_none() && self.expr_requires_expected_type(&arg_exprs[index]) {
                    continue;
                }
                let arg = self.lower_expr(&arg_exprs[index], &mut sinks[index], hint)?;
                if !self.bind_type_args(
                    param_tys[index],
                    arg.ty,
                    &mut bindings,
                    type_params,
                    arg.span,
                ) {
                    return None;
                }
                args[index] = Some(arg);
                progress = true;
            }
            if args.iter().all(Option::is_some) {
                break;
            }
            if !progress {
                // No later constraint could type the first deferred
                // expression. Lower it without a hint to retain the
                // focused diagnostic (`cannot infer the type of None`,
                // empty-array element type, and so on).
                let index = args
                    .iter()
                    .position(Option::is_none)
                    .expect("an unresolved argument remains");
                let arg = self.lower_expr(&arg_exprs[index], &mut sinks[index], None)?;
                if !self.bind_type_args(
                    param_tys[index],
                    arg.ty,
                    &mut bindings,
                    type_params,
                    arg.span,
                ) {
                    return None;
                }
                args[index] = Some(arg);
            }
        }
        Some(InferredArguments {
            args,
            bindings,
            sinks,
        })
    }

    /// Expressions whose type cannot be synthesized without context. Calls
    /// to generic constructors are contextual only when their own
    /// context-independent arguments cannot bind every constructor variable;
    /// this lets nested calls perform their own fixed-point inference.
    pub(crate) fn expr_requires_expected_type(&self, expr: &ast::Expr) -> bool {
        match expr {
            ast::Expr::Var(name) => {
                name.text == "None"
                    && self.scopes.lookup(&name.text).is_none()
                    && !self.host_has_property(&name.text)
            }
            ast::Expr::FieldAccess(access) => self.unit_variant_from_field(access).is_some(),
            ast::Expr::TupleLiteral { elements, .. } | ast::Expr::ArrayLiteral { elements, .. } => {
                elements.is_empty()
                    || elements
                        .iter()
                        .any(|element| self.expr_requires_expected_type(element))
            }
            ast::Expr::Call(call) if !call.type_args.is_empty() => false,
            ast::Expr::Call(call) => self.constructor_requires_expected(&call.callee, &call.args),
            ast::Expr::StructInit { name, args, .. } => {
                self.constructor_requires_expected(name, args)
            }
            ast::Expr::MethodCall {
                receiver,
                name,
                type_args,
                args,
                ..
            } => {
                type_args.is_empty()
                    && self.qualified_variant_requires_expected(receiver, name, args)
            }
            // A lambda with an explicit, fully typed parameter header can
            // synthesize its own function type and therefore participate in
            // generic inference before an overload is selected. Untyped or
            // omitted parameters remain contextual and are probed against
            // each candidate transactionally.
            ast::Expr::Lambda {
                parameters: Some(parameters),
                ..
            } if parameters.iter().all(|parameter| parameter.ty.is_some()) => false,
            ast::Expr::Lambda { .. }
            | ast::Expr::AnonymousFunction { .. }
            | ast::Expr::CallableReference { .. } => true,
            // Structured expressions perform their own branch-level fixed
            // point and therefore do not need to be postponed as a whole.
            ast::Expr::If(_) | ast::Expr::When(_) | ast::Expr::Try(_) => false,
            _ => false,
        }
    }

    /// Type one context-dependent expression in a cloned semantic state. This is
    /// the transactional probe used by overload applicability: generated
    /// function/closure entities, inferred types, captures, and diagnostics are
    /// all discarded with the clone. The selected candidate is lowered once in
    /// the original state afterwards.
    pub(crate) fn probe_contextual_expr(
        &self,
        expr: &ast::Expr,
        expected: TypeId,
    ) -> Result<(), String> {
        let mut probe = self.clone();
        let diagnostics_before = probe.diagnostics.len();
        let mut sink = Vec::new();
        let value = probe.lower_expr(expr, &mut sink, Some(expected));
        let diagnostics: Vec<_> = probe.diagnostics[diagnostics_before..]
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect();
        let Some(value) = value else {
            return Err(if diagnostics.is_empty() {
                "contextual expression could not be typed".to_string()
            } else {
                diagnostics.join(", ")
            });
        };
        if !diagnostics.is_empty() {
            return Err(diagnostics.join(", "));
        }
        if !probe.is_subtype(value.ty, expected) {
            return Err(format!(
                "expression has type {}, expected {}",
                probe.type_name(value.ty),
                probe.type_name(expected)
            ));
        }
        Ok(())
    }

    pub(super) fn constructor_requires_expected(
        &self,
        name: &ast::Ident,
        args: &[ast::Expr],
    ) -> bool {
        let Some((type_param_count, fields)) = self.constructor_inference_shape(&name.text) else {
            return false;
        };
        if type_param_count == 0 {
            return false;
        }
        let mut bound = vec![false; type_param_count];
        for (arg, field) in args.iter().zip(fields) {
            if !self.expr_requires_expected_type(arg) {
                self.mark_type_params(field, &mut bound);
            }
        }
        bound.iter().any(|bound| !bound)
    }

    pub(super) fn qualified_variant_requires_expected(
        &self,
        receiver: &ast::Expr,
        name: &ast::Ident,
        args: &[ast::Expr],
    ) -> bool {
        let ast::Expr::Var(enum_name) = receiver else {
            return false;
        };
        if self.scopes.lookup(&enum_name.text).is_some() || self.host_has_property(&enum_name.text)
        {
            return false;
        }
        let qualified = format!("{}.{}", enum_name.text, name.text);
        self.constructor_requires_expected(
            &ast::Ident {
                text: qualified,
                span: Span::new(enum_name.span.start, name.span.end),
            },
            args,
        )
    }

    /// Generic constructor parameter count and field templates, without
    /// producing diagnostics. `None` means the name is a function/class or
    /// does not denote a constructor.
    pub(super) fn constructor_inference_shape(&self, name: &str) -> Option<(usize, Vec<TypeId>)> {
        let variant = if let Some((enum_name, variant_name)) = name.split_once('.') {
            let enum_id = self.enums_by_name.get(enum_name).copied()?;
            self.find_variant(enum_id, variant_name)
                .map(|variant| (enum_id, variant))
        } else {
            self.option_variant(name)
        };
        if let Some((enum_id, variant)) = variant {
            return Some((
                self.enums[enum_id].type_params.len(),
                self.enums[enum_id].variants[variant as usize]
                    .fields
                    .iter()
                    .map(|field| field.ty)
                    .collect(),
            ));
        }
        self.structs_by_name
            .get(name)
            .map(|(struct_id, _)| {
                (
                    self.structs[*struct_id].type_params.len(),
                    self.structs[*struct_id]
                        .semantic_fields()
                        .iter()
                        .map(|field| field.ty)
                        .collect(),
                )
            })
            .or_else(|| {
                self.classes_by_name.get(name).map(|(class_id, _)| {
                    (
                        self.classes[*class_id].type_params.len(),
                        self.classes[*class_id]
                            .semantic_constructor()
                            .iter()
                            .map(|field| field.ty)
                            .collect(),
                    )
                })
            })
    }

    pub(super) fn mark_type_params(&self, ty: TypeId, bound: &mut [bool]) {
        match &self.types[ty] {
            Type::Param(index) => bound[index.into_raw() as usize] = true,
            Type::Struct(application) => {
                for arg in &self.struct_applications[*application].arguments {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Class(application) => {
                for arg in &self.class_applications[*application].arguments {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Enum(application) => {
                for arg in &self.enum_applications[*application].arguments {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Interface(application) => {
                for arg in &self.interface_applications[*application].arguments {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Tuple(args) => {
                for arg in args {
                    self.mark_type_params(*arg, bound);
                }
            }
            Type::Function(id) => {
                let function = &self.function_types[*id];
                for parameter in &function.parameter_types {
                    self.mark_type_params(*parameter, bound);
                }
                self.mark_type_params(function.return_type, bound);
            }
            Type::Ptr(pointee) => self.mark_type_params(*pointee, bound),
            Type::FunPtr(id) => {
                let function = &self.function_types[*id];
                for parameter in &function.parameter_types {
                    self.mark_type_params(*parameter, bound);
                }
                self.mark_type_params(function.return_type, bound);
            }
            _ => {}
        }
    }

    pub(super) fn unit_variant_from_field(
        &self,
        access: &ast::FieldAccess,
    ) -> Option<(hir::EnumId, u32)> {
        let ast::Expr::Var(enum_name) = access.receiver.as_ref() else {
            return None;
        };
        if self.scopes.lookup(&enum_name.text).is_some() || self.host_has_property(&enum_name.text)
        {
            return None;
        }
        let enum_id = self.enums_by_name.get(&enum_name.text).copied()?;
        let ast::FieldSelector::Name(variant_name) = &access.selector else {
            return None;
        };
        let variant = self.find_variant(enum_id, &variant_name.text)?;
        (!self.enums[enum_id].type_params.is_empty()
            && self.enums[enum_id].variants[variant as usize]
                .fields
                .is_empty())
        .then_some((enum_id, variant))
    }

    /// Bind type arguments by matching a parameter (or variant field)
    /// type against the argument type: `T` binds to the argument type,
    /// `Option<T>` vs `Option<Int>` recurses (so `T = Int`) — as do
    /// other enum applications — generic struct applications
    /// (`PinnedPtr<T>`, M12) match by struct and recurse into their
    /// argument lists, and tuples match elementwise. Anything
    /// else is left to the argument type check. Returns `false` after
    /// recording a conflict diagnostic.
    pub(super) fn bind_type_args(
        &mut self,
        param_ty: TypeId,
        arg_ty: TypeId,
        bindings: &mut [Option<TypeId>],
        type_params: &[hir::TypeParamDecl],
        span: Span,
    ) -> bool {
        match (self.types[param_ty].clone(), self.types[arg_ty].clone()) {
            (Type::Param(index), _) => {
                let index = index.into_raw() as usize;
                match bindings[index] {
                    Some(existing) => {
                        if self.types_equal(existing, arg_ty) {
                            true
                        } else {
                            let first = self.type_name(existing);
                            let second = self.type_name(arg_ty);
                            self.error(
                                span,
                                format!(
                                    "conflicting types for `{}`: {first} and {second}",
                                    type_params[index].name
                                ),
                            );
                            false
                        }
                    }
                    None => {
                        bindings[index] = Some(arg_ty);
                        true
                    }
                }
            }
            (Type::Enum(param), Type::Enum(arg)) => {
                let param = self.enum_applications[param].clone();
                let arg = self.enum_applications[arg].clone();
                if param.template != arg.template || param.arguments.len() != arg.arguments.len() {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.arguments.iter().zip(arg.arguments.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok
            }
            (Type::Struct(param), Type::Struct(arg)) => {
                let param = self.struct_applications[param].clone();
                let arg = self.struct_applications[arg].clone();
                if param.template != arg.template || param.arguments.len() != arg.arguments.len() {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.arguments.iter().zip(arg.arguments.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok
            }
            (Type::Class(param), Type::Class(arg)) => {
                let param = self.class_applications[param].clone();
                let arg = self.class_applications[arg].clone();
                if param.template != arg.template || param.arguments.len() != arg.arguments.len() {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.arguments.iter().zip(arg.arguments.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok
            }
            (Type::Interface(param), Type::Interface(arg)) => {
                let param = self.interface_applications[param].clone();
                let arg = self.interface_applications[arg].clone();
                if param.template != arg.template || param.arguments.len() != arg.arguments.len() {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.arguments.iter().zip(arg.arguments.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok
            }
            (Type::Interface(param), _) => {
                let param = self.interface_applications[param].clone();
                let Some(arg_args) = self.implemented_interface_application(arg_ty, param.template)
                else {
                    return true;
                };
                if param.arguments.len() != arg_args.len() {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.arguments.iter().zip(arg_args) {
                    ok &= self.bind_type_args(*param, arg, bindings, type_params, span);
                }
                ok
            }
            (Type::Ptr(param), Type::Ptr(arg)) => {
                self.bind_type_args(param, arg, bindings, type_params, span)
            }
            (Type::Tuple(param_elements), Type::Tuple(arg_elements))
                if param_elements.len() == arg_elements.len() =>
            {
                let mut ok = true;
                for (param, arg) in param_elements.iter().zip(arg_elements.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok
            }
            (Type::Function(param_id), Type::Function(arg_id)) => {
                let param = self.function_types[param_id].clone();
                let arg = self.function_types[arg_id].clone();
                if param.is_suspend != arg.is_suspend
                    || param.parameter_types.len() != arg.parameter_types.len()
                {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.parameter_types.iter().zip(arg.parameter_types.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok &= self.bind_type_args(
                    param.return_type,
                    arg.return_type,
                    bindings,
                    type_params,
                    span,
                );
                ok
            }
            (Type::FunPtr(param_id), Type::FunPtr(arg_id)) => {
                let param = self.function_types[param_id].clone();
                let arg = self.function_types[arg_id].clone();
                if param.is_suspend != arg.is_suspend
                    || param.parameter_types.len() != arg.parameter_types.len()
                {
                    return true;
                }
                let mut ok = true;
                for (param, arg) in param.parameter_types.iter().zip(arg.parameter_types.iter()) {
                    ok &= self.bind_type_args(*param, *arg, bindings, type_params, span);
                }
                ok &= self.bind_type_args(
                    param.return_type,
                    arg.return_type,
                    bindings,
                    type_params,
                    span,
                );
                ok
            }
            _ => true,
        }
    }
}
