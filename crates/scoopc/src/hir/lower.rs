//! AST → HIR 的最小 lowering（TODO T0701）。
//!
//! 说明：
//! - 这里的 lowering 仅用于 `dump-hir` 的调试输出，因此实现上优先保证“稳定输出 + 不 panic”；。
//! - 完整 lowering（含类型推断结果、更多语法节点）会在后续任务（TODO T0702+）逐步补齐。

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::parser::{ParseError, parse_file};
use crate::resolve::{Index, ResolveError};
use crate::session::Session;
use crate::source::SourceFile;
use crate::ty::{BuiltinTypes, EffectRow, TypeId, TypeStore};

use super::{
    Block, CallArg, Expr, ExprKind, File, FunDecl, Item, LiteralKind, Param, Stmt, StmtKind,
    ValDecl, ValueRef,
};

/// HIR lowering 错误（目前仅包装 parser/resolve 错误）。
#[derive(Debug, Error, Diagnostic)]
pub enum HirLowerError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Parse(#[from] ParseError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    Resolve(#[from] ResolveError),
}

/// 一次 lowering 的产物：HIR + 对应的 `TypeStore`。
///
/// 说明：HIR 节点里的 `TypeId` 仅在同一个 `TypeStore` 里可解码/展示。
#[derive(Debug)]
pub struct LoweredHir {
    pub file: File,
    pub types: TypeStore,
}

/// 为 `scoop dump-hir` 生成 HIR（最小实现）。
///
/// 流程：
/// 1) parse 源文件为 AST；
/// 2) 构建 sysroot + 当前文件的 `Index`；
/// 3) 运行 resolver（headers + bodies）把绑定结果写回 AST；
/// 4) 在一个新的 `TypeStore` 中 intern builtin types，并把 AST 降为 HIR（未覆盖节点用 `Any` 占位）。
pub fn lower_for_dump(session: &Session, source: &SourceFile) -> Result<LoweredHir, HirLowerError> {
    let mut ast = parse_file(source)?;

    let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, &ast));

    let index = Index::build(&pairs)?;

    let headers = crate::resolve::check_file_headers(source, &ast, &index)?;
    crate::resolve::check_file_bodies(source, &mut ast, &index, &headers)?;

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    let file = lower_file(source, &ast, &mut types, builtins);
    Ok(LoweredHir { file, types })
}

