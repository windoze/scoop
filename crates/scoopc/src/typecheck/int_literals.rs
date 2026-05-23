use crate::source::SourceFile;
use crate::span::Span;
use crate::syntax::int_literal::{
    IntLiteralTarget, checked_negated_int_literal_bits, checked_positive_int_literal_bits,
    parse_int_literal_checked,
};
use crate::ty::{BuiltinTypes, TypeId, TypeKind, ValueTypeKind};

use super::TypeLowering;
use super::expr::ExprTypeError;

pub(super) fn check_positive_int_literal_for_type(
    source: &SourceFile,
    span: Span,
    ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), ExprTypeError> {
    let text = source.slice(span);
    check_int_literal_text_for_type(span, text, text, false, ty, lower, builtins)
}

pub(super) fn check_negated_int_literal_for_type(
    source: &SourceFile,
    span: Span,
    literal_span: Span,
    ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), ExprTypeError> {
    let text = source.slice(span);
    let literal_text = source.slice(literal_span);
    check_int_literal_text_for_type(span, text, literal_text, true, ty, lower, builtins)
}

fn check_int_literal_text_for_type(
    span: Span,
    display_text: &str,
    parse_text: &str,
    negative: bool,
    ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Result<(), ExprTypeError> {
    let Some(target) = int_literal_target_for_type(ty, lower, builtins) else {
        return Ok(());
    };
    let raw = parse_int_literal_checked(parse_text).map_err(|err| {
        ExprTypeError::InvalidIntegerLiteral {
            reason: err.reason(),
            text: display_text.to_string(),
            span: span.into(),
        }
    })?;
    let valid = if negative {
        checked_negated_int_literal_bits(raw, target).is_some()
    } else {
        checked_positive_int_literal_bits(raw, target).is_some()
    };
    if !valid {
        return Err(ExprTypeError::InvalidIntegerLiteral {
            reason: "超出目标整数类型可表示范围",
            text: display_text.to_string(),
            span: span.into(),
        });
    }
    Ok(())
}

fn int_literal_target_for_type(
    ty: TypeId,
    lower: &TypeLowering<'_>,
    builtins: BuiltinTypes,
) -> Option<IntLiteralTarget> {
    if ty == builtins.int {
        return Some(IntLiteralTarget::new(64, true));
    }
    if ty == builtins.uint {
        return Some(IntLiteralTarget::new(64, false));
    }

    match lower.type_kind(ty) {
        TypeKind::Value(ValueTypeKind::Int) => Some(IntLiteralTarget::new(64, true)),
        TypeKind::Value(ValueTypeKind::UInt) => Some(IntLiteralTarget::new(64, false)),
        TypeKind::Value(ValueTypeKind::IntN(bits)) => {
            Some(IntLiteralTarget::new(u32::from(bits), true))
        }
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => {
            Some(IntLiteralTarget::new(u32::from(bits), false))
        }
        TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            nominal_int_literal_target(&nominal.fqn)
        }
        _ => None,
    }
}

fn nominal_int_literal_target(fqn: &str) -> Option<IntLiteralTarget> {
    match fqn {
        "scoop.core.Int8" => Some(IntLiteralTarget::new(8, true)),
        "scoop.core.Int16" => Some(IntLiteralTarget::new(16, true)),
        "scoop.core.Int32" => Some(IntLiteralTarget::new(32, true)),
        "scoop.core.Int64" => Some(IntLiteralTarget::new(64, true)),
        "scoop.core.UInt8" => Some(IntLiteralTarget::new(8, false)),
        "scoop.core.UInt16" => Some(IntLiteralTarget::new(16, false)),
        "scoop.core.UInt32" => Some(IntLiteralTarget::new(32, false)),
        "scoop.core.UInt64" => Some(IntLiteralTarget::new(64, false)),
        _ => None,
    }
}
