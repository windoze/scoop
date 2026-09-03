//! Compiler-known construction of raw FFI pointer values.

use super::*;

impl Lowerer {
    pub(in crate::expr) fn lower_ffi_struct_init(
        &mut self,
        struct_id: hir::StructId,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        if Some(struct_id) == self.ffi_ptr {
            if call.args.len() != 1 {
                self.error(
                    call.span,
                    format!(
                        "`Ptr` takes exactly 1 argument, but {} were supplied",
                        call.args.len()
                    ),
                );
                return None;
            }
            let explicit = self.resolve_call_type_args(call.type_args)?;
            if explicit.len() > 1 {
                self.error(
                    call.span,
                    format!(
                        "`Ptr` takes exactly 1 type argument, but {} were supplied",
                        explicit.len()
                    ),
                );
                return None;
            }
            let expected_pointee = expected.and_then(|ty| match self.types[ty] {
                Type::Ptr(pointee) => Some(pointee),
                _ => None,
            });
            let pointee = explicit.first().copied().or(expected_pointee);
            let Some(pointee) = pointee else {
                self.error(
                    call.span,
                    "cannot infer `Ptr` pointee type; provide `Ptr<T>` or an expected `Ptr<T>` type"
                        .to_string(),
                );
                return None;
            };
            if explicit.first().is_some_and(|explicit| {
                expected_pointee.is_some_and(|expected| !self.types_equal(*explicit, expected))
            }) {
                self.error(
                    call.span,
                    format!(
                        "`Ptr` constructor produces Ptr<{}>, which does not match the expected type",
                        self.type_name(pointee)
                    ),
                );
                return None;
            }
            if !self.is_value_ty(pointee)
                || self.type_contains_param(pointee)
                || !self.is_gc_free(pointee)
            {
                self.error(
                    call.span,
                    format!(
                        "`Ptr` pointee must be a concrete GC-free value type, found {}",
                        self.type_name(pointee)
                    ),
                );
                return None;
            }
            self.require_unsafe_operation(call.span, "constructing `Ptr` from a raw integer");
            let raw = self.lower_expr(&call.args[0], sink, Some(self.uint))?;
            if raw.ty != self.uint {
                self.error(
                    raw.span,
                    format!(
                        "`Ptr` raw value must be of type UInt, found {}",
                        self.type_name(raw.ty)
                    ),
                );
                return None;
            }
            let ty = self.intern_type(Type::Ptr(pointee));
            return Some(hir::Expr {
                kind: ExprKind::PtrFromUInt(Box::new(raw)),
                ty,
                span: call.span,
            });
        }

        debug_assert_eq!(Some(struct_id), self.ffi_fun_ptr);
        if !call.args.is_empty() {
            self.error(
                call.span,
                "`FunPtr` only supports the zero-argument null constructor".to_string(),
            );
            return None;
        }
        let explicit = self.resolve_call_type_args(call.type_args)?;
        if explicit.len() > 1 {
            self.error(
                call.span,
                format!(
                    "`FunPtr` takes exactly 1 type argument, but {} were supplied",
                    explicit.len()
                ),
            );
            return None;
        }
        let expected_signature = expected.and_then(|ty| match self.types[ty] {
            Type::FunPtr(signature) => Some(signature),
            _ => None,
        });
        let explicit_signature = explicit.first().and_then(|ty| match self.types[*ty] {
            Type::Function(signature) => Some(signature),
            _ => None,
        });
        if !explicit.is_empty() && explicit_signature.is_none() {
            self.error(
                call.span,
                "`FunPtr` type argument must be an ordinary concrete function type".to_string(),
            );
            return None;
        }
        let signature = explicit_signature.or(expected_signature);
        let Some(signature) = signature else {
            self.error(
                call.span,
                "cannot infer `FunPtr` signature; provide `FunPtr<F>` or an expected `FunPtr<F>` type"
                    .to_string(),
            );
            return None;
        };
        if explicit_signature.is_some()
            && expected_signature.is_some()
            && explicit_signature != expected_signature
        {
            self.error(
                call.span,
                "explicit `FunPtr` signature does not match the expected type".to_string(),
            );
            return None;
        }
        if self.function_types[signature].is_suspend || self.function_type_contains_param(signature)
        {
            self.error(
                call.span,
                "`FunPtr` type argument must be an ordinary concrete function type".to_string(),
            );
            return None;
        }
        let ty = self.intern_type(Type::FunPtr(signature));
        Some(hir::Expr {
            kind: ExprKind::FunPtrNull,
            ty,
            span: call.span,
        })
    }
}
