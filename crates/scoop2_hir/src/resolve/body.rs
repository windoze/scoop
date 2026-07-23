//! Body 名字解析（resolve 第二阶段：函数体 / 顶层初始化器）。
//!
//! 在 header 收集（顶层声明已进 [`Index`]）与 import 收集之后，遍历函数体 /
//! 顶层 `val` 初始化器，把值位置的 `Ident` 绑定到具体符号：
//!
//! 解析顺序（非成员函数体）：**局部作用域**（参数 / 局部 `val`·`var` / 模式
//! 绑定 / lambda 参数 / `for`·`when`·`handle` 绑定）→ **当前包顶层**
//! `<pkg>.<name>` → **import**（显式优先于通配）；都不命中 →
//! `scoop::resolve::unresolved_value`。
//!
//! 当前覆盖：**顶层** `fun` 体（含 `= expr` 表达式体）与顶层 `val`(Name) 初始化器。
//! 类型体成员（成员函数 / `init` / 属性访问器，含 `this`/成员/扩展解析）在成员
//! 解析增量补齐——其名字解析复用本 walker，仅入口不同。
//!
//! 类型本身（`TypeRef`、`::class`、`is`/`as` 的类型、`StructLit`/`Call` 的类型
//! 实参）的解析是 type-ref lowering（typecheck 阶段），不在本模块。

use scoop2_base::diag::DiagnosticSink;
use scoop2_base::{Interner, NodeId, Symbol};

use crate::syntax::ast::{
    self, AnnotationUse, AssignTargetKind, Block, CallArg, Expr, ExprKind, File, FunBody, ItemKind,
    LambdaExpr, Pattern, PatternKind, Stmt, StmtKind, ValBinding, ValDecl,
};

use super::errors;
use super::imports::ImportTable;
use super::index::Index;
use super::output::{Resolution, ResolvedValue};
use super::scopes::{DefineOutcome, LocalBinding, ScopeStack};

/// 解析一个文件的**顶层**函数体 / 顶层 `val` 初始化器。
///
/// 需先完成 header 收集（`collect_file`）与 import 收集（`ImportTable::collect`）。
pub fn resolve_file_bodies(
    file: &File,
    index: &Index,
    imports: &ImportTable,
    interner: &Interner,
    diags: &mut DiagnosticSink,
    resolution: &mut Resolution,
    package_prefix: &str,
) {
    let mut r = BodyResolver {
        index,
        imports,
        interner,
        diags,
        resolution,
        package_prefix: package_prefix.to_string(),
        scopes: ScopeStack::new(),
    };
    for item in &file.items {
        match &item.kind {
            ItemKind::Fun(d) => {
                if let Some(body) = &d.body {
                    r.resolve_function(&d.params, body);
                }
            }
            ItemKind::Val(d) => {
                if let Some(init) = &d.init {
                    r.resolve_top_level_init(init, &d.binding, Some(item.id));
                }
            }
            // 扩展属性 / typealias / object / type 的成员体在成员解析增量处理。
            ItemKind::ExtensionProperty(_)
            | ItemKind::TypeAlias(_)
            | ItemKind::Object(_)
            | ItemKind::Type(_) => {}
        }
    }
}

struct BodyResolver<'a> {
    index: &'a Index,
    imports: &'a ImportTable,
    interner: &'a Interner,
    diags: &'a mut DiagnosticSink,
    resolution: &'a mut Resolution,
    package_prefix: String,
    scopes: ScopeStack,
}

impl<'a> BodyResolver<'a> {
    fn resolve_function(&mut self, params: &[ast::Param], body: &FunBody) {
        self.scopes.enter();
        for p in params {
            let outcome = self.scopes.define(
                p.name.symbol,
                LocalBinding {
                    decl: Some(p.id),
                    span: p.name.span,
                },
            );
            if let DefineOutcome::Redefined { prev } = outcome {
                self.report_duplicate(p.name.symbol, prev.span, p.name.span);
            }
        }
        match body {
            FunBody::Block(b) => self.walk_block(b),
            FunBody::Expr(e) => self.walk_expr(e),
        }
        self.scopes.leave();
    }

