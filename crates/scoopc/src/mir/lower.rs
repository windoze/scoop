//! HIR → MIR 的最小 lowering（TODO T0708）。
//!
//! 说明：
//! - 该 lowering 目前仅用于 `scoop dump-mir` 与 `tests/fixtures/mir/**` 的回归；
//! - 实现优先保证“稳定输出 + 不 panic”；
//! - 未覆盖的表达式/语句会以 `Todo(...)` 占位，避免阻断后续迭代。

use std::collections::{HashMap, HashSet};

use miette::Diagnostic;
use thiserror::Error;

use crate::hir;
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore};

use super::{
    BasicBlock, BasicBlockId, Body, ConstValue, File, FunDecl, HandlerArm, Item, LocalDecl,
    LocalId, Operand, Param, Rvalue, Statement, StatementKind, Terminator, TerminatorKind,
    UnwindAction,
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
/// `var` 可变捕获在 MIR dump 阶段使用的内部 box 类型名（T0714）。
const CAPTURE_BOX_FQN: &str = "scoop.__CaptureBox";

/// 为 `scoop dump-mir` / mir fixtures 生成 MIR（最小实现）。
///
/// 当前阶段 pipeline：
/// 1) parse/resolve 源文件并降到 HIR（复用 `hir::lower_for_dump`）；
/// 2) 把 HIR 再降到 MIR（本文件实现），并生成显式 CFG。
pub fn lower_for_dump(session: &Session, source: &SourceFile) -> Result<LoweredMir, MirLowerError> {
    let mut lowered_hir = hir::lower_for_dump(session, source)?;
    let builtins = lowered_hir.types.intern_builtins();

    let file = {
        let mut lowering = MirLowering::new(builtins, &mut lowered_hir.types);
        lowering.lower_file(&lowered_hir.file)
    };
    Ok(LoweredMir {
        file,
        types: lowered_hir.types,
    })
}

/// 将一份已构造的 HIR 文件降低为 MIR（供单态化实例生成复用，T0712）。
///
/// 说明：
/// - 与 `lower_for_dump` 不同，该函数不负责 parse/resolve/HIR lowering；
/// - 调用方需要确保 `hir_file` 中的 `TypeId` 与 `types` 来自同一个 `TypeStore`。
pub(crate) fn lower_hir_file_for_dump(
    builtins: BuiltinTypes,
    types: &mut TypeStore,
    hir_file: &hir::File,
) -> File {
    let mut lowering = MirLowering::new(builtins, types);
    lowering.lower_file(hir_file)
}

/// 文件级 lowering：负责遍历顶层 item 并为每个函数构造 MIR body。
struct MirLowering<'a> {
    builtins: BuiltinTypes,
    types: &'a mut TypeStore,
}

impl<'a> MirLowering<'a> {
    /// 创建一个 MIR lowering 上下文（仅保存 builtin type ids）。
    fn new(builtins: BuiltinTypes, types: &'a mut TypeStore) -> Self {
        Self { builtins, types }
    }

