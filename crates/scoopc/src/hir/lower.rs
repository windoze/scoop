//! AST → HIR 的最小 lowering（TODO T0701）。
//!
//! 说明：
//! - 这里的 lowering 仅用于 `dump-hir` 的调试输出，因此实现上优先保证“稳定输出 + 不 panic”；。
//! - 完整 lowering（含类型推断结果、更多语法节点）会在后续任务（TODO T0702+）逐步补齐。

use std::collections::HashMap;

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::parser::{ParseError, parse_file};
use crate::resolve::{Index, ResolveError};
use crate::session::Session;
use crate::source::SourceFile;
use crate::ty::{
    BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore,
    ValueTypeKind,
};

use super::{
    Block, CallArg, Expr, ExprKind, File, FunDecl, Item, LiteralKind, Param, Stmt, StmtKind,
    SymbolId, ValDecl, ValueRef,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum SymbolKey {
    Local { decl_span: crate::span::Span },
    TopLevel { fqn: String },
}

/// 一个最小的 symbol interner：把“解析后的符号键”映射为一个紧凑的 `SymbolId`。
///
/// 说明：
/// - 该表仅用于 HIR dump/fixtures 的稳定标识，并不试图提供跨 session 的全局稳定性；
/// - `SymbolId` 的分配顺序依赖 traversal 顺序，但 traversal 对同一个 AST 是确定的，因此 golden 可回归。
#[derive(Debug, Default)]
struct SymbolInterner {
    next: u32,
    by_key: HashMap<SymbolKey, SymbolId>,
}

impl SymbolInterner {
    fn intern_local(&mut self, decl_span: crate::span::Span) -> SymbolId {
        self.intern(SymbolKey::Local { decl_span })
    }

    fn intern_top_level(&mut self, fqn: String) -> SymbolId {
        self.intern(SymbolKey::TopLevel { fqn })
    }

    fn intern(&mut self, key: SymbolKey) -> SymbolId {
        if let Some(id) = self.by_key.get(&key).copied() {
            return id;
        }

        let id = SymbolId(self.next);
        self.next = self.next.saturating_add(1);
        self.by_key.insert(key, id);
        id
    }
}

/// HIR lowering 的上下文（按单文件构建，用于 `dump-hir` 与 HIR fixtures）。
struct HirLowering<'a> {
    source: &'a SourceFile,
    file: &'a ast::File,
    index: &'a Index,
    /// `type fqn -> ast::TypeKind` 的最小索引，用于决定 nominal type 是 ref 还是 value。
    type_kinds: &'a HashMap<String, ast::TypeKind>,
    symbols: SymbolInterner,
    /// 类型表（HIR 内所有 `TypeId` 必须来自同一个 store）。
    types: &'a mut TypeStore,
    builtins: BuiltinTypes,
    /// type parameter 作用域栈：用于 lowering `T` 这类抽象类型引用。
    type_param_scopes: Vec<HashMap<String, TypeId>>,
}

impl<'a> HirLowering<'a> {
    fn new(
        source: &'a SourceFile,
        file: &'a ast::File,
        index: &'a Index,
        type_kinds: &'a HashMap<String, ast::TypeKind>,
        types: &'a mut TypeStore,
        builtins: BuiltinTypes,
    ) -> Self {
        Self {
            source,
            file,
            index,
            type_kinds,
            symbols: SymbolInterner::default(),
            types,
            builtins,
            type_param_scopes: Vec::new(),
        }
    }

    fn lower_file(&mut self) -> File {
        let pkg_prefix = package_prefix(self.source, self.file.package.as_ref());
        let mut items = Vec::with_capacity(self.file.items.len());

        for item in &self.file.items {
            items.push(self.lower_item(&pkg_prefix, item));
        }

        File { items }
    }