    fn resolve_top_level_init(&mut self, init: &Expr, binding: &ValBinding, decl: Option<NodeId>) {
        self.scopes.enter();
        self.walk_expr(init);
        self.bind_val_binding(binding, decl);
        self.scopes.leave();
    }

    // ---- 语句 / 块 ----

    fn walk_block(&mut self, block: &Block) {
        self.scopes.enter();
        for stmt in &block.stmts {
            self.walk_stmt(stmt);
        }
        self.scopes.leave();
    }

    fn walk_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Empty | StmtKind::Break | StmtKind::Continue => {}
            StmtKind::Expr(e) => self.walk_expr(e),
            StmtKind::Assign { target, value } => {
                self.walk_assign_target(&target.kind);
                self.walk_expr(value);
            }
            StmtKind::LocalVal(val) => self.walk_local_val(val, stmt),
            StmtKind::Return { value } => {
                if let Some(e) = value {
                    self.walk_expr(e);
                }
            }
            StmtKind::While { cond, body } => {
                self.walk_expr(cond);
                self.walk_block(body);
            }
            StmtKind::For { binder, iter, body } => {
                self.walk_expr(iter);
                self.scopes.enter();
                self.define_synthetic(binder.symbol, binder.span);
                self.walk_block(body);
                self.scopes.leave();
            }
        }
    }

    fn walk_local_val(&mut self, val: &ValDecl, stmt: &Stmt) {
        // 先解析 init（绑定未生效），再登记绑定。
        if let Some(init) = &val.init {
            self.walk_expr(init);
        }
        self.bind_val_binding(&val.binding, Some(stmt.id));
    }

    fn walk_assign_target(&mut self, kind: &AssignTargetKind) {
        match kind {
            AssignTargetKind::Ident(ident) => {
                // 赋值目标 Ident 无独立 Expr 节点 id，无法写回 value_refs；
                // 仅做 unresolved 检测（可写性由 typecheck 判定）。
                self.resolve_or_report(*ident);
            }
            AssignTargetKind::Member { receiver, .. } => self.walk_expr(receiver),
            AssignTargetKind::Index { receiver, indices } => {
                self.walk_expr(receiver);
                for i in indices {
                    self.walk_expr(i);
                }
            }
        }
    }

    // ---- 表达式 ----

    fn walk_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Ident(ident) => self.resolve_ident(*ident, expr.id),
            ExprKind::IntLit(_)
            | ExprKind::FloatLit(_)
            | ExprKind::CharLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::UnitLit => {}
            ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let ast::StringPart::Expr(e) = part {
                        self.walk_expr(e);
                    }
                }
            }
            ExprKind::TupleLit(els) | ExprKind::ArrayLit(els) => {
                for e in els {
                    self.walk_expr(e);
                }
            }
            ExprKind::StructLit { name: _, fields } => {
                for f in fields {
                    self.walk_expr(&f.value);
                }
            }
            ExprKind::Block(b) | ExprKind::DoBlock(b) => self.walk_block(b),
            ExprKind::UnsafeBlock(b) | ExprKind::SafeBlock(b) => self.walk_block(b),
            ExprKind::Lambda(lambda) => self.walk_lambda(lambda),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.walk_expr(cond);
                self.walk_expr(then_branch);
                if let Some(e) = else_branch {
                    self.walk_expr(e);
                }
            }
            ExprKind::When { subject, arms } => {
                self.walk_expr(subject);
                for arm in arms {
                    self.scopes.enter();
                    self.bind_pattern(&arm.pat);
                    if let Some(g) = &arm.guard {
                        self.walk_expr(g);
                    }
                    self.walk_expr(&arm.body);
                    self.scopes.leave();
                }
            }
            ExprKind::Handle {
                body,
                arms,
                finally,
            } => {
                self.walk_block(body);
                for arm in arms {
                    self.scopes.enter();
                    for b in &arm.op.binders {
                        self.define_synthetic(b.name.symbol, b.name.span);
                    }
                    if let Some(k) = &arm.escape_continuation {
                        self.define_synthetic(k.symbol, k.span);
                    }
                    self.walk_expr(&arm.body);
                    self.scopes.leave();
                }
                if let Some(f) = finally {
                    self.walk_block(f);
                }
            }
            ExprKind::MemberAccess { receiver, .. }
            | ExprKind::SafeMemberAccess { receiver, .. } => self.walk_expr(receiver),
            ExprKind::SpliceField { receiver, field } => {
                self.walk_expr(receiver);
                self.walk_expr(field);
            }
            ExprKind::Index { receiver, indices } => {
                self.walk_expr(receiver);
                for i in indices {
                    self.walk_expr(i);
                }
            }
            ExprKind::NotNullAssert { expr: e } => self.walk_expr(e),
            ExprKind::TypeApply { callee, .. } => self.walk_expr(callee),
            ExprKind::Call { callee, args } => {
                self.walk_expr(callee);
                for a in args {
                    self.walk_call_arg(a);
                }
            }
            ExprKind::ClassLit { path: _ } => {}
            ExprKind::Unary { expr: e, .. } => self.walk_expr(e),
            ExprKind::Binary { lhs, rhs, .. } => {
                self.walk_expr(lhs);
                self.walk_expr(rhs);
            }
            ExprKind::InfixCall { receiver, arg, .. } => {
                self.walk_expr(receiver);
                self.walk_expr(arg);
            }
            ExprKind::TypeCheck { expr: e, .. } | ExprKind::Cast { expr: e, .. } => {
                self.walk_expr(e)
            }
            ExprKind::WithUpdate { base, updates } => {
                self.walk_expr(base);
                for u in updates {
                    self.walk_expr(&u.value);
                }
            }
            ExprKind::Annotated { annotations, expr } => {
                for ann in annotations {
                    self.walk_annotation(ann);
                }
                self.walk_expr(expr);
            }
        }
    }

    fn walk_call_arg(&mut self, arg: &CallArg) {
        self.walk_expr(&arg.value);
    }

    fn walk_annotation(&mut self, ann: &AnnotationUse) {
        for a in &ann.args {
            self.walk_expr(&a.value);
        }
    }

    fn walk_lambda(&mut self, lambda: &LambdaExpr) {
        self.scopes.enter();
        for p in &lambda.params {
            let outcome = self.scopes.define(
                p.name.symbol,
                LocalBinding {
                    decl: Some(p.id),
                    span: p.name.span,
                },
            );
            if let DefineOutcome::Redefined { prev } = outcome {
                self.report_duplicate(p.name.symbol, prev.span, p.name.span);
            }
        }
        match &lambda.body {
            ast::LambdaBody::Block(b) => self.walk_block(b),
            ast::LambdaBody::Expr(e) => self.walk_expr(e),
        }
        self.scopes.leave();
    }

    // ---- 名字解析 ----

    fn resolve_ident(&mut self, ident: ast::Ident, expr_id: NodeId) {
        let name_text = self.interner.resolve(ident.symbol);
        // `true` / `false` 是普通 IDENT，由 typecheck 解释为 Bool；不在此报 unresolved。
        if name_text == "true" || name_text == "false" {
            return;
        }
        // 1. 局部
        if let Some(local) = self.scopes.resolve(ident.symbol) {
            self.resolution
                .value_refs
                .set(expr_id, ResolvedValue::Local { decl: local.decl });
            return;
        }
        // 2. 当前包顶层 <pkg>.<name>
        let pkg_fqn = self.current_package_lookup(ident.symbol);
        if let Some(fqn) = pkg_fqn
            && self.record_top_level(expr_id, fqn)
        {
            return;
        }
        // 3. import
        let imp_fqn = self
            .imports
            .resolve_name(ident.symbol, self.index, self.interner);
        if let Some(fqn) = imp_fqn
            && self.record_top_level(expr_id, fqn)
        {
            return;
        }
        // 4. unresolved
        self.diags
            .push(errors::unresolved_value(name_text, ident.span));
    }

    /// 仅做 unresolved 检测（不写回 value_refs），用于赋值目标等无 Expr 节点的位置。
    fn resolve_or_report(&mut self, ident: ast::Ident) {
        let name_text = self.interner.resolve(ident.symbol);
        if name_text == "true" || name_text == "false" {
            return;
        }
        if self.scopes.resolve(ident.symbol).is_some() {
            return;
        }
        if self.current_package_lookup(ident.symbol).is_some() {
            return;
        }
        if self
            .imports
            .resolve_name(ident.symbol, self.index, self.interner)
            .is_some()
        {
            return;
        }
        self.diags
            .push(errors::unresolved_value(name_text, ident.span));
    }

    /// 在当前包前缀下探测 `<pkg>.<name>` 是否存在于 Index；返回其 FQN。
    fn current_package_lookup(&self, name: Symbol) -> Option<Symbol> {
        let name_text = self.interner.resolve(name);
        let fqn_text = if self.package_prefix.is_empty() {
            name_text.to_string()
        } else {
            format!("{}.{}", self.package_prefix, name_text)
        };
        let fqn = self.interner.get(&fqn_text)?;
        if self.index.lookup(fqn).is_some() {
            Some(fqn)
        } else {
            None
        }
    }

    /// 记录一个顶层 FQN 到 value_refs（value 优先于 fun）；返回是否记录成功。
    fn record_top_level(&mut self, expr_id: NodeId, fqn: Symbol) -> bool {
        let Some(ns) = self.index.lookup(fqn) else {
            return false;
        };
        if ns.value.is_some() {
            self.resolution
                .value_refs
                .set(expr_id, ResolvedValue::TopLevelValue { fqn });
            true
        } else if !ns.funs.is_empty() {
            self.resolution
                .value_refs
                .set(expr_id, ResolvedValue::TopLevelFun { fqn });
            true
        } else {
            // 命中纯类型命名空间：值位置引用类型名由 typecheck 判定，不记录不报错。
            false
        }
    }

    // ---- 绑定登记 ----

    fn report_duplicate(
        &mut self,
        name: Symbol,
        first: scoop2_base::Span,
        second: scoop2_base::Span,
    ) {
        let text = self.interner.resolve(name).to_string();
        self.diags
            .push(errors::duplicate_definition(&text, first, second));
    }

    /// 登记一个无专属节点的合成绑定（`for` / `when` / `handle` / 模式绑定）。
    fn define_synthetic(&mut self, name: Symbol, span: scoop2_base::Span) {
        let outcome = self.scopes.define(name, LocalBinding { decl: None, span });
        if let DefineOutcome::Redefined { prev } = outcome {
            self.report_duplicate(name, prev.span, span);
        }
    }

    /// 登记 `val` 绑定（Name 或解构模式）。
    fn bind_val_binding(&mut self, binding: &ValBinding, decl: Option<NodeId>) {
        match binding {
            ValBinding::Name(name) => {
                let outcome = self.scopes.define(
                    name.symbol,
                    LocalBinding {
                        decl,
                        span: name.span,
                    },
                );
                if let DefineOutcome::Redefined { prev } = outcome {
                    self.report_duplicate(name.symbol, prev.span, name.span);
                }
            }
            ValBinding::Pattern(p) => self.bind_pattern(p),
        }
    }

    fn bind_pattern(&mut self, pat: &Pattern) {
        let mut names: Vec<(Symbol, scoop2_base::Span)> = Vec::new();
        collect_pattern_binders(pat, &mut names);
        for (name, span) in names {
            self.define_synthetic(name, span);
        }
    }
}

