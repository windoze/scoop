//! Construction-time conversions between immutable and mutable arrays.

use super::*;

impl Lowerer {
    pub(in crate::expr) fn lower_array_conversion(
        &mut self,
        call: &ast::CallExpr,
        sink: &mut Vec<hir::Statement>,
        target_kind: ArrayKind,
    ) -> Option<hir::Expr> {
        let name = call.callee.text.clone();
        let explicit_type_args = self.resolve_call_type_args(&call.type_args)?;
        if explicit_type_args.len() > 1 {
            self.error(
                call.callee.span,
                format!(
                    "`{name}` takes exactly 1 type argument, but {} were supplied",
                    explicit_type_args.len()
                ),
            );
            return None;
        }
        if call.args.len() != 1 {
            let supplied = call.args.len();
            self.error(
                call.span,
                format!("`{name}` takes exactly 1 argument, but {supplied} were supplied"),
            );
            return None;
        }
        let arg = self.lower_expr(&call.args[0], sink, None)?;
        let element_ty = match (target_kind, self.array_type_info(arg.ty)) {
            (
                ArrayKind::Immutable,
                Some(ArrayType {
                    kind: ArrayKind::Mutable,
                    element,
                }),
            )
            | (
                ArrayKind::Mutable,
                Some(ArrayType {
                    kind: ArrayKind::Immutable,
                    element,
                }),
            ) => element,
            (_, Some(_)) => {
                self.error(
                    arg.span,
                    "use the value directly; conversion is only between Array and MutableArray"
                        .to_string(),
                );
                return None;
            }
            _ => {
                let expected = if target_kind == ArrayKind::Immutable {
                    "a MutableArray"
                } else {
                    "an Array"
                };
                let found = self.type_name(arg.ty);
                self.error(
                    arg.span,
                    format!("argument of `{name}` conversion must be {expected}, found {found}"),
                );
                return None;
            }
        };
        if let Some(&explicit) = explicit_type_args.first()
            && !self.types_equal(explicit, element_ty)
        {
            let expected = self.type_name(explicit);
            let found = self.type_name(element_ty);
            self.error(
                call.callee.span,
                format!("explicit element type is {expected}, but the argument contains {found}"),
            );
            return None;
        }
        let ty = self.array_type(target_kind, element_ty);
        Some(hir::Expr {
            kind: ExprKind::ArrayClone(Box::new(arg)),
            ty,
            span: call.span,
        })
    }
}
