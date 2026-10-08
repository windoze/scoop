use super::*;

impl Lowerer {
    pub(super) fn annotation_scalar_kind(
        &self,
        ty: hir::TypeId,
    ) -> Option<hir::CanonicalConstValueKindV1> {
        if self.is_char_type(ty) {
            return Some(hir::CanonicalConstValueKindV1::Char);
        }
        if let Some(kind) = self.float_kind(ty) {
            return Some(hir::CanonicalConstValueKindV1::Float(kind));
        }
        match self.types[ty] {
            hir::Type::Integer(kind) => Some(hir::CanonicalConstValueKindV1::Integer(kind)),
            hir::Type::Boolean => Some(hir::CanonicalConstValueKindV1::Boolean),
            hir::Type::String => Some(hir::CanonicalConstValueKindV1::String),
            _ => None,
        }
    }

    pub(crate) fn annotation_constant(
        &mut self,
        value: &ast::AnnotationLiteral,
        ty: hir::TypeId,
        span: ast::Span,
    ) -> Option<hir::CanonicalConstValueV1> {
        let value = match value {
            ast::AnnotationLiteral::Int(literal) => {
                self.lower_integer_literal(*literal, Some(ty), false, span)?
            }
            ast::AnnotationLiteral::SignedInt { negative, literal } => {
                self.lower_integer_literal(*literal, Some(ty), *negative, span)?
            }
            ast::AnnotationLiteral::Float(literal) => {
                self.lower_float_literal(literal, Some(ty), false, span)?
            }
            ast::AnnotationLiteral::SignedFloat { negative, literal } => {
                self.lower_float_literal(literal, Some(ty), *negative, span)?
            }
            ast::AnnotationLiteral::ConstReference(source) => {
                let mut statements = Vec::new();
                let value = self.lower_expr(source, &mut statements, Some(ty))?;
                if !statements.is_empty() {
                    self.error(
                        span,
                        "annotation arguments must refer to a const val".into(),
                    );
                    return None;
                }
                value
            }
            literal => {
                let source = match literal {
                    ast::AnnotationLiteral::String(value) => ast::Expr::StringLiteral {
                        value: value.clone(),
                        span,
                    },
                    ast::AnnotationLiteral::Boolean(value) => ast::Expr::BoolLiteral {
                        value: *value,
                        span,
                    },
                    ast::AnnotationLiteral::Char(value) => ast::Expr::CharLiteral {
                        value: *value,
                        span,
                    },
                    _ => unreachable!("integer and constant-reference forms were handled above"),
                };
                self.lower_expr(&source, &mut Vec::new(), Some(ty))?
            }
        };
        if !self.types_equal(value.ty, ty) {
            self.error(
                span,
                format!(
                    "annotation argument must be {}, found {}",
                    self.type_name(ty),
                    self.type_name(value.ty)
                ),
            );
            return None;
        }
        let constant = match value.kind {
            hir::ExprKind::IntegerLiteral(value) => hir::ConstPropertyValue::Integer(value),
            hir::ExprKind::FloatLiteral(value) => hir::ConstPropertyValue::Float(value),
            hir::ExprKind::BoolLiteral(value) => hir::ConstPropertyValue::Boolean(value),
            hir::ExprKind::CharLiteral(value) => hir::ConstPropertyValue::Char(value),
            hir::ExprKind::StringLiteral { value, .. } => hir::ConstPropertyValue::String(value),
            _ => {
                self.error(
                    span,
                    "annotation arguments must refer to a const val".into(),
                );
                return None;
            }
        };
        Some(constant.into())
    }
}
