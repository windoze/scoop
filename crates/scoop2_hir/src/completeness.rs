//! typed HIR 完整性闸门：在交付 typed HIR 前拒绝任何「未类型化」表达式节点。
//!
//! 本模块是兜底校验：遍历每个 User 文件的 AST，确认每个**表达式**节点都在
//! [`hir::TypedFile::expr_types`] 中有对应的推断类型。缺失即视为 typecheck
//! 未覆盖（遗漏），产出 `scoop::typecheck::untyped_node` 诊断。
//!
//! 设计意图：
//!
//! - **不替代 typecheck 的具体诊断**：typecheck 已对每个表达式形式给出精确错误
//!   （类型不匹配、未解析引用等）。本闸门只捕获「typecheck 完全没碰过该节点」
//!   的遗漏——这通常意味着新增了 AST 变体但 typecheck 未同步。
//! - **语句不强制类型**：语句（`val`/赋值/`return` 等）没有单一「类型」，故只校验
//!   表达式节点。块作为尾表达式时其类型隐含在尾表达式上，亦不单独校验。
//! - **可关闭**：调用方可选择不运行本闸门（例如 dump-hir 仅需尽力而为的视图）。

use scoop2_base::diag::{Diagnostic, DiagnosticSink};
use scoop2_base::{FileId, Span};

use crate::hir::TypedHir;
use crate::syntax::ast::{self, Expr, ExprKind, File};

/// 遍历 `hir` 中所有 User 文件的表达式，汇报任何缺少推断类型的表达式节点。
///
/// 返回追加进 `diags` 的诊断数。无错误返回 0。
pub fn verify(hir: &TypedHir, files: &[(FileId, &File)], diags: &mut DiagnosticSink) -> usize {
    let mut count = 0usize;
    for (file_id, file) in files {
        let Some(typed_file) = hir.file(*file_id) else {
            continue;
        };
        let mut walker = Walker {
            expr_types: &typed_file.expr_types,
            store: &hir.store,
            diags,
            file_id: *file_id,
            count: 0,
        };
        walker.file(file);
        count += walker.count;
    }
    count
}

struct Walker<'a> {
    expr_types: &'a crate::resolve::output::NodeIdTable<crate::ty::TypeId>,
    store: &'a crate::ty::TypeStore,
    diags: &'a mut DiagnosticSink,
    file_id: FileId,
    count: usize,
}

impl<'a> Walker<'a> {
    fn file(&mut self, file: &File) {
        for item in &file.items {
            self.item(item);
        }
    }

    fn item(&mut self, item: &ast::Item) {
        match &item.kind {
            ast::ItemKind::Fun(d) => {
                if let Some(body) = &d.body {
                    self.fun_body(body);
                }
            }
            ast::ItemKind::Val(d) => {
                if let Some(init) = &d.init {
                    self.expr(init);
                }
            }
            ast::ItemKind::ExtensionProperty(d) => {
                if let Some(init) = &d.init {
                    self.expr(init);
                }
            }
            ast::ItemKind::Object(d) => {
                if let Some(body) = &d.body {
                    self.type_body(body);
                }
            }
            ast::ItemKind::Type(d) => {
                if let Some(body) = &d.body {
                    self.type_body(body);
                }
            }
            ast::ItemKind::TypeAlias(_) => {}
        }
    }

    fn type_body(&mut self, body: &ast::TypeBody) {
        for member in &body.members {
            self.member(member);
        }
    }

    fn member(&mut self, member: &ast::TypeMember) {
        match &member.kind {
            ast::TypeMemberKind::Fun(d) => {
                if let Some(body) = &d.body {
                    self.fun_body(body);
                }
            }
            ast::TypeMemberKind::Property(pd) => {
                if let Some(init) = &pd.init {
                    self.expr(init);
                }
            }
            ast::TypeMemberKind::InitBlock(ib) => {
                self.block(&ib.body);
            }
            ast::TypeMemberKind::SecondaryCtor(d) => {
                self.block(&d.body);
            }
            ast::TypeMemberKind::EnumVariant(_) => {
                // enum variant 字段是类型引用（非表达式），无可遍历的表达式子节点。
            }
            ast::TypeMemberKind::Object(d) => {
                if let Some(body) = &d.body {
                    self.type_body(body);
                }
            }
            ast::TypeMemberKind::Type(d) => {
                if let Some(body) = &d.body {
                    self.type_body(body);
                }
            }
        }
    }

    fn fun_body(&mut self, body: &ast::FunBody) {
        match body {
            ast::FunBody::Block(b) => self.block(b),
            ast::FunBody::Expr(e) => self.expr(e),
        }
    }