    /// 把 HIR 文件降到 MIR 文件。
    fn lower_file(&mut self, file: &hir::File) -> File {
        let mut items = Vec::with_capacity(file.items.len());
        for item in &file.items {
            match item {
                hir::Item::Fun(fun) => {
                    let (primary, nested) = self.lower_fun(fun);
                    items.push(Item::Fun(primary));
                    items.extend(nested.into_iter().map(Item::Fun));
                }
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
    fn lower_fun(&mut self, fun: &hir::FunDecl) -> (FunDecl, Vec<FunDecl>) {
        FnLowering::new(self.builtins, self.types, fun.fqn.clone()).lower_fun(fun)
    }
}

/// 函数体 lowering：负责为单个函数构造 `Body`、管理 locals、并生成显式 CFG。
#[derive(Debug)]
struct FnLowering<'a> {
    builtins: BuiltinTypes,
    types: &'a mut TypeStore,
    owner_fqn: String,
    body: Body,
    current_bb: BasicBlockId,
    next_temp: u32,
    symbol_locals: HashMap<hir::SymbolId, LocalId>,
    /// 当前函数内哪些 `SymbolId` 以 box 形式存储（用于 `var` 被 closure 捕获时的别名语义，T0714）。
    boxed_symbols: HashSet<hir::SymbolId>,
    loop_stack: Vec<LoopContext>,
    nested_funs: Vec<FunDecl>,
}

/// 当前函数内的一个 loop 语境（用于 `break/continue` lowering）。
#[derive(Debug, Clone, Copy)]
struct LoopContext {
    break_target: BasicBlockId,
    continue_target: BasicBlockId,
}

/// 一个 closure 捕获的外部局部变量在 env tuple 中的布局信息（T0711）。
#[derive(Debug, Clone)]
struct ClosureCaptureLayout {
    id: hir::SymbolId,
    name: String,
    decl_span: Span,
    ty: TypeId,
    mutable: bool,
    /// 在“创建该 closure 的函数”中，对应被捕获值的 local。
    source_local: LocalId,
}

impl<'a> FnLowering<'a> {
    /// 创建一个新的函数 lowering builder。
    fn new(builtins: BuiltinTypes, types: &'a mut TypeStore, owner_fqn: String) -> Self {
        Self {
            builtins,
            types,
            owner_fqn,
            body: Body::new_empty(),
            current_bb: BasicBlockId(0),
            next_temp: 0,
            symbol_locals: HashMap::new(),
            boxed_symbols: HashSet::new(),
            loop_stack: Vec::new(),
            nested_funs: Vec::new(),
        }
    }

    /// 把一个 HIR 函数声明降到 MIR（当前阶段仅关注 body 的 CFG 形态）。
    fn lower_fun(mut self, fun: &hir::FunDecl) -> (FunDecl, Vec<FunDecl>) {
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
        let mir_body = if let Some(block) = fun.body.as_ref() {
            // 先扫描函数体：若某个 `var` 被任意深度的嵌套 closure 捕获，则该 `var` 在本函数内需要 box 存储。
            self.boxed_symbols = boxed_symbols_in_block(block);
            self.lower_block_as_stmt(block);
            self.finish_function(fun.span);
            Some(std::mem::replace(&mut self.body, Body::new_empty()))
        } else {
            None
        };

        let out = FunDecl {
            span: fun.span,
            fqn: fun.fqn.clone(),
            name: fun.name.clone(),
            ty: fun.ty,
            params,
            return_ty: fun.return_ty,
            body: mir_body,
        };

        (out, self.nested_funs)
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
        self.body.blocks[bb.as_usize()]
            .stmts
            .push(Statement { span, kind });
    }

    /// 覆盖指定 basic block 的 terminator。
    fn set_terminator_with_unwind(
        &mut self,
        bb: BasicBlockId,
        span: Span,
        kind: TerminatorKind,
        unwind: UnwindAction,
    ) {
        self.body.blocks[bb.as_usize()].terminator = Terminator { span, kind, unwind };
    }

    /// 覆盖指定 basic block 的 terminator（默认 `NoUnwind`）。
    fn set_terminator(&mut self, bb: BasicBlockId, span: Span, kind: TerminatorKind) {
        self.set_terminator_with_unwind(bb, span, kind, UnwindAction::NoUnwind);
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

        if self.current_is_terminated() {
            // block 由于 `return/break/continue` 等提前终止：结果永远不会被使用。
            // 为保持接口一致，仍返回一个临时 local，但不额外发射赋值语句（避免“终止后又生成语句”）。
            return self.push_temp_local(block.span, block.ty);
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
            hir::StmtKind::While { cond, body } => self.lower_while_stmt(stmt.span, cond, body),
            hir::StmtKind::Break { .. } => self.lower_break_stmt(stmt.span),
            hir::StmtKind::Continue { .. } => self.lower_continue_stmt(stmt.span),
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value {
                    let _ = self.lower_expr_to_local(expr);
                }
                self.set_terminator(self.current_bb, stmt.span, TerminatorKind::Return);
            }
            hir::StmtKind::Todo(kind) => self.push_stmt(stmt.span, StatementKind::Todo(kind)),
        }
    }

    /// 降低一个 `while` 语句：构造 loop CFG，并为 `break/continue` 建立跳转目标。
    fn lower_while_stmt(&mut self, span: Span, cond: &hir::Expr, body: &hir::Block) {
        // CFG 形态（无 label）：
        //
        //   parent ──goto──▶ cond_bb ──condbr──▶ body_bb ──goto──▶ cond_bb
        //                 └───────────────▶ exit_bb
        //
        // `break`    → exit_bb
        // `continue` → cond_bb

        let parent = self.current_bb;
        let cond_bb = self.push_block(cond.span);
        let body_bb = self.push_block(body.span);
        let exit_bb = self.push_block(span);

        self.set_terminator(parent, span, TerminatorKind::Goto { target: cond_bb });

        // 1) condition：在 cond_bb 中求值条件，并用 CondBr 结束。
        self.current_bb = cond_bb;
        let cond_local = self.lower_expr_to_local(cond);
        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::CondBr {
                    cond: Operand::Local(cond_local),
                    then_target: body_bb,
                    else_target: exit_bb,
                },
            );
        }

