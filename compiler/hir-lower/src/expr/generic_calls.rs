//! Generic call resolution, argument inference and concrete callable binding.

use super::*;

mod intrinsics;

impl Lowerer {
    /// A bare call `f(args)` (M7): the candidate layers are, in order,
    /// the current host's methods (inside a member function, where
    /// `f(...)` means `this.f(...)`), the top-level functions declared
    /// on the call site's own side of the core/user boundary (its
    /// "same package" layer), and the other side (the implicitly
    /// imported layer) — the first layer containing any candidate currently
    /// wins whole (milestone7 DESIGN.md 1.2). The layering is relative to
    /// the call site's file: for a user-file call that is user
    /// top-level → core, for a core-file call core → user, so a user
    /// declaration shadows core overloads for user code without
    /// breaking the core library's own internal calls. A single candidate and
    /// an overload set both enter the same M16 resolver.
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
        let members = self
            .current_this_ty()
            .map(|host_ty| self.methods_by_name(host_ty, &name))
            .unwrap_or_default();
        if !members.is_empty() {
            let receiver = self
                .lower_current_this(call.callee.span)
                .expect("a member callable body always has a lexical `this`");
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
        if let [function] = candidates.as_slice()
            && self.ffi_core.is_some_and(|core| {
                *function == core.address_of
                    || *function == core.size_of
                    || *function == core.align_of
            })
        {
            // These operations still need the source lvalue/type syntax. The
            // winner-first intrinsic commit replaces this temporary boundary
            // in the dedicated M16 commit slice.
            return self.lower_pointer_top_level_intrinsic(*function, call);
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
}
