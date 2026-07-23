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

/// `scoop::typecheck::unsupported_in_this_phase`：该语法形式在当前类型检查里程碑
/// 尚未覆盖（仅在里程碑之间过渡使用；M8 退出闸门要求零到达）。
pub fn unsupported_in_this_phase(what: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::unsupported_in_this_phase",
        format!("当前类型检查阶段暂不支持：{what}"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::break_not_in_loop`：`break`/`continue` 出现在循环体外。
pub fn break_not_in_loop(what: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::break_not_in_loop",
        format!("`{what}` 只能出现在循环体内"),
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::no_applicable_overload`：没有重载候选可接受给定的实参。
pub fn no_applicable_overload(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::no_applicable_overload",
        "没有匹配的重载候选（实参类型 / 数量不匹配）",
    )
    .with_primary(span, "这里")
}

/// `scoop::typecheck::ambiguous_overload`：多个重载候选同等匹配，无法选择。
pub fn ambiguous_overload(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::typecheck::ambiguous_overload",
        "重载解析有歧义：多个候选同等匹配",
    )
    .with_primary(span, "这里")
}