    fn lower_item(&mut self, pkg_prefix: &str, item: &ast::Item) -> Item {
        match item {
            ast::Item::Fun(fun) => Item::Fun(self.lower_fun_decl(pkg_prefix, fun)),
            ast::Item::Val(v) => Item::Val(self.lower_val_decl(pkg_prefix, v, ValScope::TopLevel)),
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

    fn lower_fun_decl(&mut self, pkg_prefix: &str, fun: &ast::FunDecl) -> FunDecl {
        // 进入函数作用域：先把 type params lower 成 `TypeId`，保证签名与 body 内引用一致。
        self.push_type_params(&fun.type_params);

        let name = fun.name.text(self.source).to_string();
        let fqn = if pkg_prefix.is_empty() {
            name.clone()
        } else {
            format!("{pkg_prefix}.{name}")
        };

        let params: Vec<Param> = fun
            .params
            .iter()
            .map(|p| {
                let name = p.name.text(self.source).to_string();
                let id = self.symbols.intern_local(p.name.span);
                let ty = p
                    .ty
                    .as_ref()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
                Param {
                    span: p.name.span,
                    id,
                    name,
                    ty,
                }
            })
            .collect();

        let receiver_ty = fun.receiver.as_ref().map(|t| self.lower_type_ref(t));

        // 当前阶段：未接入返回类型推断，缺省时用 `Any` 占位。
        let return_ty = fun
            .return_ty
            .as_ref()
            .map(|t| self.lower_type_ref(t))
            .unwrap_or(self.builtins.any);

        let effects = self.lower_effect_row_expr(fun.effects.as_ref());
        let ty = self.types.ty_function(
            receiver_ty,
            params.iter().map(|p| p.ty).collect(),
            return_ty,
            effects,
        );

        let body = match &fun.body {
            ast::FunBody::Block(b) => Some(self.lower_block(pkg_prefix, b)),
            ast::FunBody::Missing => None,
        };

        self.pop_type_params();

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

    fn lower_val_decl(&mut self, pkg_prefix: &str, v: &ast::ValDecl, scope: ValScope) -> ValDecl {
        let init = v.init.as_ref().map(|e| self.lower_expr(pkg_prefix, e));

        let declared_ty = v.ty.as_ref().map(|t| self.lower_type_ref(t));

        let ty = declared_ty
            .or_else(|| init.as_ref().map(|e| e.ty))
            .unwrap_or(self.builtins.any);

        let (id, name) = match v.name() {
            Some(id) => {
                let name = id.text(self.source).to_string();
                let sym = match scope {
                    ValScope::TopLevel => {
                        let fqn = if pkg_prefix.is_empty() {
                            name.clone()
                        } else {
                            format!("{pkg_prefix}.{name}")
                        };
                        self.symbols.intern_top_level(fqn)
                    }
                    ValScope::Local => self.symbols.intern_local(id.span),
                };
                (Some(sym), Some(name))
            }
            None => (None, None),
        };

        ValDecl {
            span: v.span,
            id,
            name,
            mutable: v.kind == ast::ValKind::Var,
            ty,
            init,
        }
    }

    fn lower_block(&mut self, pkg_prefix: &str, b: &ast::Block) -> Block {
        let mut stmts = Vec::with_capacity(b.stmts.len());
        for s in &b.stmts {
            stmts.push(self.lower_stmt(pkg_prefix, s));
        }

        // 当前阶段：用 block 最后一条“表达式语句”的类型作为 block 类型，否则视为 Unit。
        let ty = stmts
            .last()
            .and_then(|s| match &s.kind {
                StmtKind::Expr(e) => Some(e.ty),
                _ => None,
            })
            .unwrap_or(self.builtins.unit);

        Block {
            span: b.span,
            ty,
            stmts,
        }
    }

    fn lower_stmt(&mut self, pkg_prefix: &str, s: &ast::Stmt) -> Stmt {
        let (kind, ty) = match &s.kind {
            ast::StmtKind::Empty => (StmtKind::Empty, self.builtins.unit),
            ast::StmtKind::Expr(e) => {
                let e = self.lower_expr(pkg_prefix, e);
                (StmtKind::Expr(e), self.builtins.unit)
            }
            ast::StmtKind::Val(v) => {
                let v = self.lower_val_decl(pkg_prefix, v, ValScope::Local);
                (StmtKind::Val(v), self.builtins.unit)
            }
            ast::StmtKind::Return { value, .. } => {
                let value = value.as_ref().map(|e| self.lower_expr(pkg_prefix, e));
                (StmtKind::Return { value }, self.builtins.nothing)
            }
            ast::StmtKind::Missing => (StmtKind::Todo("missing_stmt"), self.builtins.unit),
            ast::StmtKind::While { .. } => (StmtKind::Todo("while"), self.builtins.unit),
            ast::StmtKind::Break { .. } => (StmtKind::Todo("break"), self.builtins.unit),
            ast::StmtKind::Continue { .. } => (StmtKind::Todo("continue"), self.builtins.unit),
            ast::StmtKind::ComptimeBlock { .. } => {
                (StmtKind::Todo("comptime_block"), self.builtins.unit)
            }
            ast::StmtKind::ComptimeIf(_) => (StmtKind::Todo("comptime_if"), self.builtins.unit),
            ast::StmtKind::ComptimeFor(_) => (StmtKind::Todo("comptime_for"), self.builtins.unit),
        };

        Stmt {
            span: s.span,
            ty,
            kind,
        }
    }

    fn lower_expr(&mut self, pkg_prefix: &str, e: &ast::Expr) -> Expr {
        let (kind, ty) = match &e.kind {
            ast::ExprKind::Missing => (ExprKind::Missing, self.builtins.any),
            ast::ExprKind::IntLit => (ExprKind::Literal(LiteralKind::Int), self.builtins.int),
            ast::ExprKind::StringLit => (ExprKind::Literal(LiteralKind::String), self.builtins.string),
            ast::ExprKind::UnitLit => (ExprKind::Literal(LiteralKind::Unit), self.builtins.unit),
            ast::ExprKind::InterpolatedString { .. } => {
                (ExprKind::Literal(LiteralKind::String), self.builtins.string)
            }
            ast::ExprKind::Ident(id) => self.lower_ident_expr(id),
            ast::ExprKind::Block(b) => {
                let b = self.lower_block(pkg_prefix, b);
                let ty = b.ty;
                (ExprKind::Block(b), ty)
            }
            ast::ExprKind::Call { callee, args } => {
                let callee = Box::new(self.lower_expr(pkg_prefix, callee));
                let args = args.iter().map(|arg| self.lower_call_arg(pkg_prefix, arg)).collect();
                (ExprKind::Call { callee, args }, self.builtins.any)
            }
            ast::ExprKind::NamedArg { .. } => (ExprKind::Todo("named_arg"), self.builtins.any),
            ast::ExprKind::TupleLit { .. } => (ExprKind::Todo("tuple_lit"), self.builtins.any),
            ast::ExprKind::Lambda(_) => (ExprKind::Todo("lambda"), self.builtins.any),
            ast::ExprKind::StructLit { .. } => (ExprKind::Todo("struct_lit"), self.builtins.any),
            ast::ExprKind::If { .. } => (ExprKind::Todo("if"), self.builtins.any),
            ast::ExprKind::When { .. } => (ExprKind::Todo("when"), self.builtins.any),
            ast::ExprKind::Handle { .. } => (ExprKind::Todo("handle"), self.builtins.any),
            ast::ExprKind::MemberAccess { .. } => (ExprKind::Todo("member_access"), self.builtins.any),
            ast::ExprKind::SpliceField { .. } => (ExprKind::Todo("splice_field"), self.builtins.any),
            ast::ExprKind::SafeMemberAccess { .. } => {
                (ExprKind::Todo("safe_member_access"), self.builtins.any)
            }
            ast::ExprKind::NotNullAssert { .. } => {
                (ExprKind::Todo("not_null_assert"), self.builtins.any)
            }
            ast::ExprKind::Unary { .. } => (ExprKind::Todo("unary"), self.builtins.any),
            ast::ExprKind::Binary { .. } => (ExprKind::Todo("binary"), self.builtins.any),
            ast::ExprKind::Assign { .. } => (ExprKind::Todo("assign"), self.builtins.any),
            ast::ExprKind::TypeCheck { .. } => (ExprKind::Todo("type_check"), self.builtins.any),
            ast::ExprKind::Cast { .. } => (ExprKind::Todo("cast"), self.builtins.any),
            ast::ExprKind::WithUpdate { .. } => (ExprKind::Todo("with_update"), self.builtins.any),
        };

        Expr {
            span: e.span,
            ty,
            kind,
        }
    }

    fn lower_call_arg(&mut self, pkg_prefix: &str, arg: &ast::Expr) -> CallArg {
        match &arg.kind {
            ast::ExprKind::NamedArg { name, value, .. } => CallArg::Named {
                name: name.text(self.source).to_string(),
                name_span: name.span,
                value: self.lower_expr(pkg_prefix, value),
            },
            _ => CallArg::Positional(self.lower_expr(pkg_prefix, arg)),
        }
    }

    fn lower_ident_expr(&mut self, id: &ast::ValueIdent) -> (ExprKind, TypeId) {
        let text = self.source.slice(id.span);
        if text == "true" {
            return (ExprKind::Literal(LiteralKind::Bool(true)), self.builtins.bool_);
        }
        if text == "false" {
            return (ExprKind::Literal(LiteralKind::Bool(false)), self.builtins.bool_);
        }

        let Some(resolved) = id.resolved.as_ref() else {
            return (ExprKind::Todo("unresolved_ident"), self.builtins.any);
        };

        let resolved = match resolved {
            ast::ResolvedValueRef::Local { name, decl_span } => ValueRef::Local {
                id: self.symbols.intern_local(*decl_span),
                name: name.clone(),
                decl_span: *decl_span,
            },
            ast::ResolvedValueRef::TopLevel { fqn } => ValueRef::TopLevel {
                id: self.symbols.intern_top_level(fqn.clone()),
                fqn: fqn.clone(),
            },
        };

        (ExprKind::VarRef(resolved), self.builtins.any)
    }

    fn lower_type_ref(&mut self, t: &ast::TypeRef) -> TypeId {
        match t {
            ast::TypeRef::Path(p) => self.lower_type_path(p),
            ast::TypeRef::Tuple(tt) => {
                if tt.elements.is_empty() {
                    return self.builtins.unit;
                }
                let elements = tt.elements.iter().map(|e| self.lower_type_ref(e)).collect();
                self.types.ty_tuple(elements)
            }
            ast::TypeRef::Nullable { inner, .. } => {
                let inner = self.lower_type_ref(inner);
                self.types.ty_option(inner)
            }
            ast::TypeRef::Function(fun) => {
                let receiver = fun.receiver.as_ref().map(|r| self.lower_type_ref(r));
                let params = fun.params.iter().map(|p| self.lower_type_ref(p)).collect();
                let return_ty = self.lower_type_ref(&fun.return_ty);
                let effects = self.lower_effect_row_expr(fun.effects.as_ref());
                self.types.ty_function(receiver, params, return_ty, effects)
            }
            ast::TypeRef::Star { .. } | ast::TypeRef::EffectRowArg { .. } => self.builtins.any,
        }
    }

    fn lower_type_path(&mut self, p: &ast::TypePath) -> TypeId {
        // 单段名且无实参：优先解析为当前作用域的 type parameter。
        if p.segments.len() == 1 && p.args.is_empty() {
            let name = p.segments[0].text(self.source);
            if let Some(id) = self.lookup_type_param(name) {
                return id;
            }
        }

        let fqn = self
            .index
            .type_ref_to_fqn_in_file(self.source, self.file, &ast::TypeRef::Path(p.clone()));

        let Some(fqn) = fqn else {
            return self.builtins.any;
        };

        // 分离：普通 type args vs use-site effect row arg（`eff ...`）。
        let mut eff_arg: Option<&ast::EffectRowExpr> = None;
        let mut type_args: Vec<&ast::TypeRef> = Vec::new();
        for a in &p.args {
            match a {
                ast::TypeRef::EffectRowArg { row, .. } => {
                    eff_arg.get_or_insert(row);
                }
                other => type_args.push(other),
            }
        }

        // 少数 builtin/special-case：不走 nominal。
        match fqn.as_str() {
            "scoop.core.Any" => return self.builtins.any,
            "scoop.core.String" => return self.builtins.string,
            "scoop.core.Unit" => return self.builtins.unit,
            "scoop.core.Nothing" => return self.builtins.nothing,
            "scoop.core.Bool" => return self.builtins.bool_,
            "scoop.core.Int" => return self.builtins.int,
            "scoop.core.UInt" => return self.builtins.uint,
            "scoop.core.Option" => {
                let inner = type_args
                    .first()
                    .map(|t| self.lower_type_ref(t))
                    .unwrap_or(self.builtins.any);
                return self.types.ty_option(inner);
            }
            _ => {}
        }

        // `Int32`/`UInt64` 这类固定位宽整数：若出现在 sysroot/type env 中，直接 lowering 为内建整数族。
        if let Some(bits) = fqn.strip_prefix("scoop.core.Int").and_then(|s| s.parse::<u16>().ok()) {
            return self.types.ty_int_n(bits);
        }
        if let Some(bits) = fqn.strip_prefix("scoop.core.UInt").and_then(|s| s.parse::<u16>().ok()) {
            return self.types.ty_uint_n(bits);
        }

        let args = type_args.iter().map(|a| self.lower_type_ref(a)).collect::<Vec<_>>();
        let eff = eff_arg.map(|e| self.lower_effect_row_expr(Some(e)));
        self.intern_nominal(fqn, args, eff)
    }

    fn lower_effect_row_expr(&mut self, expr: Option<&ast::EffectRowExpr>) -> EffectRow {
        let Some(expr) = expr else {
            return EffectRow::pure();
        };
        if expr.terms.is_empty() {
            return EffectRow::pure();
        }

        let mut terms: Vec<TypeId> = Vec::with_capacity(expr.terms.len());
        for term in &expr.terms {
            terms.push(self.lower_type_path(term));
        }
        EffectRow::new(terms)
    }

    fn intern_nominal(&mut self, fqn: String, args: Vec<TypeId>, eff: Option<EffectRow>) -> TypeId {
        let nominal = NominalType { fqn, args, eff };

        // 尝试用 `type_kinds` 判断 struct/enum（value type）vs class/interface/effect（ref type）。
        let kind = self.type_kinds.get(&nominal.fqn).copied();
        match kind {
            Some(ast::TypeKind::Struct | ast::TypeKind::Enum) => self
                .types
                .intern(TypeKind::Value(ValueTypeKind::Nominal(nominal))),
            _ => self
                .types
                .intern(TypeKind::Ref(RefTypeKind::Nominal(nominal))),
        }
    }

    fn push_type_params(&mut self, params: &[ast::TypeParam]) {
        if params.is_empty() {
            self.type_param_scopes.push(HashMap::new());
            return;
        }

        let decl_file = self.source.path().to_path_buf();
        let mut frame = HashMap::new();
        for p in params {
            let name = p.name.text(self.source).to_string();
            let id = self.types.ty_param(TypeParamType {
                name: name.clone(),
                decl_file: decl_file.clone(),
                decl_span: p.name.span,
            });
            frame.insert(name, id);
        }
        self.type_param_scopes.push(frame);
    }

    fn pop_type_params(&mut self) {
        let _ = self.type_param_scopes.pop();
    }

    fn lookup_type_param(&self, name: &str) -> Option<TypeId> {
        self.type_param_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValScope {
    TopLevel,
    Local,
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

    let index = {
        // 注意：`check_file_bodies` 需要 `&mut ast`，因此这里把构建 index 的临时借用放在独立作用域中，
        // 避免把 `&ast` 存到更长生命周期的容器里导致借用冲突。
        let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
        for f in &session.sysroot().files {
            pairs.push((&f.source, &f.ast));
        }
        pairs.push((source, &ast));
        Index::build(&pairs)?
    };

    let headers = crate::resolve::check_file_headers(source, &ast, &index)?;
    crate::resolve::check_file_bodies(source, &mut ast, &index, &headers)?;

    let mut pairs: Vec<(&SourceFile, &ast::File)> = Vec::new();
    for f in &session.sysroot().files {
        pairs.push((&f.source, &f.ast));
    }
    pairs.push((source, &ast));
    let type_kinds = collect_type_decl_kinds(&pairs);

    let mut types = TypeStore::new();
    let builtins = types.intern_builtins();

    let mut ctx = HirLowering::new(source, &ast, &index, &type_kinds, &mut types, builtins);
    let file = ctx.lower_file();
    Ok(LoweredHir { file, types })
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

fn collect_type_decl_kinds(pairs: &[(&SourceFile, &ast::File)]) -> HashMap<String, ast::TypeKind> {
    let mut out: HashMap<String, ast::TypeKind> = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            let ast::Item::Type(ty) = item else {
                continue;
            };
            let name = ty.name.text(source).to_string();
            let fqn = if pkg_prefix.is_empty() {
                name
            } else {
                format!("{pkg_prefix}.{name}")
            };
            out.insert(fqn, ty.kind);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn lower_minimal_file_smoke() {
        let sess = Session::new().unwrap();
        let src = SourceFile::new_virtual("<mem>", "package a\nfun main() { val x: Int = 1; x }");

        let lowered = lower_for_dump(&sess, &src).unwrap();
        assert!(!lowered.file.items.is_empty());
    }

    #[test]
    fn hir_fixture_minimal_golden() {
        let sess = Session::new().unwrap();

        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir/minimal.scoop");
        let file = SourceFile::load(&fixture_path).unwrap();

        let lowered = lower_for_dump(&sess, &file).unwrap();
        let actual = format!("{:#?}\n", lowered.file);

        let golden_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/hir/minimal.hir");
        let expected = std::fs::read_to_string(&golden_path).unwrap();

        assert_eq!(actual, expected);
    }
}
