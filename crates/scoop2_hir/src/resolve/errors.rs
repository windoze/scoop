//! Diagnostic helpers and stable codes for the resolve phase.

use scoop2_base::{Span, diag::Diagnostic};

pub fn duplicate_definition(name: &str, first: Span, second: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::resolve::duplicate_definition",
        format!("重复定义：{name}"),
    )
    .with_primary(second, "重复定义在这里")
    .with_related(first, "第一次定义在这里")
}

pub fn unresolved_import(import: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::resolve::unresolved_import",
        format!("未解析的 import：{import}"),
    )
    .with_primary(span, "这里")
}

pub fn unresolved_type(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::resolve::unresolved_type",
        format!("未解析的类型：{name}"),
    )
    .with_primary(span, "这里")
}

pub fn unresolved_value(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::resolve::unresolved_value",
        format!("未解析的值：{name}"),
    )
    .with_primary(span, "这里")
}

pub fn unresolved_member(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::resolve::unresolved_member",
        format!("未解析的成员：{name}"),
    )
    .with_primary(span, "这里")
}

pub fn unresolved_type_param(name: &str, span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::resolve::unresolved_type_param",
        format!("未解析的类型参数：{name}"),
    )
    .with_primary(span, "这里的类型参数不在当前声明的泛型参数列表中")
}

pub fn invalid_visibility(span: Span) -> Diagnostic {
    Diagnostic::error(
        "scoop::resolve::invalid_visibility",
        "非法的可见性修饰符组合（public/internal/private 只能出现一个）",
    )
    .with_primary(span, "这里")
}

pub fn prelude_package_not_loaded(package: &str) -> Diagnostic {
    Diagnostic::error(
        "scoop::resolve::prelude_package_not_loaded",
        format!("编译器配置错误：prelude package `{package}` 所属 cone 未加载"),
    )
}