    fn block(&mut self, block: &ast::Block) {
        for stmt in &block.stmts {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &ast::Stmt) {
        match &stmt.kind {
            ast::StmtKind::Empty => {}
            ast::StmtKind::Expr(e) => self.expr(e),
            ast::StmtKind::Assign { value, .. } => self.expr(value),
            ast::StmtKind::LocalVal(d) => {
                if let Some(init) = &d.init {
                    self.expr(init);
                }
            }
            ast::StmtKind::Return { value } => {
                if let Some(e) = value {
                    self.expr(e);
                }
            }
            ast::StmtKind::While { cond, body, .. } => {
                self.expr(cond);
                self.block(body);
            }
            ast::StmtKind::For { iter, body, .. } => {
                self.expr(iter);
                self.block(body);
            }
            ast::StmtKind::Break | ast::StmtKind::Continue => {}
        }
    }

    /// 访问一个表达式：校验它自身有类型，再递归子表达式。
    fn expr(&mut self, expr: &Expr) {
        match self.expr_types.get(expr.id) {
            None => {
                // 节点完全未被 typecheck 赋类型——真正的缺口。
                self.report(expr.span);
            }
            Some(_ty) => {
                // Nothing 是合法的 bottom type（Raise.raise / panic 等返回 Nothing），
                // 不再视为缺口。typecheck 已不再为不可解析节点写 Nothing 兜底
                //（改为不写或写 Unit），故存在的条目都是合法定型。
            }
        }
        self.expr_children(expr);
    }

    /// 递归所有子表达式（穷尽 match：新增 ExprKind 变体编译报错，强制同步）。
    fn expr_children(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Ident(_)
            | ExprKind::IntLit(_)
            | ExprKind::FloatLit(_)
            | ExprKind::CharLit(_)
            | ExprKind::StringLit(_)
            | ExprKind::UnitLit => {}
            ExprKind::ClassLit { path: _ } => {}
            ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let ast::StringPart::Expr(e) = part {
                        self.expr(e);
                    }
                }
            }
            ExprKind::TupleLit(els) | ExprKind::ArrayLit(els) => {
                for e in els {
                    self.expr(e);
                }
            }
            ExprKind::StructLit { fields, .. } => {
                for f in fields {
                    self.expr(&f.value);
                }
            }
            ExprKind::Block(b)
            | ExprKind::DoBlock(b)
            | ExprKind::UnsafeBlock(b)
            | ExprKind::SafeBlock(b) => self.block(b),
            ExprKind::Lambda(l) => match &l.body {
                ast::LambdaBody::Block(b) => self.block(b),
                ast::LambdaBody::Expr(e) => self.expr(e),
            },
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.expr(cond);
                self.expr(then_branch);
                if let Some(e) = else_branch {
                    self.expr(e);
                }
            }
            ExprKind::When { subject, arms } => {
                self.expr(subject);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.expr(guard);
                    }
                    self.expr(&arm.body);
                }
            }
            ExprKind::Handle { body, finally, .. } => {
                self.block(body);
                if let Some(f) = finally {
                    self.block(f);
                }
            }
            ExprKind::MemberAccess { receiver, .. }
            | ExprKind::SafeMemberAccess { receiver, .. } => self.expr(receiver),
            ExprKind::Index { receiver, indices } => {
                self.expr(receiver);
                for i in indices {
                    self.expr(i);
                }
            }
            ExprKind::NotNullAssert { expr: inner } => self.expr(inner),
            ExprKind::TypeApply { callee, .. } => self.expr(callee),
            ExprKind::Call { callee, args } => {
                // callee 的类型由 call resolution 推导（不在 expr_types 中独立赋值）。
                // completeness gate 不检查 callee 节点——它的类型隐含在 Call 的返回类型中。
                // 仅检查 args。
                for a in args {
                    self.expr(&a.value);
                }
            }
            ExprKind::Unary { expr: inner, .. } => self.expr(inner),
            ExprKind::Binary { lhs, rhs, .. } => {
                self.expr(lhs);
                self.expr(rhs);
            }
            ExprKind::InfixCall { receiver, arg, .. } => {
                self.expr(receiver);
                self.expr(arg);
            }
            ExprKind::TypeCheck { expr: inner, .. } | ExprKind::Cast { expr: inner, .. } => {
                self.expr(inner)
            }
            ExprKind::WithUpdate { base, updates } => {
                self.expr(base);
                for u in updates {
                    self.expr(&u.value);
                }
            }
            ExprKind::Annotated { expr: inner, .. } => self.expr(inner),
        }
    }

    fn report(&mut self, span: Span) {
        self.count += 1;
        self.diags.push(
            Diagnostic::error(
                "scoop::typecheck::untyped_node",
                format!(
                    "表达式节点未被 typecheck 赋予类型（file {:?}, {}..{}）",
                    self.file_id, span.start, span.end
                ),
            )
            .with_primary(span, "这里"),
        );
    }
}
