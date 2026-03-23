//! HIR → MIR 的最小 lowering（TODO T0708）。
//!
//! 说明：
//! - 该 lowering 目前仅用于 `scoop dump-mir` 与 `tests/fixtures/mir/**` 的回归；
//! - 实现优先保证“稳定输出 + 不 panic”；
//! - 未覆盖的表达式/语句会以 `Todo(...)` 占位，避免阻断后续迭代。

use std::collections::HashMap;

use miette::Diagnostic;
use thiserror::Error;

use crate::hir;
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, TypeId, TypeStore};

use super::{
    BasicBlock, BasicBlockId, Body, ConstValue, File, FunDecl, Item, LocalDecl, LocalId, Operand,
    Param, Rvalue, Statement, StatementKind, Terminator, TerminatorKind, UnwindAction,
};

/// MIR lowering 错误（当前阶段仅包装 HIR lowering 错误）。
#[derive(Debug, Error, Diagnostic)]
pub enum MirLowerError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Hir(#[from] hir::HirLowerError),
}

/// 一次 lowering 的产物：MIR + 对应的 `TypeStore`。
///
/// 说明：MIR 节点里的 `TypeId` 仅在同一个 `TypeStore` 里可解码/展示。
#[derive(Debug)]
pub struct LoweredMir {
    pub file: File,
    pub types: TypeStore,
}

/// 新建 basic block 时使用的默认 terminator 标记。
///
/// 说明：builder 在 block 完成后应当覆盖该 terminator；若最终仍保留该值，说明 lowering 未覆盖到
/// 某条控制流路径（对 dump/fixtures 来说仍可接受，但在后续阶段应当更严格约束）。
const UNTERMINATED: &str = "unterminated";

/// 为 `scoop dump-mir` / mir fixtures 生成 MIR（最小实现）。
///
/// 当前阶段 pipeline：
/// 1) parse/resolve 源文件并降到 HIR（复用 `hir::lower_for_dump`）；
/// 2) 把 HIR 再降到 MIR（本文件实现），并生成显式 CFG。
pub fn lower_for_dump(session: &Session, source: &SourceFile) -> Result<LoweredMir, MirLowerError> {
    let mut lowered_hir = hir::lower_for_dump(session, source)?;
    let builtins = lowered_hir.types.intern_builtins();

    let file = MirLowering::new(builtins).lower_file(&lowered_hir.file);
    Ok(LoweredMir {
        file,
        types: lowered_hir.types,
    })
}

/// 文件级 lowering：负责遍历顶层 item 并为每个函数构造 MIR body。
#[derive(Debug, Clone, Copy)]
struct MirLowering {
    builtins: BuiltinTypes,
}

impl MirLowering {
    /// 创建一个 MIR lowering 上下文（仅保存 builtin type ids）。
    fn new(builtins: BuiltinTypes) -> Self {
        Self { builtins }
    }

    /// 把 HIR 文件降到 MIR 文件。
    fn lower_file(self, file: &hir::File) -> File {
        let mut items = Vec::with_capacity(file.items.len());
        for item in &file.items {
            match item {
                hir::Item::Fun(fun) => items.push(Item::Fun(self.lower_fun(fun))),
                hir::Item::Val(decl) => items.push(Item::Todo {
                    span: decl.span,
                    kind: "top-level val",
                }),
                hir::Item::Todo { span, kind } => items.push(Item::Todo { span: *span, kind }),
            }
        }
        File { items }
    }

    /// 把一个函数降到 MIR。
    fn lower_fun(self, fun: &hir::FunDecl) -> FunDecl {
        FnLowering::new(self.builtins).lower_fun(fun)
    }
}

/// 函数体 lowering：负责为单个函数构造 `Body`、管理 locals、并生成显式 CFG。
#[derive(Debug)]
struct FnLowering {
    builtins: BuiltinTypes,
    body: Body,
    current_bb: BasicBlockId,
    next_temp: u32,
    symbol_locals: HashMap<hir::SymbolId, LocalId>,
}

