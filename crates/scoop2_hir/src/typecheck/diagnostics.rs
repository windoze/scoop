//! typecheck 阶段的稳定诊断码与构造辅助。
//!
//! 诊断码形如 `scoop::typecheck::<name>`，需与 `tests/fixtures/typecheck/` 的
//! `EXPECT-ERROR-CODE` 对齐。

use scoop2_base::{Span, diag::Diagnostic};

/// `scoop::typecheck::type_mismatch`：期望类型与实际类型不兼容。
pub fn type_mismatch(expected: &str, found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::type_mismatch",
        format!("类型不匹配：期望 {expected}，但得到 {found}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::cannot_call`：表达式不可调用（非函数类型）。
pub fn cannot_call(found: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::cannot_call",
        format!("不可调用：{found} 不是函数类型"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::arity_mismatch`：调用实参数量与形参不符。
pub fn arity_mismatch(expected: usize, found: usize, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::arity_mismatch",
        format!("参数数量不匹配：期望 {expected} 个，但传入 {found} 个"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::unresolved_type_ref`：类型引用无法降级为类型（resolve 未捕获的残余）。
pub fn unresolved_type_ref(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::unresolved_type_ref",
        format!("无法解析的类型引用：{name}"),
    )
    .with_primary(span, "这里")
}
