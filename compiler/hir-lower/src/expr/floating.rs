//! Candidate-local decimal conversion to the selected IEEE format.

use super::*;

mod operations;
use rustc_apfloat::{
    Float, Round,
    ieee::{Double, Single},
};

impl Lowerer {
    pub(crate) fn lower_float_literal(
        &mut self,
        literal: &ast::FloatLiteralSyntax,
        expected: Option<TypeId>,
        negative: bool,
        span: Span,
    ) -> Option<hir::Expr> {
        let kind = match literal.suffix {
            ast::FloatSuffix::Float => hir::FloatKind::F32,
            ast::FloatSuffix::None => expected
                .and_then(|ty| self.float_kind(ty))
                .unwrap_or(hir::FloatKind::F64),
        };
        let ty = self
            .core_float_type(kind)
            .map_err(|error| {
                self.error(span, error.diagnostic("floating literal type"));
            })
            .ok()?;
        let value = match parse_decimal(&literal.decimal, kind) {
            Ok(value) => value,
            Err(message) => {
                self.error(
                    span,
                    format!(
                        "floating literal is out of range for {}: {message}",
                        kind.canonical_name()
                    ),
                );
                return None;
            }
        };
        Some(hir::Expr {
            kind: ExprKind::FloatLiteral(if negative { value.negate() } else { value }),
            ty,
            span,
            origin: self.expression_origin(span),
        })
    }
}

fn parse_decimal(decimal: &str, kind: hir::FloatKind) -> Result<hir::HirFloatConstant, String> {
    match kind {
        hir::FloatKind::F32 => {
            let value = Single::from_str_r(decimal, Round::NearestTiesToEven)
                .map_err(|error| format!("{error:?}"))?
                .value;
            if value.is_infinite() {
                return Err("rounding produces infinity".into());
            }
            Ok(hir::HirFloatConstant::F32(value.to_bits() as u32))
        }
        hir::FloatKind::F64 => {
            let value = Double::from_str_r(decimal, Round::NearestTiesToEven)
                .map_err(|error| format!("{error:?}"))?
                .value;
            if value.is_infinite() {
                return Err("rounding produces infinity".into());
            }
            Ok(hir::HirFloatConstant::F64(value.to_bits() as u64))
        }
    }
}