fn lower_file(
    source: &SourceFile,
    file: &ast::File,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> File {
    let pkg_prefix = package_prefix(source, file.package.as_ref());
    let mut items = Vec::with_capacity(file.items.len());

    for item in &file.items {
        items.push(lower_item(source, &pkg_prefix, item, types, builtins));
    }

    File { items }
}

fn lower_item(
    source: &SourceFile,
    pkg_prefix: &str,
    item: &ast::Item,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Item {
    match item {
        ast::Item::Fun(fun) => Item::Fun(lower_fun_decl(source, pkg_prefix, fun, types, builtins)),
        ast::Item::Val(v) => Item::Val(lower_val_decl(source, v, types, builtins)),
        ast::Item::TypeAlias(ta) => Item::Todo {
            span: ta.span,
            kind: "typealias",
        },
        ast::Item::Type(ty) => Item::Todo {
            span: ty.span,
            kind: "type",
        },
        ast::Item::Object(obj) => Item::Todo {
            span: obj.span,
            kind: "object",
        },
        ast::Item::ExtensionProperty(p) => Item::Todo {
            span: p.span,
            kind: "extension_property",
        },
    }
}

fn lower_fun_decl(
    source: &SourceFile,
    pkg_prefix: &str,
    fun: &ast::FunDecl,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> FunDecl {
    let name = fun.name.text(source).to_string();
    let fqn = if pkg_prefix.is_empty() {
        name.clone()
    } else {
        format!("{pkg_prefix}.{name}")
    };

    let params: Vec<Param> = fun
        .params
        .iter()
        .map(|p| Param {
            span: p.name.span,
            name: p.name.text(source).to_string(),
            ty: p
                .ty
                .as_ref()
                .map(|t| lower_type_ref(source, t, types, builtins))
                .unwrap_or(builtins.any),
        })
        .collect();

    let receiver_ty = fun
        .receiver
        .as_ref()
        .map(|t| lower_type_ref(source, t, types, builtins));

    // 当前阶段：未接入返回类型推断，缺省时用 `Any` 占位。
    let return_ty = fun
        .return_ty
        .as_ref()
        .map(|t| lower_type_ref(source, t, types, builtins))
        .unwrap_or(builtins.any);

    let effects = EffectRow::pure();
    let ty = types.ty_function(
        receiver_ty,
        params.iter().map(|p| p.ty).collect(),
        return_ty,
        effects,
    );

    let body = match &fun.body {
        ast::FunBody::Block(b) => Some(lower_block(source, b, types, builtins)),
        ast::FunBody::Missing => None,
    };

    FunDecl {
        span: fun.span,
        fqn,
        name,
        ty,
        params,
        return_ty,
        body,
    }
}

fn lower_val_decl(
    source: &SourceFile,
    v: &ast::ValDecl,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> ValDecl {
    let init = v
        .init
        .as_ref()
        .map(|e| lower_expr(source, e, types, builtins));

    let declared_ty =
        v.ty.as_ref()
            .map(|t| lower_type_ref(source, t, types, builtins));

    let ty = declared_ty
        .or_else(|| init.as_ref().map(|e| e.ty))
        .unwrap_or(builtins.any);

    ValDecl {
        span: v.span,
        name: v.name().map(|id| id.text(source).to_string()),
        mutable: v.kind == ast::ValKind::Var,
        ty,
        init,
    }
}

fn lower_block(
    source: &SourceFile,
    b: &ast::Block,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Block {
    let mut stmts = Vec::with_capacity(b.stmts.len());
    for s in &b.stmts {
        stmts.push(lower_stmt(source, s, types, builtins));
    }

    // 当前阶段：用 block 最后一条“表达式语句”的类型作为 block 类型，否则视为 Unit。
    let ty = stmts
        .last()
        .and_then(|s| match &s.kind {
            StmtKind::Expr(e) => Some(e.ty),
            _ => None,
        })
        .unwrap_or(builtins.unit);

    Block {
        span: b.span,
        ty,
        stmts,
    }
}

fn lower_stmt(
    source: &SourceFile,
    s: &ast::Stmt,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Stmt {
    let (kind, ty) = match &s.kind {
        ast::StmtKind::Empty => (StmtKind::Empty, builtins.unit),
        ast::StmtKind::Expr(e) => {
            let e = lower_expr(source, e, types, builtins);
            (StmtKind::Expr(e), builtins.unit)
        }
        ast::StmtKind::Val(v) => {
            let v = lower_val_decl(source, v, types, builtins);
            (StmtKind::Val(v), builtins.unit)
        }
        ast::StmtKind::Return { value, .. } => {
            let value = value
                .as_ref()
                .map(|e| lower_expr(source, e, types, builtins));
            (StmtKind::Return { value }, builtins.nothing)
        }
        ast::StmtKind::Missing => (StmtKind::Todo("missing_stmt"), builtins.unit),
        ast::StmtKind::While { .. } => (StmtKind::Todo("while"), builtins.unit),
        ast::StmtKind::Break { .. } => (StmtKind::Todo("break"), builtins.unit),
        ast::StmtKind::Continue { .. } => (StmtKind::Todo("continue"), builtins.unit),
        ast::StmtKind::ComptimeBlock { .. } => (StmtKind::Todo("comptime_block"), builtins.unit),
        ast::StmtKind::ComptimeIf(_) => (StmtKind::Todo("comptime_if"), builtins.unit),
        ast::StmtKind::ComptimeFor(_) => (StmtKind::Todo("comptime_for"), builtins.unit),
    };

    Stmt {
        span: s.span,
        ty,
        kind,
    }
}

fn lower_expr(
    source: &SourceFile,
    e: &ast::Expr,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> Expr {
    let (kind, ty) = match &e.kind {
        ast::ExprKind::Missing => (ExprKind::Missing, builtins.any),
        ast::ExprKind::IntLit => (ExprKind::Literal(LiteralKind::Int), builtins.int),
        ast::ExprKind::StringLit => (ExprKind::Literal(LiteralKind::String), builtins.string),
        ast::ExprKind::UnitLit => (ExprKind::Literal(LiteralKind::Unit), builtins.unit),
        ast::ExprKind::InterpolatedString { .. } => {
            (ExprKind::Literal(LiteralKind::String), builtins.string)
        }
        ast::ExprKind::Ident(id) => lower_ident_expr(source, id, builtins),
        ast::ExprKind::Block(b) => {
            let b = lower_block(source, b, types, builtins);
            let ty = b.ty;
            (ExprKind::Block(b), ty)
        }
        ast::ExprKind::Call { callee, args } => {
            let callee = Box::new(lower_expr(source, callee, types, builtins));
            let args = args
                .iter()
                .map(|arg| lower_call_arg(source, arg, types, builtins))
                .collect();
            (ExprKind::Call { callee, args }, builtins.any)
        }
        ast::ExprKind::NamedArg { .. } => (ExprKind::Todo("named_arg"), builtins.any),
        ast::ExprKind::TupleLit { .. } => (ExprKind::Todo("tuple_lit"), builtins.any),
        ast::ExprKind::Lambda(_) => (ExprKind::Todo("lambda"), builtins.any),
        ast::ExprKind::StructLit { .. } => (ExprKind::Todo("struct_lit"), builtins.any),
        ast::ExprKind::If { .. } => (ExprKind::Todo("if"), builtins.any),
        ast::ExprKind::When { .. } => (ExprKind::Todo("when"), builtins.any),
        ast::ExprKind::Handle { .. } => (ExprKind::Todo("handle"), builtins.any),
        ast::ExprKind::MemberAccess { .. } => (ExprKind::Todo("member_access"), builtins.any),
        ast::ExprKind::SpliceField { .. } => (ExprKind::Todo("splice_field"), builtins.any),
        ast::ExprKind::SafeMemberAccess { .. } => {
            (ExprKind::Todo("safe_member_access"), builtins.any)
        }
        ast::ExprKind::NotNullAssert { .. } => (ExprKind::Todo("not_null_assert"), builtins.any),
        ast::ExprKind::Unary { .. } => (ExprKind::Todo("unary"), builtins.any),
        ast::ExprKind::Binary { .. } => (ExprKind::Todo("binary"), builtins.any),
        ast::ExprKind::Assign { .. } => (ExprKind::Todo("assign"), builtins.any),
        ast::ExprKind::TypeCheck { .. } => (ExprKind::Todo("type_check"), builtins.any),
        ast::ExprKind::Cast { .. } => (ExprKind::Todo("cast"), builtins.any),
        ast::ExprKind::WithUpdate { .. } => (ExprKind::Todo("with_update"), builtins.any),
    };

    Expr {
        span: e.span,
        ty,
        kind,
    }
}

fn lower_call_arg(
    source: &SourceFile,
    arg: &ast::Expr,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> CallArg {
    match &arg.kind {
        ast::ExprKind::NamedArg { name, value, .. } => CallArg::Named {
            name: name.text(source).to_string(),
            name_span: name.span,
            value: lower_expr(source, value, types, builtins),
        },
        _ => CallArg::Positional(lower_expr(source, arg, types, builtins)),
    }
}

fn lower_ident_expr(
    source: &SourceFile,
    id: &ast::ValueIdent,
    builtins: BuiltinTypes,
) -> (ExprKind, TypeId) {
    let text = source.slice(id.span);
    if text == "true" {
        return (ExprKind::Literal(LiteralKind::Bool(true)), builtins.bool_);
    }
    if text == "false" {
        return (ExprKind::Literal(LiteralKind::Bool(false)), builtins.bool_);
    }

    let Some(resolved) = id.resolved.as_ref() else {
        return (ExprKind::Todo("unresolved_ident"), builtins.any);
    };

    let resolved = match resolved {
        ast::ResolvedValueRef::Local { name, decl_span } => ValueRef::Local {
            name: name.clone(),
            decl_span: *decl_span,
        },
        ast::ResolvedValueRef::TopLevel { fqn } => ValueRef::TopLevel { fqn: fqn.clone() },
    };

    (ExprKind::VarRef(resolved), builtins.any)
}

fn lower_type_ref(
    source: &SourceFile,
    t: &ast::TypeRef,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> TypeId {
    match t {
        ast::TypeRef::Path(p) => lower_type_path(source, p, types, builtins),
        ast::TypeRef::Tuple(tt) => {
            let elements = tt
                .elements
                .iter()
                .map(|e| lower_type_ref(source, e, types, builtins))
                .collect();
            types.ty_tuple(elements)
        }
        ast::TypeRef::Nullable { inner, .. } => {
            let inner = lower_type_ref(source, inner, types, builtins);
            types.ty_option(inner)
        }
        ast::TypeRef::Function(fun) => {
            let receiver = fun
                .receiver
                .as_ref()
                .map(|r| lower_type_ref(source, r, types, builtins));
            let params = fun
                .params
                .iter()
                .map(|p| lower_type_ref(source, p, types, builtins))
                .collect();
            let return_ty = lower_type_ref(source, &fun.return_ty, types, builtins);
            types.ty_function(receiver, params, return_ty, EffectRow::pure())
        }
        ast::TypeRef::Star { .. } | ast::TypeRef::EffectRowArg { .. } => builtins.any,
    }
}

fn lower_type_path(
    source: &SourceFile,
    p: &ast::TypePath,
    types: &mut TypeStore,
    builtins: BuiltinTypes,
) -> TypeId {
    let Some(last) = p.segments.last() else {
        return builtins.any;
    };
    let name = last.text(source);

    match name {
        "Any" => builtins.any,
        "String" => builtins.string,
        "Unit" => builtins.unit,
        "Nothing" => builtins.nothing,
        "Bool" => builtins.bool_,
        "Int" => builtins.int,
        "UInt" => builtins.uint,
        "Option" if p.args.len() == 1 => {
            let inner = lower_type_ref(source, &p.args[0], types, builtins);
            types.ty_option(inner)
        }
        _ => builtins.any,
    }
}

fn package_prefix(source: &SourceFile, package: Option<&ast::PackageDecl>) -> String {
    let Some(p) = package else {
        return String::new();
    };

    let mut out = String::new();
    for (idx, seg) in p.path.iter().enumerate() {
        if idx != 0 {
            out.push('.');
        }
        out.push_str(seg.text(source));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_minimal_file_smoke() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual("<mem>", "package a\nfun main() { val x: Int = 1; x }");

        let lowered = lower_for_dump(&sess, &src).unwrap();
        assert!(!lowered.file.items.is_empty());
    }
}