impl FnLowering {
    /// 创建一个新的函数 lowering builder。
    fn new(builtins: BuiltinTypes) -> Self {
        Self {
            builtins,
            body: Body::new_empty(),
            current_bb: BasicBlockId(0),
            next_temp: 0,
            symbol_locals: HashMap::new(),
        }
    }

    /// 把一个 HIR 函数声明降到 MIR（当前阶段仅关注 body 的 CFG 形态）。
    fn lower_fun(mut self, fun: &hir::FunDecl) -> FunDecl {
        // 1) 创建入口块。
        let entry = self.push_block(fun.span);
        self.body.start = entry;
        self.current_bb = entry;

        // 2) 参数变为 locals，并建立 SymbolId → LocalId 映射。
        let mut params = Vec::with_capacity(fun.params.len());
        for p in &fun.params {
            let local = self.push_named_local(p.span, &p.name, p.ty);
            self.symbol_locals.insert(p.id, local);
            params.push(Param {
                span: p.span,
                name: p.name.clone(),
                ty: p.ty,
                local,
            });
        }

        // 3) lower 函数体。
        let mir_body = fun.body.as_ref().map(|block| {
            self.lower_block_as_stmt(block);
            self.finish_function(fun.span);
            self.body
        });

        FunDecl {
            span: fun.span,
            fqn: fun.fqn.clone(),
            name: fun.name.clone(),
            ty: fun.ty,
            params,
            return_ty: fun.return_ty,
            body: mir_body,
        }
    }

