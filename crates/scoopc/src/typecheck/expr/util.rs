use crate::ast;
use crate::resolve::Visibility;
use crate::source::SourceFile;
use crate::ty::{EffectRow, TypeId};

use super::super::lower::TypeLowering;

pub(super) fn fmt_effect_row(row: &EffectRow, lower: &TypeLowering<'_>) -> String {
    if row.terms.is_empty() {
        return "Pure".to_string();
    }
    row.terms
        .iter()
        .copied()
        .map(|e| lower.fmt_type(e))
        .collect::<Vec<_>>()
        .join(" + ")
}

pub(super) fn short_name_from_fqn(fqn: &str) -> &str {
    fqn.rsplit('.').next().unwrap_or(fqn)
}

pub(super) fn fmt_overload_signature(
    name: &str,
    receiver_ty: Option<TypeId>,
    params: &[TypeId],
    lower: &TypeLowering<'_>,
) -> String {
    let params = params
        .iter()
        .copied()
        .map(|ty| lower.fmt_type(ty))
        .collect::<Vec<_>>()
        .join(", ");

    match receiver_ty {
        Some(recv) => format!("{}.{}({})", lower.fmt_type(recv), name, params),
        None => format!("{name}({params})"),
    }
}

pub(super) fn join_overload_signatures(mut sigs: Vec<String>) -> String {
    sigs.sort();
    sigs.dedup();
    sigs.join(" | ")
}

pub(super) fn visibility_from_modifiers(modifiers: &[ast::Modifier]) -> Visibility {
    // 当前阶段（T0245）parser 只负责“解析并存储”，不做组合合法性校验；
    // 这里沿用 resolver 的最小优先级规则：`private` > `internal` > 默认 `public`。
    if modifiers.contains(&ast::Modifier::Private) {
        return Visibility::Private;
    }
    if modifiers.contains(&ast::Modifier::Internal) {
        return Visibility::Internal;
    }
    Visibility::Public
}

pub(super) fn expr_kind_name(kind: &ast::ExprKind) -> &'static str {
    match kind {
        ast::ExprKind::Missing => "missing",
        ast::ExprKind::Ident(_) => "ident",
        ast::ExprKind::IntLit => "int literal",
        ast::ExprKind::FloatLit => "float literal",
        ast::ExprKind::CharLit => "char literal",
        ast::ExprKind::StringLit => "string literal",
        ast::ExprKind::UnitLit => "unit literal",
        ast::ExprKind::TupleLit { .. } => "tuple literal",
        ast::ExprKind::ArrayLit { .. } => "array literal",
        ast::ExprKind::InterpolatedString { .. } => "interpolated string",
        ast::ExprKind::Block(_) => "block",
        ast::ExprKind::DoBlock { .. } => "do block",
        ast::ExprKind::UnsafeBlock { .. } => "unsafe block",
        ast::ExprKind::SafeBlock { .. } => "safe block",
        ast::ExprKind::Lambda(_) => "lambda",
        ast::ExprKind::StructLit { .. } => "struct literal",
        ast::ExprKind::ClassLit { .. } => "class literal",
        ast::ExprKind::If { .. } => "if expression",
        ast::ExprKind::When { .. } => "when expression",
        ast::ExprKind::Handle { .. } => "handle expression",
        ast::ExprKind::Async { .. } => "async expression",
        ast::ExprKind::Spawn { .. } => "spawn expression",
        ast::ExprKind::Await { .. } => "await expression",
        ast::ExprKind::Join { .. } => "join expression",
        ast::ExprKind::MemberAccess { .. } => "member access",
        ast::ExprKind::SpliceField { .. } => "splice field access",
        ast::ExprKind::SafeMemberAccess { .. } => "safe member access",
        ast::ExprKind::TypeApply { .. } => "type apply",
        ast::ExprKind::Call { .. } => "call",
        ast::ExprKind::SpreadArg { .. } => "spread argument",
        ast::ExprKind::NamedArg { .. } => "named argument",
        ast::ExprKind::NotNullAssert { .. } => "not-null assertion",
        ast::ExprKind::Unary { .. } => "unary expression",
        ast::ExprKind::Binary { .. } => "binary expression",
        ast::ExprKind::Assign { .. } => "assignment",
        ast::ExprKind::TypeCheck { .. } => "type check (`is`/`!is`)",
        ast::ExprKind::Cast { .. } => "cast (`as`/`as?`)",
        ast::ExprKind::WithUpdate { .. } => "with-update",
    }
}

pub(super) fn package_prefix(source: &SourceFile, pkg: Option<&ast::PackageDecl>) -> String {
    let Some(pkg) = pkg else {
        return String::new();
    };
    pkg.path
        .iter()
        .map(|id| source.slice(id.span))
        .collect::<Vec<_>>()
        .join(".")
}