        // 2) body：在 loop context 下 lower body；若 body 自然结束则回跳 cond_bb。
        self.current_bb = body_bb;
        self.loop_stack.push(LoopContext {
            break_target: exit_bb,
            continue_target: cond_bb,
        });
        self.lower_block_as_stmt(body);
        let _ = self.loop_stack.pop();

        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                body.span,
                TerminatorKind::Goto { target: cond_bb },
            );
        }

        // 3) 后续语句继续在 exit_bb 生成。
        self.current_bb = exit_bb;
    }

    /// 降低 `break`：跳转到当前 loop 的 exit block。
    fn lower_break_stmt(&mut self, span: Span) {
        let Some(ctx) = self.loop_stack.last().copied() else {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Todo("break not in loop"),
            );
            return;
        };
        self.set_terminator(
            self.current_bb,
            span,
            TerminatorKind::Goto {
                target: ctx.break_target,
            },
        );
    }

    /// 降低 `continue`：跳转到当前 loop 的 cond block。
    fn lower_continue_stmt(&mut self, span: Span) {
        let Some(ctx) = self.loop_stack.last().copied() else {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Todo("continue not in loop"),
            );
            return;
        };
        self.set_terminator(
            self.current_bb,
            span,
            TerminatorKind::Goto {
                target: ctx.continue_target,
            },
        );
    }

    /// 降低一个 `val/var` 声明：分配 local，并 lower initializer（若存在）。
    fn lower_val_decl(&mut self, decl: &hir::ValDecl) {
        let Some(id) = decl.id else {
            self.push_stmt(decl.span, StatementKind::Todo("val decl missing symbol id"));
            return;
        };

        let name = decl.name.as_deref().unwrap_or("<anon>");
        // `var` 若被 closure 捕获，需要在本函数内以 box 形式存储，保证后续读写别名一致（T0714）。
        if decl.mutable && self.boxed_symbols.contains(&id) {
            let box_ty = self.capture_box_ty(decl.ty);
            let local = self.push_named_local(decl.span, name, box_ty);
            self.symbol_locals.insert(id, local);

            if let Some(init) = &decl.init {
                let value = self.lower_expr_to_local(init);
                self.assign(
                    decl.span,
                    local,
                    Rvalue::CaptureBoxNew {
                        value: Operand::Local(value),
                    },
                );
            } else {
                self.assign(
                    decl.span,
                    local,
                    Rvalue::Todo("boxed var decl init pending"),
                );
            }
            return;
        }

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
        if self.boxed_symbols.contains(id) {
            let tmp = self.push_temp_local(span, self.builtins.unit);
            self.assign(
                span,
                tmp,
                Rvalue::CaptureBoxSet {
                    box_operand: Operand::Local(target),
                    value: Operand::Local(value),
                },
            );
        } else {
            self.assign(span, target, Rvalue::Use(Operand::Local(value)));
        }
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
            hir::ExprKind::UnresolvedIdent { .. } => {
                self.emit_todo_value(expr.span, expr.ty, "unresolved ident")
            }
            hir::ExprKind::Todo(kind) => {
                let tmp = self.push_temp_local(expr.span, expr.ty);
                self.assign(expr.span, tmp, Rvalue::Todo(kind));
                tmp
            }
            hir::ExprKind::Literal(lit) => self.lower_literal(expr.span, expr.ty, lit),
            hir::ExprKind::VarRef(v) => self.lower_var_ref(expr.span, expr.ty, v),
            hir::ExprKind::StructLit { .. } => {
                self.emit_todo_value(expr.span, expr.ty, "struct literal lowering pending")
            }
            hir::ExprKind::TupleLit { .. } => {
                self.emit_todo_value(expr.span, expr.ty, "tuple literal lowering pending")
            }
            hir::ExprKind::InterpolatedString { .. } => {
                self.emit_todo_value(expr.span, expr.ty, "interpolated string lowering pending")
            }
            hir::ExprKind::Unary { .. } => {
                // 当前阶段 MIR 仍以 CFG 形态回归为主；一元表达式求值留给后续 codegen 任务补齐。
                let tmp = self.push_temp_local(expr.span, expr.ty);
                self.assign(expr.span, tmp, Rvalue::Todo("unary"));
                tmp
            }
            hir::ExprKind::Binary { .. } => {
                // 当前阶段 MIR 仍以 CFG 形态回归为主；二元表达式求值留给后续 codegen 任务补齐。
                let tmp = self.push_temp_local(expr.span, expr.ty);
                self.assign(expr.span, tmp, Rvalue::Todo("binary"));
                tmp
            }
            hir::ExprKind::Block(block) => self.lower_block_as_expr(block),
            hir::ExprKind::Closure(closure) => self.lower_closure_expr(expr.span, expr.ty, closure),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.lower_if_expr(
                expr.span,
                expr.ty,
                cond,
                then_branch,
                else_branch.as_deref(),
            ),
            hir::ExprKind::When { subject, arms } => {
                self.lower_when_expr(expr.span, expr.ty, subject, arms)
            }
            hir::ExprKind::MemberAccess { .. } => {
                self.emit_todo_value(expr.span, expr.ty, "member access lowering pending")
            }
            hir::ExprKind::Call { .. } => {
                self.emit_todo_value(expr.span, expr.ty, "call lowering pending")
            }
            hir::ExprKind::Perform { op, args } => {
                self.lower_perform_expr(expr.span, expr.ty, op, args)
            }
            hir::ExprKind::Handle(handle) => self.lower_handle_expr(expr.span, expr.ty, handle),
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

    fn capture_box_ty(&mut self, inner: TypeId) -> TypeId {
        self.types
            .intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
                fqn: CAPTURE_BOX_FQN.to_string(),
                args: vec![inner],
                eff: None,
            })))
    }

    /// 降低一个 effect operation 调用（HIR `Perform`）到 MIR。
    ///
    /// 当前阶段（TODO T0713）先做“结构落地 + 不 panic”：
    /// - 先按顺序 lowering 实参表达式（即便暂不把参数传入 MIR terminator）；
    /// - 为该表达式分配一个临时结果 local（值本身用 `Todo` 占位）；
    /// - 用 `TerminatorKind::Perform` 结束当前基本块，并标记该点“可能发生 unwinding”。
    fn lower_perform_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
    ) -> LocalId {
        for arg in args {
            if self.current_is_terminated() {
                break;
            }
            match arg {
                hir::CallArg::Positional(expr) => {
                    let _ = self.lower_expr_to_local(expr);
                }
                hir::CallArg::Named { value, .. } => {
                    let _ = self.lower_expr_to_local(value);
                }
            }
        }

        if self.current_is_terminated() {
            // 实参 lowering 提前终止了 CFG：该 perform 永远不会发生。
            return self.push_temp_local(span, ty);
        }

        let result = self.push_temp_local(span, ty);
        self.assign(span, result, Rvalue::Todo("perform result pending"));

        self.set_terminator_with_unwind(
            self.current_bb,
            span,
            TerminatorKind::Perform {
                op_fqn: op.fqn.clone(),
            },
            UnwindAction::Todo("perform unwind pending"),
        );

        result
    }

    /// 降低一个 effect handler 表达式（HIR `Handle`）到 MIR。
    ///
    /// 当前阶段（TODO T0713）策略：
    /// - 把 handler boundary 以 `TerminatorKind::Handle { .. }` 占位落在当前块末尾；
    /// - 同时把 handle 的 body/arms/finally 降到**独立的新 block**里，便于 `dump-mir`/fixtures
    ///   观察内部的 `perform`/控制流形态；
    /// - 这些 block 暂未与主 CFG 连接（后续会在更完整的 effect lowering 中展开为显式 CFG）。
    fn lower_handle_expr(&mut self, span: Span, ty: TypeId, handle: &hir::HandleExpr) -> LocalId {
        let outer_bb = self.current_bb;

        let result = self.push_temp_local(span, ty);
        self.assign(span, result, Rvalue::Todo("handle result pending"));

        let arms = handle
            .arms
            .iter()
            .map(|arm| HandlerArm {
                op_fqn: arm.op.op.fqn.clone(),
                binder_count: arm.op.binders.len(),
            })
            .collect();

        self.set_terminator(
            outer_bb,
            span,
            TerminatorKind::Handle {
                arms,
                has_finally: handle.finally.is_some(),
            },
        );

        // 额外 lower handle 内部结构到独立 block（不连接到主 CFG）。
        let body_bb = self.push_block(handle.body.span);
        self.current_bb = body_bb;
        self.lower_block_as_stmt(&handle.body);
        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                handle.body.span,
                TerminatorKind::Todo("handle body exit pending"),
            );
        }

        for arm in &handle.arms {
            let arm_bb = self.push_block(arm.span);
            self.current_bb = arm_bb;
            let _ = self.lower_expr_to_local(&arm.body);
            if !self.current_is_terminated() {
                self.set_terminator(
                    self.current_bb,
                    arm.span,
                    TerminatorKind::Todo("handle arm exit pending"),
                );
            }
        }

        if let Some(finally) = &handle.finally {
            let finally_bb = self.push_block(finally.span);
            self.current_bb = finally_bb;
            self.lower_block_as_stmt(finally);
            if !self.current_is_terminated() {
                self.set_terminator(
                    self.current_bb,
                    finally.span,
                    TerminatorKind::Todo("handle finally exit pending"),
                );
            }
        }

        // 注意：必须把 current_bb 恢复回 outer_bb，保证外层 CFG 继续认为“当前块已终止”。
        self.current_bb = outer_bb;

        result
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

    /// 降低变量引用：
    /// - 普通 local：直接返回其 local；
    /// - 被 capture 的 `var`（box 存储）：生成 `CaptureBoxGet` 并返回读取到的临时值 local；
    /// - 其它引用：降为 `Todo`。
    fn lower_var_ref(&mut self, span: Span, ty: TypeId, v: &hir::ValueRef) -> LocalId {
        match v {
            hir::ValueRef::Local { id, .. } => {
                let Some(local) = self.symbol_locals.get(id).copied() else {
                    return self.emit_todo_value(span, ty, "unbound local ref");
                };

                if self.boxed_symbols.contains(id) {
                    let tmp = self.push_temp_local(span, ty);
                    self.assign(
                        span,
                        tmp,
                        Rvalue::CaptureBoxGet {
                            box_operand: Operand::Local(local),
                        },
                    );
                    tmp
                } else {
                    local
                }
            }
            hir::ValueRef::TopLevel { .. } => {
                self.emit_todo_value(span, ty, "top-level ref lowering pending")
            }
        }
    }

    fn lower_closure_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        closure: &hir::ClosureExpr,
    ) -> LocalId {
        let name = format!("$lambda{}", closure.id.as_u32());
        let fqn = format!("{}.{}", self.owner_fqn, name);

        // 1) 计算 capture set，并决定 env 的 tuple 类型。
        let mut captures: Vec<ClosureCaptureLayout> = Vec::new();
        for cap in &closure.captures {
            let Some(source_local) = self.symbol_locals.get(&cap.id).copied() else {
                // 防御性：若当前函数未为该 symbol 分配 local（理论上不应发生），跳过该 capture。
                continue;
            };
            let source_ty = self.body.locals[source_local.as_u32() as usize].ty;
            captures.push(ClosureCaptureLayout {
                id: cap.id,
                name: cap.name.clone(),
                decl_span: cap.decl_span,
                ty: source_ty,
                mutable: cap.mutable,
                source_local,
            });
        }

        let (env_ty, env_operand) = if captures.is_empty() {
            (self.builtins.unit, Operand::Const(ConstValue::Unit))
        } else {
            let env_ty = self.types.ty_tuple(captures.iter().map(|c| c.ty).collect());
            let env_local = self.push_temp_local(span, env_ty);
            self.assign(
                span,
                env_local,
                Rvalue::MakeTuple {
                    elements: captures
                        .iter()
                        .map(|c| Operand::Local(c.source_local))
                        .collect(),
                },
            );
            (env_ty, Operand::Local(env_local))
        };

        let (fun, nested) = {
            let types = &mut *self.types;
            FnLowering::new(self.builtins, types, fqn.clone()).lower_closure_fun(
                fqn.clone(),
                name,
                closure,
                env_ty,
                &captures,
            )
        };
        self.nested_funs.push(fun);
        self.nested_funs.extend(nested);

        let tmp = self.push_temp_local(span, ty);
        self.assign(
            span,
            tmp,
            Rvalue::MakeClosure {
                env: env_operand,
                fn_ptr: fqn,
            },
        );
        tmp
    }

    fn lower_closure_fun(
        mut self,
        closure_fqn: String,
        closure_name: String,
        closure: &hir::ClosureExpr,
        env_ty: TypeId,
        captures: &[ClosureCaptureLayout],
    ) -> (FunDecl, Vec<FunDecl>) {
        // 0) 预扫描 closure body：本 closure 内部若存在嵌套 closure 捕获 `var`，则需要 box 存储（T0714）。
        self.boxed_symbols = boxed_symbols_in_expr(closure.body.as_ref());

        // 1) 创建入口块。
        let entry = self.push_block(closure.span);
        self.body.start = entry;
        self.current_bb = entry;

        // 2) env + captures + 参数变为 locals。
        let mut params = Vec::with_capacity(closure.params.len() + 1);

        let env_local = self.push_named_local(closure.span, "$env", env_ty);
        params.push(Param {
            span: closure.span,
            name: "$env".to_string(),
            ty: env_ty,
            local: env_local,
        });

        // 把捕获字段从 `$env` 解包到局部 local，并写入 SymbolId → LocalId 映射，使得后续 body lowering
        // 可以像普通局部变量一样引用它们。
        for (idx, cap) in captures.iter().enumerate() {
            let local = self.push_named_local(cap.decl_span, &cap.name, cap.ty);
            self.symbol_locals.insert(cap.id, local);
            if cap.mutable {
                self.boxed_symbols.insert(cap.id);
            }
            self.assign(
                cap.decl_span,
                local,
                Rvalue::TupleGet {
                    tuple: Operand::Local(env_local),
                    index: idx,
                },
            );
        }

        for p in &closure.params {
            let local = self.push_named_local(p.span, &p.name, p.ty);
            self.symbol_locals.insert(p.id, local);
            params.push(Param {
                span: p.span,
                name: p.name.clone(),
                ty: p.ty,
                local,
            });
        }

        // 3) lower lambda body（当前阶段只关注 CFG 形态）。
        let _ = self.lower_expr_to_local(closure.body.as_ref());
        self.finish_function(closure.span);

        let out = FunDecl {
            span: closure.span,
            fqn: closure_fqn,
            name: closure_name,
            ty: self.builtins.any,
            params,
            return_ty: closure.body.ty,
            body: Some(self.body),
        };

        (out, self.nested_funs)
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
            self.assign(
                then_branch.span,
                result,
                Rvalue::Use(Operand::Local(then_value)),
            );
            self.set_terminator(
                self.current_bb,
                then_branch.span,
                TerminatorKind::Goto { target: merge_bb },
            );
        }

        // 3) else 分支：同上；若缺省 else，则使用 Unit 占位。
        self.current_bb = else_bb;
        let else_value = else_branch
            .map(|e| self.lower_expr_to_local(e))
            .unwrap_or_else(|| self.emit_unit(span));
        if !self.current_is_terminated() {
            self.assign(span, result, Rvalue::Use(Operand::Local(else_value)));
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Goto { target: merge_bb },
            );
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
                    self.set_terminator(
                        self.current_bb,
                        arm.span,
                        TerminatorKind::Goto { target: merge_bb },
                    );
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
                self.set_terminator(
                    self.current_bb,
                    arm.span,
                    TerminatorKind::Goto { target: merge_bb },
                );
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