/// 收集一个模式中的所有绑定名（`Bind` / 结构体简写字段）。
fn collect_pattern_binders(pat: &Pattern, out: &mut Vec<(Symbol, scoop2_base::Span)>) {
    match &pat.kind {
        PatternKind::Wildcard
        | PatternKind::Literal(_)
        | PatternKind::Rest
        | PatternKind::Is(_)
        | PatternKind::Else => {}
        PatternKind::Bind(name) => out.push((name.symbol, name.span)),
        PatternKind::Tuple(elems) => {
            for e in elems {
                collect_pattern_binders(e, out);
            }
        }
        PatternKind::Struct { fields, .. } => {
            for f in fields {
                match &f.pattern {
                    None => out.push((f.name.symbol, f.name.span)),
                    Some(p) => collect_pattern_binders(p, out),
                }
            }
        }
        PatternKind::Variant { args, .. } => {
            if let Some(elems) = args {
                for e in elems {
                    collect_pattern_binders(e, out);
                }
            }
        }
        PatternKind::Or(alts) => {
            for a in alts {
                collect_pattern_binders(a, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::collect::collect_file;
    use crate::resolve::imports::ImportTable;
    use crate::resolve::symbol::ConeKind;
    use scoop2_base::{FileId, SourceFile};
    use scoop2_syntax::parser::parse_file;

    /// 解析 + 收集 + import + body 解析；返回 (resolution, 诊断码)。
    fn resolve_bodies(src: &str) -> (Resolution, Vec<String>) {
        let result = parse_file(&SourceFile::new_virtual("<mem>", src));
        let mut interner = result.interner;
        let mut index = Index::new();
        let mut diags = DiagnosticSink::new();
        let cone = index.intern_cone("test", ConeKind::Bin);
        let prefix = package_prefix_of(&result.file, &interner);
        collect_file(
            &result.file,
            FileId(0),
            cone,
            &mut index,
            &mut interner,
            &mut diags,
        );
        let imports =
            ImportTable::collect(&result.file, FileId(0), &index, &mut interner, &mut diags);
        let mut resolution = Resolution::new();
        resolve_file_bodies(
            &result.file,
            &index,
            &imports,
            &interner,
            &mut diags,
            &mut resolution,
            &prefix,
        );
        let codes = diags.iter().map(|d| d.code.to_string()).collect();
        (resolution, codes)
    }

    fn package_prefix_of(file: &File, interner: &Interner) -> String {
        match &file.package {
            Some(pkg) => pkg
                .path
                .segments
                .iter()
                .map(|seg| interner.resolve(seg.symbol))
                .collect::<Vec<_>>()
                .join("."),
            None => String::new(),
        }
    }

    #[test]
    fn resolves_local_param_and_top_level() {
        let (res, codes) = resolve_bodies("fun f(x: Int) { g(x) }\nfun g(y: Int) {}\n");
        assert!(codes.is_empty(), "{codes:?}");
        assert!(
            res.value_refs.len() >= 2,
            "expected >=2 refs, got {}",
            res.value_refs.len()
        );
    }

    #[test]
    fn reports_unresolved_value() {
        let (_, codes) = resolve_bodies("fun f() { missing }\n");
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::resolve::unresolved_value"),
            "{codes:?}"
        );
    }

    #[test]
    fn local_val_binds_and_resolves() {
        let (res, codes) = resolve_bodies("fun g() {}\nfun f() { val a = g\n a }\n");
        assert!(codes.is_empty(), "{codes:?}");
        assert!(res.value_refs.len() >= 2);
    }

    #[test]
    fn lambda_params_in_scope() {
        let (_, codes) = resolve_bodies("fun f() { val h = { x -> x }\n h }\n");
        assert!(codes.is_empty(), "lambda param x in scope: {codes:?}");
    }

    #[test]
    fn for_binder_in_scope_iter_unresolved() {
        // xs 未定义 → unresolved_value（binder x 在作用域内，不报错）。
        let (_, codes) = resolve_bodies("fun f() { for (x in xs) { x } }\n");
        assert!(
            codes
                .iter()
                .any(|c| c == "scoop::resolve::unresolved_value"),
            "{codes:?}"
        );
    }

    #[test]
    fn when_pattern_binds_in_arm() {
        // when on Option：Some(x) 绑定 x；arms 体引用 x 合法。但 Option/Some 是 prelude
        // 类型，未加载时 subject 的 opt 解析为 unresolved。这里只关心 x 不被误报。
        // 用一个不依赖 prelude 的结构：when on 一个局部。
        let (_, codes) = resolve_bodies("fun f() { val v = 1\n when (v) { else -> v } }\n");
        assert!(
            codes.is_empty(),
            "when else-arm + subject resolve: {codes:?}"
        );
    }
}
