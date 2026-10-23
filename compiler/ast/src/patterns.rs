//! The literal syntax accepted in match patterns (language spec chapter 5).

use crate::{Expr, FloatLiteralSyntax, IntegerLiteralSyntax, Span, UnOp};

#[derive(Debug, Clone, PartialEq)]
pub enum LiteralPattern {
    Integer {
        value: IntegerLiteralSyntax,
        negated: bool,
    },
    Float {
        value: FloatLiteralSyntax,
        negated: bool,
    },
    Char(char),
    String(String),
    Boolean(bool),
    Unit,
}

impl LiteralPattern {
    /// Extract only literal syntax when ordinary when conditions contribute
    /// coverage. Calls, constants and other expressions are not patterns.
    pub fn from_expr(expr: &Expr) -> Option<Self> {
        let (operand, negated) = match expr {
            Expr::Unary {
                op: UnOp::Neg,
                operand,
                ..
            } => (operand.as_ref(), true),
            _ => (expr, false),
        };
        Some(match operand {
            Expr::IntLiteral(value) => Self::Integer {
                value: *value,
                negated,
            },
            Expr::FloatLiteral(value) => Self::Float {
                value: value.clone(),
                negated,
            },
            Expr::CharLiteral { value, .. } if !negated => Self::Char(*value),
            Expr::StringLiteral { value, .. } if !negated => Self::String(value.clone()),
            Expr::BoolLiteral { value, .. } if !negated => Self::Boolean(*value),
            Expr::UnitLiteral { .. } if !negated => Self::Unit,
            _ => return None,
        })
    }

    /// Reuse ordinary literal typing while preserving numeric token spans.
    pub fn to_expr(&self, span: Span) -> Expr {
        let (operand, negated) = match self {
            Self::Integer { value, negated } => (Expr::IntLiteral(*value), *negated),
            Self::Float { value, negated } => (Expr::FloatLiteral(value.clone()), *negated),
            Self::Char(value) => {
                return Expr::CharLiteral {
                    value: *value,
                    span,
                };
            }
            Self::String(value) => {
                return Expr::StringLiteral {
                    value: value.clone(),
                    span,
                };
            }
            Self::Boolean(value) => {
                return Expr::BoolLiteral {
                    value: *value,
                    span,
                };
            }
            Self::Unit => return Expr::UnitLiteral { span },
        };
        if negated {
            Expr::Unary {
                op: UnOp::Neg,
                operator_span: None,
                operand: Box::new(operand),
                span,
            }
        } else {
            operand
        }
    }
}