fn boxed_symbols_in_block(block: &hir::Block) -> HashSet<hir::SymbolId> {
    let mut out = HashSet::new();
    collect_boxed_symbols_in_block(block, &mut out);
    out
}

fn boxed_symbols_in_expr(expr: &hir::Expr) -> HashSet<hir::SymbolId> {
    let mut out = HashSet::new();
    collect_boxed_symbols_in_expr(expr, &mut out);
    out
}

fn collect_boxed_symbols_in_block(block: &hir::Block, out: &mut HashSet<hir::SymbolId>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
            hir::StmtKind::Expr(expr) => collect_boxed_symbols_in_expr(expr, out),
            hir::StmtKind::Val(decl) => {
                if let Some(init) = &decl.init {
                    collect_boxed_symbols_in_expr(init, out);
                }
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                collect_boxed_symbols_in_expr(lhs, out);
                collect_boxed_symbols_in_expr(rhs, out);
            }
            hir::StmtKind::While { cond, body } => {
                collect_boxed_symbols_in_expr(cond, out);
                collect_boxed_symbols_in_block(body, out);
            }
            hir::StmtKind::Return { value } => {
                if let Some(v) = value {
                    collect_boxed_symbols_in_expr(v, out);
                }
            }
        }
    }
}

fn collect_boxed_symbols_in_expr(expr: &hir::Expr, out: &mut HashSet<hir::SymbolId>) {
    match &expr.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::Todo(_) => {}
        hir::ExprKind::StructLit { fields, .. } => {
            for f in fields {
                collect_boxed_symbols_in_expr(&f.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for e in elements {
                collect_boxed_symbols_in_expr(e, out);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = p {
                    collect_boxed_symbols_in_expr(expr, out);
                }
            }
        }
        hir::ExprKind::Unary { expr, .. } => collect_boxed_symbols_in_expr(expr.as_ref(), out),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_boxed_symbols_in_expr(lhs.as_ref(), out);
            collect_boxed_symbols_in_expr(rhs.as_ref(), out);
        }
        hir::ExprKind::Block(block) => collect_boxed_symbols_in_block(block, out),
        hir::ExprKind::Closure(closure) => {
            for cap in &closure.captures {
                if cap.mutable {
                    out.insert(cap.id);
                }
            }
            collect_boxed_symbols_in_expr(closure.body.as_ref(), out);
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_boxed_symbols_in_expr(cond, out);
            collect_boxed_symbols_in_expr(then_branch, out);
            if let Some(e) = else_branch.as_deref() {
                collect_boxed_symbols_in_expr(e, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_boxed_symbols_in_expr(subject, out);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_boxed_symbols_in_expr(g, out);
                }
                collect_boxed_symbols_in_expr(&arm.body, out);
            }
        }
        hir::ExprKind::MemberAccess { receiver, .. } => {
            collect_boxed_symbols_in_expr(receiver, out)
        }
        hir::ExprKind::Call { callee, args } => {
            collect_boxed_symbols_in_expr(callee, out);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_boxed_symbols_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => collect_boxed_symbols_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_boxed_symbols_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => collect_boxed_symbols_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Handle(handle) => {
            collect_boxed_symbols_in_block(&handle.body, out);
            for arm in &handle.arms {
                collect_boxed_symbols_in_expr(&arm.body, out);
            }
            if let Some(finally) = &handle.finally {
                collect_boxed_symbols_in_block(finally, out);
            }
        }
    }
}