    /// 创建一个新的 basic block，并返回其 id。
    fn push_block(&mut self, span: Span) -> BasicBlockId {
        self.body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span,
                kind: TerminatorKind::Todo(UNTERMINATED),
                unwind: UnwindAction::NoUnwind,
            },
        })
    }

    /// 在当前 basic block 末尾追加一条语句。
    fn push_stmt(&mut self, span: Span, kind: StatementKind) {
        let bb = self.current_bb;
        self.body.blocks[bb.as_usize()].stmts.push(Statement { span, kind });
    }

    /// 覆盖指定 basic block 的 terminator。
    fn set_terminator(&mut self, bb: BasicBlockId, span: Span, kind: TerminatorKind) {
        self.body.blocks[bb.as_usize()].terminator = Terminator {
            span,
            kind,
            unwind: UnwindAction::NoUnwind,
        };
    }

    /// 当前 basic block 是否已经被 terminator 结束。
    fn current_is_terminated(&self) -> bool {
        let bb = self.current_bb;
        !matches!(
            self.body.blocks[bb.as_usize()].terminator.kind,
            TerminatorKind::Todo(msg) if msg == UNTERMINATED
        )
    }

    /// 若函数尾部没有显式 terminator，则默认补一个 `return`（保持 body 可验证/可 dump）。
    fn finish_function(&mut self, span: Span) {
        if !self.current_is_terminated() {
            self.set_terminator(self.current_bb, span, TerminatorKind::Return);
        }
    }

    /// 分配一个具名 local（用于参数与 `val/var` 声明）。
    fn push_named_local(&mut self, span: Span, name: &str, ty: TypeId) -> LocalId {
        self.body.push_local(LocalDecl {
            span,
            name: Some(name.to_string()),
            ty,
        })
    }

    /// 分配一个临时 local（用于表达式求值与 if/when merge）。
    fn push_temp_local(&mut self, span: Span, ty: TypeId) -> LocalId {
        let name = format!("tmp{}", self.next_temp);
        self.next_temp += 1;
        self.push_named_local(span, &name, ty)
    }

    /// 生成 `target = value` 赋值语句。
    fn assign(&mut self, span: Span, target: LocalId, value: Rvalue) {
        self.push_stmt(span, StatementKind::Assign { target, value });
    }

    /// 把一个 block 作为“语句块”来 lower（顺序执行；最后表达式结果被丢弃）。
    fn lower_block_as_stmt(&mut self, block: &hir::Block) {
        for stmt in &block.stmts {
            if self.current_is_terminated() {
                break;
            }
            self.lower_stmt(stmt);
        }
    }

    /// 把一个 block 作为“表达式块”来 lower，并返回 block 的结果 local。
    fn lower_block_as_expr(&mut self, block: &hir::Block) -> LocalId {
        let mut result: Option<LocalId> = None;
        for (idx, stmt) in block.stmts.iter().enumerate() {
            if self.current_is_terminated() {
                break;
            }
            let is_last = idx + 1 == block.stmts.len();
            match (&stmt.kind, is_last) {
                (hir::StmtKind::Expr(expr), true) => result = Some(self.lower_expr_to_local(expr)),
                _ => self.lower_stmt(stmt),
            }
        }

        result.unwrap_or_else(|| self.emit_unit(block.span))
    }

    /// 把一条 HIR 语句降到 MIR（当前阶段只覆盖必要子集；未覆盖节点以 `Todo` 占位）。
    fn lower_stmt(&mut self, stmt: &hir::Stmt) {
        match &stmt.kind {
            hir::StmtKind::Empty => {}
            hir::StmtKind::Expr(expr) => {
                let _ = self.lower_expr_to_local(expr);
            }
            hir::StmtKind::Val(decl) => self.lower_val_decl(decl),
            hir::StmtKind::Assign { lhs, rhs, .. } => self.lower_assign_stmt(stmt.span, lhs, rhs),
            hir::StmtKind::While { .. } => {
                self.push_stmt(stmt.span, StatementKind::Todo("while lowering pending (T0709)"));
            }
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value {
                    let _ = self.lower_expr_to_local(expr);
                }
                self.set_terminator(self.current_bb, stmt.span, TerminatorKind::Return);
            }
            hir::StmtKind::Todo(kind) => self.push_stmt(stmt.span, StatementKind::Todo(kind)),
        }
    }

    /// 降低一个 `val/var` 声明：分配 local，并 lower initializer（若存在）。
    fn lower_val_decl(&mut self, decl: &hir::ValDecl) {
        let Some(id) = decl.id else {
            self.push_stmt(decl.span, StatementKind::Todo("val decl missing symbol id"));
            return;
        };

        let name = decl.name.as_deref().unwrap_or("<anon>");
        let local = self.push_named_local(decl.span, name, decl.ty);
        self.symbol_locals.insert(id, local);

        if let Some(init) = &decl.init {
            let value = self.lower_expr_to_local(init);
            self.assign(decl.span, local, Rvalue::Use(Operand::Local(value)));
        }
    }

    /// 降低一个赋值语句（当前仅覆盖 `local = expr`）。
    fn lower_assign_stmt(&mut self, span: Span, lhs: &hir::Expr, rhs: &hir::Expr) {
        let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &lhs.kind else {
            self.push_stmt(span, StatementKind::Todo("assign lhs lowering pending"));
            return;
        };
        let Some(target) = self.symbol_locals.get(id).copied() else {
            self.push_stmt(span, StatementKind::Todo("assign lhs missing local"));
            return;
        };

        let value = self.lower_expr_to_local(rhs);
        self.assign(span, target, Rvalue::Use(Operand::Local(value)));
    }

    /// 把一个 HIR 表达式降为“产生值的 local”，并返回该 local。
    ///
    /// 说明：当前阶段优先保证 CFG 形态正确，因此表达式求值本身常以 `Todo` 占位。
    fn lower_expr_to_local(&mut self, expr: &hir::Expr) -> LocalId {
        match &expr.kind {
            hir::ExprKind::Missing => {
                let tmp = self.push_temp_local(expr.span, expr.ty);
                self.assign(expr.span, tmp, Rvalue::Todo("missing expr"));
                tmp
            }
            hir::ExprKind::Todo(kind) => {
                let tmp = self.push_temp_local(expr.span, expr.ty);
                self.assign(expr.span, tmp, Rvalue::Todo(kind));
                tmp
            }
            hir::ExprKind::Literal(lit) => self.lower_literal(expr.span, expr.ty, lit),
            hir::ExprKind::VarRef(v) => self.lower_var_ref(expr.span, expr.ty, v),
            hir::ExprKind::Block(block) => self.lower_block_as_expr(block),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.lower_if_expr(expr.span, expr.ty, cond, then_branch, else_branch.as_deref()),
            hir::ExprKind::When { subject, arms } => self.lower_when_expr(expr.span, expr.ty, subject, arms),
            hir::ExprKind::MemberAccess { .. } => self.emit_todo_value(expr.span, expr.ty, "member access lowering pending"),
            hir::ExprKind::Call { .. } => self.emit_todo_value(expr.span, expr.ty, "call lowering pending"),
            hir::ExprKind::Perform { .. } => self.emit_todo_value(expr.span, expr.ty, "perform lowering pending"),
            hir::ExprKind::Handle(_) => self.emit_todo_value(expr.span, expr.ty, "handle lowering pending"),
        }
    }

    /// 生成一个 `Unit` 值，并返回其 local。
    fn emit_unit(&mut self, span: Span) -> LocalId {
        let tmp = self.push_temp_local(span, self.builtins.unit);
        self.assign(span, tmp, Rvalue::Use(Operand::Const(ConstValue::Unit)));
        tmp
    }

    /// 生成一个“未实现的值”，并返回其 local。
    fn emit_todo_value(&mut self, span: Span, ty: TypeId, msg: &'static str) -> LocalId {
        let tmp = self.push_temp_local(span, ty);
        self.assign(span, tmp, Rvalue::Todo(msg));
        tmp
    }

    /// 降低字面量：把常量写入一个临时 local。
    fn lower_literal(&mut self, span: Span, ty: TypeId, lit: &hir::LiteralKind) -> LocalId {
        let tmp = self.push_temp_local(span, ty);
        let c = match lit {
            hir::LiteralKind::Bool(b) => ConstValue::Bool(*b),
            hir::LiteralKind::Unit => ConstValue::Unit,
            hir::LiteralKind::Int => ConstValue::Int,
            hir::LiteralKind::String => ConstValue::String,
        };
        self.assign(span, tmp, Rvalue::Use(Operand::Const(c)));
        tmp
    }

    /// 降低变量引用：local 引用直接返回对应的 local；其它引用降为 `Todo`。
    fn lower_var_ref(&mut self, span: Span, ty: TypeId, v: &hir::ValueRef) -> LocalId {
        match v {
            hir::ValueRef::Local { id, .. } => self
                .symbol_locals
                .get(id)
                .copied()
                .unwrap_or_else(|| self.emit_todo_value(span, ty, "unbound local ref")),
            hir::ValueRef::TopLevel { .. } => self.emit_todo_value(span, ty, "top-level ref lowering pending"),
        }
    }

    /// 降低 `if` 表达式：生成 then/else/merge 基本块，并在 merge 点写回一个临时结果 local。
    fn lower_if_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        cond: &hir::Expr,
        then_branch: &hir::Expr,
        else_branch: Option<&hir::Expr>,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);

        // 1) 先在当前块求值条件，并以 CondBr 结束当前块。
        let cond_local = self.lower_expr_to_local(cond);
        let parent = self.current_bb;
        let then_bb = self.push_block(then_branch.span);
        let else_bb = self.push_block(else_branch.map(|e| e.span).unwrap_or(span));
        let merge_bb = self.push_block(span);

        self.set_terminator(
            parent,
            span,
            TerminatorKind::CondBr {
                cond: Operand::Local(cond_local),
                then_target: then_bb,
                else_target: else_bb,
            },
        );

        // 2) then 分支：lower 表达式并写回 result，然后跳到 merge。
        self.current_bb = then_bb;
        let then_value = self.lower_expr_to_local(then_branch);
        if !self.current_is_terminated() {
            self.assign(then_branch.span, result, Rvalue::Use(Operand::Local(then_value)));
            self.set_terminator(self.current_bb, then_branch.span, TerminatorKind::Goto { target: merge_bb });
        }

        // 3) else 分支：同上；若缺省 else，则使用 Unit 占位。
        self.current_bb = else_bb;
        let else_value = else_branch
            .map(|e| self.lower_expr_to_local(e))
            .unwrap_or_else(|| self.emit_unit(span));
        if !self.current_is_terminated() {
            self.assign(span, result, Rvalue::Use(Operand::Local(else_value)));
            self.set_terminator(self.current_bb, span, TerminatorKind::Goto { target: merge_bb });
        }

        // 4) merge：后续语句继续在 merge 块中生成。
        self.current_bb = merge_bb;
        result
    }

    /// 降低 `when` 表达式：把每个 arm 降为一段 CFG（当前以“链式 CondBr”表达）。
    fn lower_when_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        subject: &hir::Expr,
        arms: &[hir::WhenArm],
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);

        // 1) 先在当前块求值 subject。
        let subject_local = self.lower_expr_to_local(subject);

        // 2) 构造 merge block，并从当前块开始链式生成“匹配测试块”。
        let merge_bb = self.push_block(span);
        let mut test_bb = self.current_bb;

        for arm in arms {
            let body_bb = self.push_block(arm.span);

            // else / wildcard 作为默认分支：直接跳转到 body，并结束 when 链。
            if matches!(
                &arm.pat,
                hir::WhenPat::Else { .. } | hir::WhenPat::Wildcard { .. }
            ) {
                self.set_terminator(test_bb, arm.span, TerminatorKind::Goto { target: body_bb });
                self.current_bb = body_bb;
                let body_value = self.lower_expr_to_local(&arm.body);
                if !self.current_is_terminated() {
                    self.assign(arm.span, result, Rvalue::Use(Operand::Local(body_value)));
                    self.set_terminator(self.current_bb, arm.span, TerminatorKind::Goto { target: merge_bb });
                }
                self.current_bb = merge_bb;
                return result;
            }

            // 非默认分支：生成一个条件 local，并以 CondBr 结束当前测试块。
            let next_test_bb = self.push_block(arm.span);
            self.current_bb = test_bb;

            // 当前阶段不实现真实 pattern/guard 匹配：用 `Todo` 占位一个 bool 值，
            // 但保留 subject_local，便于后续替换为真正的判定逻辑。
            let cond = self.push_temp_local(arm.span, self.builtins.bool_);
            let _ = subject_local;
            let cond_msg = if arm.guard.is_some() {
                "when arm condition (pat+guard) pending"
            } else {
                "when arm condition (pat) pending"
            };
            self.assign(arm.span, cond, Rvalue::Todo(cond_msg));

            self.set_terminator(
                test_bb,
                arm.span,
                TerminatorKind::CondBr {
                    cond: Operand::Local(cond),
                    then_target: body_bb,
                    else_target: next_test_bb,
                },
            );

            // body 分支：lower 表达式并写回 result，然后跳到 merge。
            self.current_bb = body_bb;
            let body_value = self.lower_expr_to_local(&arm.body);
            if !self.current_is_terminated() {
                self.assign(arm.span, result, Rvalue::Use(Operand::Local(body_value)));
                self.set_terminator(self.current_bb, arm.span, TerminatorKind::Goto { target: merge_bb });
            }

            // 继续下一个 arm 的测试块。
            test_bb = next_test_bb;
            self.current_bb = test_bb;
        }

        // 若没有 else/wildcard arm，当前阶段以 `unreachable` 收束。
        self.set_terminator(test_bb, span, TerminatorKind::Unreachable);
        self.current_bb = merge_bb;
        result
    }
}
