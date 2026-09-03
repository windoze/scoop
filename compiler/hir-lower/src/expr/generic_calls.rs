//! Generic call resolution, argument inference and concrete callable binding.

use super::*;

mod intrinsics;

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
}
