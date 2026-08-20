//! Numeric literal preferences apply after the declaration MSC pool.

use scoop_ast as ast;
use scoop_hir::{FloatKind, IntegerKind, Type, TypeId};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NumericLiteralKind {
    Integer(IntegerKind),
    Float(FloatKind),
}

impl crate::Lowerer {
    pub(crate) fn numeric_literal_kind(&self, ty: TypeId) -> Option<NumericLiteralKind> {
        match self.types[ty] {
            Type::Integer(kind) => Some(NumericLiteralKind::Integer(kind)),
            _ => self.float_kind(ty).map(NumericLiteralKind::Float),
        }
    }
}

pub(crate) fn prefer_literal_defaults(
    candidates: &[usize],
    arguments: &[ast::CallArgument],
    kind: impl Fn(usize, usize) -> Option<NumericLiteralKind>,
) -> Vec<usize> {
    let dominates = |preferred, other| {
        let mut better = false;
        for (index, argument) in arguments.iter().enumerate() {
            let Some(default) = crate::expr::integer_literal_default_kind(&argument.expression)
                .map(NumericLiteralKind::Integer)
                .or_else(|| {
                    crate::expr::float_literal_default_kind(&argument.expression)
                        .map(NumericLiteralKind::Float)
                })
            else {
                continue;
            };
            let (Some(preferred), Some(other)) = (kind(preferred, index), kind(other, index))
            else {
                continue;
            };
            match (preferred == default, other == default) {
                (true, false) => better = true,
                (false, true) => return false,
                _ => {}
            }
        }
        better
    };
    candidates
        .iter()
        .copied()
        .filter(|&candidate| {
            !candidates
                .iter()
                .copied()
                .any(|other| other != candidate && dominates(other, candidate))
        })
        .collect()
}
