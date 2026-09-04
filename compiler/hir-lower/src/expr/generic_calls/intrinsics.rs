//! Pointer and layout intrinsics recognized at top-level call sites.

use super::*;

impl Lowerer {
    pub(super) fn lower_pointer_top_level_intrinsic(
        &mut self,
        function: hir::FunctionId,
        call: &ast::CallExpr,
        resolved: crate::overload::ResolvedCallee,
    ) -> Option<hir::Expr> {
        let core = self
            .ffi_core
            .expect("pointer core is validated before bodies are lowered");
        if function == core.address_of {
            debug_assert_eq!(call.args.len(), 1);
            debug_assert_eq!(resolved.args.len(), 1);
            debug_assert_eq!(resolved.type_args.len(), 1);
            let place = match &call.args[0].expression {
                ast::Expr::Var(name) => self
                    .scopes
                    .lookup(&name.text)
                    .map(|local| (hir::Place::Local(local), self.locals[local].ty, name.span))
                    .or_else(|| {
                        self.visible_global(&name.text).map(|global| {
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
                    call.args[0].span,
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
            let inferred = resolved.type_args[0];
            if !self.types_equal(inferred, place_ty) {
                self.error(
                    place_span,
                    format!(
                        "`addressOf` type argument must match the place type {}, found {}",
                        self.type_name(place_ty),
                        self.type_name(inferred)
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
            debug_assert!(self.types_equal(ty, resolved.return_ty));
            return Some(hir::Expr {
                kind: ExprKind::AddressOf(place),
                ty,
                span: call.span,
                origin: self.expression_origin(call.span),
            });
        }

        debug_assert!(call.args.is_empty());
        debug_assert!(resolved.args.is_empty());
        debug_assert_eq!(resolved.type_args.len(), 1);
        let target = resolved.type_args[0];
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
        self.check_call_effects(hir::Callable::Function(function), call.span);
        debug_assert!(self.types_equal(resolved.return_ty, self.uint));
        Some(hir::Expr {
            kind,
            ty: self.uint,
            span: call.span,
            origin: self.expression_origin(call.span),
        })
    }
}
