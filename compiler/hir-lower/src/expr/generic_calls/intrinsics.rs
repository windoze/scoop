//! Pointer and layout intrinsics recognized at top-level call sites.

use super::*;

impl Lowerer {
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
}
