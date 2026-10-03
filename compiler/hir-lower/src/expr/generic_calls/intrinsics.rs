//! Pointer and layout intrinsics recognized after ordinary call selection.

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
        self.check_call_effects(hir::Callable::Function(function), call.span);
        if function == core.address_of {
            let (place, place_ty) = self.addressable_source_place(&call.args[0].expression)?;
            return self.normalize_address_of(
                place,
                place_ty,
                resolved.type_args[0],
                call.args[0].span,
                call.span,
            );
        }
        let kind = if function == core.size_of {
            hir::PointerIntrinsic::SizeOf
        } else {
            hir::PointerIntrinsic::AlignOf
        };
        self.normalize_layout_intrinsic(kind, resolved.type_args[0], call.span)
    }

    pub(in crate::expr) fn addressable_source_place(
        &mut self,
        expression: &ast::Expr,
    ) -> Option<(hir::Place, hir::TypeId)> {
        let place = match expression {
            ast::Expr::Var(name) => {
                if let Some(local) = self.scopes.lookup(&name.text) {
                    Some((hir::Place::Local(local), self.locals[local].ty))
                } else if self.release_has_field_name(&name.text) {
                    self.error(name.span, "a `release` block cannot take the address of an owner field; copy its value to a local first".into());
                    return None;
                } else {
                    let target = self.resolve_value_name(name).ok()?;
                    target.and_then(|target| {
                        if let crate::imports::lookup::values::ResolvedValueTarget::Dependency(
                            binding,
                        ) = &target
                        {
                            let hir::ImportedTarget::Property(property) = binding.target() else {
                                return None;
                            };
                            let place = self.external_global_place(property.persistent()).ok()?;
                            let hir::Place::ExternalGlobal { ty, .. } = &place else {
                                unreachable!("external property lookup yields its global place")
                            };
                            return Some((place.clone(), *ty));
                        }
                        let crate::imports::lookup::values::ResolvedValueTarget::Materialized(
                            crate::imports::lookup::values::ValueTarget::Property(property),
                        ) = target
                        else {
                            return None;
                        };
                        match self.properties[property].representation {
                            hir::PropertyRepresentation::NativeStorage { storage } => {
                                Some((hir::Place::Global(storage), self.globals[storage].ty))
                            }
                            _ => None,
                        }
                    })
                }
            }
            ast::Expr::This { .. } => self
                .current_this
                .map(|(local, ty)| (hir::Place::Local(local), ty)),
            _ => None,
        };
        let Some((place, ty)) = place else {
            self.error(expression.span(), "`addressOf` argument must be an addressable local, parameter, global, or value-type `this`".into());
            return None;
        };
        if let hir::Place::Global(global) = &place
            && matches!(
                self.globals[*global].storage,
                hir::GlobalStorage::Extern { .. }
            )
        {
            self.require_unsafe_operation(
                expression.span(),
                "taking the address of an extern global",
            );
        }
        if matches!(place, hir::Place::ExternalGlobal { .. }) {
            self.require_unsafe_operation(
                expression.span(),
                "taking the address of an extern global",
            );
        }
        Some((place, ty))
    }

    pub(in crate::expr) fn normalize_address_of(
        &mut self,
        place: hir::Place,
        place_ty: hir::TypeId,
        inferred: hir::TypeId,
        place_span: Span,
        call_span: Span,
    ) -> Option<hir::Expr> {
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
            || (!self.type_contains_param(place_ty) && !self.is_gc_free(place_ty))
        {
            self.error(
                place_span,
                format!(
                    "`addressOf` requires a GC-free value type, found {}",
                    self.type_name(place_ty)
                ),
            );
            return None;
        }
        let ty = self.intern_type(Type::Ptr(place_ty));
        Some(hir::Expr {
            kind: ExprKind::AddressOf(place),
            ty,
            span: call_span,
            origin: self.expression_origin(call_span),
        })
    }

    pub(in crate::expr) fn normalize_layout_intrinsic(
        &mut self,
        intrinsic: hir::PointerIntrinsic,
        target: hir::TypeId,
        span: Span,
    ) -> Option<hir::Expr> {
        if !self.is_value_ty(target) {
            self.error(
                span,
                format!(
                    "memory layout requires a value type, found {}",
                    self.type_name(target)
                ),
            );
            return None;
        }
        let kind = match intrinsic {
            hir::PointerIntrinsic::SizeOf => ExprKind::SizeOf(target),
            hir::PointerIntrinsic::AlignOf => ExprKind::AlignOf(target),
            _ => unreachable!("layout query uses a layout intrinsic"),
        };
        Some(hir::Expr {
            kind,
            ty: self.integer_type(hir::IntegerKind::UNSIGNED_64),
            span,
            origin: self.expression_origin(span),
        })
    }
}
