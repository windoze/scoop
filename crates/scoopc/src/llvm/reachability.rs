//! LLVM lowering 入口使用的 reachability 收集。
//!
//! 这层负责在真正进入 backend lowering 之前，先把“入口 `main` 会触达哪些
//! 顶层函数 / ctor / class init 相关实现成员”整理出来，避免在 emit API 或
//! `llvm/mod.rs` 根模块里混放大段 HIR 扫描逻辑。

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::ast;
use crate::hir;
use crate::mir;
use crate::span::Span;

pub(super) struct ReachabilityInputs<'a> {
    pub(super) class_inits: &'a hir::ClassInitIndex,
    pub(super) class_vtables: &'a crate::vtable::ClassVtableIndex,
    pub(super) class_itables: &'a crate::itable::ClassItableIndex,
    pub(super) ctor_call_sites: &'a hir::CtorCallSiteIndex,
    pub(super) top_level_vars: &'a hir::TopLevelVarIndex,
    pub(super) top_level_consts: &'a hir::TopLevelConstIndex,
    pub(super) top_level_immutable_values: &'a hir::TopLevelImmutableValueIndex,
    pub(super) object_inits: &'a hir::ObjectInitIndex,
}

pub(super) fn collect_reachable_top_level_funs<'a>(
    entry: &'a hir::FunDecl,
    fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    materialized_pass_view: Option<&'a mir::MaterializedMirPassView<'a>>,
    inputs: ReachabilityInputs<'a>,
) -> Vec<&'a hir::FunDecl> {
    let ReachabilityInputs {
        class_inits,
        class_vtables,
        class_itables,
        ctor_call_sites,
        top_level_vars,
        top_level_consts,
        top_level_immutable_values,
        object_inits,
    } = inputs;
    let mut collector = ReachabilityCollector {
        fun_index,
        class_inits,
        class_vtables,
        class_itables,
        ctor_call_sites,
        top_level_vars,
        top_level_consts,
        top_level_immutable_values,
        object_inits,
        materialized_pass_view,
        seen_calls: HashSet::new(),
        fun_queue: VecDeque::new(),
        reachable_funs: HashSet::new(),
        seen_ctors: HashSet::new(),
        ctor_queue: VecDeque::new(),
        scanned_class_init_steps: HashSet::new(),
        scanned_top_level_vars: HashSet::new(),
        scanned_top_level_consts: HashSet::new(),
        scanned_top_level_immutable_values: HashSet::new(),
        scanned_object_inits: HashSet::new(),
        current_source_path: None,
    };

    // 入口：扫描 `main` 的函数体，但不把 `main` 本身加入 reachable 集合（它由 `codegen_main_exit_code` 生成）。
    collector.scan_fun(entry);

    // BFS：同时处理“顶层函数调用”和“class ctor 调用”（会引入 class init / ctor delegation 中的调用点）。
    loop {
        let mut progressed = false;

        if let Some(fqn) = collector.fun_queue.pop_front() {
            progressed = true;
            let Some(fun) = collector.fun_index.get(&fqn).copied() else {
                // 外部/内建函数：不在本文件 fun_index 里（例如 runtime intrinsics），跳过。
                continue;
            };
            if fun.name == "main" {
                continue;
            }
            if !collector.reachable_funs.insert(fqn.clone()) {
                continue;
            }
            collector.scan_fun(fun);
        }

        if let Some((class_fqn, ctor_span)) = collector.ctor_queue.pop_front() {
            progressed = true;
            collector.scan_ctor(&class_fqn, ctor_span);
        }

        if !progressed {
            break;
        }
    }

    collector
        .reachable_funs
        .into_iter()
        .filter_map(|fqn| collector.fun_index.get(&fqn).copied())
        .collect()
}

struct ReachabilityCollector<'a> {
    fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    materialized_pass_view: Option<&'a mir::MaterializedMirPassView<'a>>,
    class_inits: &'a hir::ClassInitIndex,
    class_vtables: &'a crate::vtable::ClassVtableIndex,
    class_itables: &'a crate::itable::ClassItableIndex,
    ctor_call_sites: &'a hir::CtorCallSiteIndex,
    top_level_vars: &'a hir::TopLevelVarIndex,
    top_level_consts: &'a hir::TopLevelConstIndex,
    top_level_immutable_values: &'a hir::TopLevelImmutableValueIndex,
    object_inits: &'a hir::ObjectInitIndex,

    seen_calls: HashSet<String>,
    fun_queue: VecDeque<String>,
    reachable_funs: HashSet<String>,

    seen_ctors: HashSet<(String, Option<Span>)>,
    ctor_queue: VecDeque<(String, Option<Span>)>,

    scanned_class_init_steps: HashSet<String>,
    scanned_top_level_vars: HashSet<String>,
    scanned_top_level_consts: HashSet<String>,
    scanned_top_level_immutable_values: HashSet<String>,
    scanned_object_inits: HashSet<String>,
    current_source_path: Option<PathBuf>,
}

impl<'a> ReachabilityCollector<'a> {
    fn with_source_path<T>(&mut self, source_path: &Path, f: impl FnOnce(&mut Self) -> T) -> T {
        let prev = self.current_source_path.replace(source_path.to_path_buf());
        let out = f(self);
        self.current_source_path = prev;
        out
    }

    fn current_call_site(&self, span: Span) -> Option<hir::CallSite> {
        self.current_source_path
            .as_ref()
            .map(|path| hir::CallSite::new(path.clone(), span))
    }

    fn enqueue_fun(&mut self, fqn: String) {
        if self.seen_calls.insert(fqn.clone()) {
            self.fun_queue.push_back(fqn);
        }
    }

    fn scan_top_level_const(&mut self, fqn: &str) {
        if !self.scanned_top_level_consts.insert(fqn.to_string()) {
            return;
        }
        let Some(top_level_const) = self.top_level_consts.get(fqn).cloned() else {
            return;
        };
        self.with_source_path(top_level_const.source_path.as_path(), |this| {
            if let Some(init) = top_level_const.init.as_ref() {
                this.scan_expr(init);
            }
        });
    }

    fn scan_top_level_var(&mut self, fqn: &str) {
        if !self.scanned_top_level_vars.insert(fqn.to_string()) {
            return;
        }
        let Some(var) = self.top_level_vars.get(fqn).cloned() else {
            return;
        };
        self.with_source_path(var.source_path.as_path(), |this| {
            if let Some(init) = var.init.as_ref() {
                this.scan_expr(init);
            }
        });
    }

    fn scan_top_level_immutable_value(&mut self, fqn: &str) {
        if !self
            .scanned_top_level_immutable_values
            .insert(fqn.to_string())
        {
            return;
        }
        let Some(value) = self.top_level_immutable_values.get(fqn).cloned() else {
            return;
        };
        self.with_source_path(value.source_path.as_path(), |this| {
            if let Some(init) = value.init.as_ref() {
                this.scan_expr(init);
            }
        });
    }

    fn scan_object_init(&mut self, fqn: &str) {
        if !self.scanned_object_inits.insert(fqn.to_string()) {
            return;
        }
        let Some(object) = self.object_inits.get(fqn).cloned() else {
            return;
        };
        self.with_source_path(object.source_path.as_path(), |this| {
            for step in &object.steps {
                match step {
                    hir::ObjectInitStep::PropertyInit { init, .. } => this.scan_expr(init),
                    hir::ObjectInitStep::InitBlock { block } => this.scan_block(block),
                }
            }
        });
    }

    fn scan_top_level_value_ref(&mut self, fqn: &str) {
        self.scan_top_level_const(fqn);
        self.scan_top_level_immutable_value(fqn);
        self.scan_top_level_var(fqn);
        self.scan_object_init(fqn);
    }

    fn enqueue_vtable_impls(&mut self, class_fqn: &str) {
        let Some(slots) = self.class_vtables.get(class_fqn) else {
            return;
        };
        for slot in slots {
            self.enqueue_fun(slot.impl_member_fqn.clone());
        }
    }

    fn enqueue_itable_impls(&mut self, class_fqn: &str) {
        let Some(entries) = self.class_itables.get(class_fqn) else {
            return;
        };
        for entry in entries {
            for fqn in &entry.method_impl_fqns {
                if fqn.is_empty() {
                    continue;
                }
                self.enqueue_fun(fqn.clone());
            }
        }
    }

    fn enqueue_ctor(&mut self, class_fqn: String, ctor_span: Option<Span>) {
        let key = (class_fqn, ctor_span);
        if self.seen_ctors.insert(key.clone()) {
            self.ctor_queue.push_back(key);
        }
    }

    fn enqueue_ctor_call_site(&mut self, call_span: Span) {
        let Some(call_site) = self.current_call_site(call_span) else {
            return;
        };
        let Some(info) = self.ctor_call_sites.get(&call_site) else {
            return;
        };

        self.enqueue_ctor(info.class_fqn.clone(), info.ctor_span);
    }

    fn pick_ctor_by_call_target<'b>(
        &self,
        class: &'b hir::ClassInit,
        ctor_span: Option<Span>,
    ) -> Option<&'b hir::ClassCtor> {
        match ctor_span {
            Some(span) => class.ctors.iter().find(|ctor| ctor.span == span),
            None => {
                if class.ctors.is_empty() {
                    return None;
                }
                let mut matching: Vec<&hir::ClassCtor> = class
                    .ctors
                    .iter()
                    .filter(|ctor| ctor.params.is_empty())
                    .collect();
                if matching.len() != 1 {
                    return None;
                }
                Some(matching.pop().expect("len == 1"))
            }
        }
    }

    fn scan_call_arg(&mut self, arg: &hir::CallArg) {
        match arg {
            hir::CallArg::Positional(expr) => self.scan_expr(expr),
            hir::CallArg::Named { value, .. } => self.scan_expr(value),
        }
    }

    fn scan_fun(&mut self, fun: &hir::FunDecl) {
        if let Some(pass_view) = self.materialized_pass_view {
            if self.hir_fun_requires_hir_compat_scan(fun) {
                self.scan_hir_fun_body(fun);
                return;
            }
            let body_is_overridden = pass_view.callable_body_is_overridden(&fun.fqn);
            if body_is_overridden {
                if let Some(pass_fun) = self.canonical_mir_fun(fun) {
                    self.scan_mir_fun(pass_fun);
                }
                return;
            }
            if pass_view.owner_of_callable(&fun.fqn).is_some()
                || self.raw_non_generic_candidate_body(fun).is_some()
            {
                if let Some(pass_fun) = self.canonical_mir_fun(fun) {
                    if self.mir_fun_requires_hir_compat_scan(pass_fun) {
                        self.scan_hir_fun_body(fun);
                    } else {
                        self.scan_mir_fun(pass_fun);
                    }
                } else {
                    self.scan_hir_fun_body(fun);
                }
                return;
            }
        }

        self.scan_hir_fun_body(fun);
    }

    fn scan_hir_fun_body(&mut self, fun: &hir::FunDecl) {
        self.with_source_path(fun.source_path.as_path(), |this| {
            let Some(body) = fun.body.as_ref() else {
                return;
            };
            this.scan_block(body);
        });
    }

    fn hir_fun_requires_hir_compat_scan(&self, fun: &hir::FunDecl) -> bool {
        let Some(body) = fun.body.as_ref() else {
            return false;
        };
        self.hir_block_contains_hir_compat_only_effects(body)
    }

    fn hir_block_contains_hir_compat_only_effects(&self, block: &hir::Block) -> bool {
        block
            .stmts
            .iter()
            .any(|stmt| self.hir_stmt_contains_hir_compat_only_effects(stmt))
    }

    fn hir_stmt_contains_hir_compat_only_effects(&self, stmt: &hir::Stmt) -> bool {
        match &stmt.kind {
            hir::StmtKind::Empty | hir::StmtKind::Break { .. } | hir::StmtKind::Continue { .. } => {
                false
            }
            hir::StmtKind::Expr(expr) => self.hir_expr_contains_hir_compat_only_effects(expr),
            hir::StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .is_some_and(|expr| self.hir_expr_contains_hir_compat_only_effects(expr)),
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.hir_expr_contains_hir_compat_only_effects(lhs)
                    || self.hir_expr_contains_hir_compat_only_effects(rhs)
            }
            hir::StmtKind::While { cond, body } => {
                self.hir_expr_contains_hir_compat_only_effects(cond)
                    || self.hir_block_contains_hir_compat_only_effects(body)
            }
            hir::StmtKind::Return { value } => value
                .as_ref()
                .is_some_and(|expr| self.hir_expr_contains_hir_compat_only_effects(expr)),
            hir::StmtKind::Todo(_) => true,
        }
    }

    fn hir_expr_contains_hir_compat_only_effects(&self, expr: &hir::Expr) -> bool {
        match &expr.kind {
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Todo(_) => false,
            hir::ExprKind::StructLit { fields, .. } => fields
                .iter()
                .any(|field| self.hir_expr_contains_hir_compat_only_effects(&field.value)),
            hir::ExprKind::TupleLit { elements } => elements
                .iter()
                .any(|element| self.hir_expr_contains_hir_compat_only_effects(element)),
            hir::ExprKind::InterpolatedString { parts, .. } => {
                parts.iter().any(|part| match part {
                    hir::InterpolatedStringPart::Text { .. } => false,
                    hir::InterpolatedStringPart::Expr { expr } => {
                        self.hir_expr_contains_hir_compat_only_effects(expr)
                    }
                })
            }
            hir::ExprKind::Unary { expr, .. }
            | hir::ExprKind::TypeCheck { expr, .. }
            | hir::ExprKind::Cast { expr, .. } => {
                self.hir_expr_contains_hir_compat_only_effects(expr)
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.hir_expr_contains_hir_compat_only_effects(lhs)
                    || self.hir_expr_contains_hir_compat_only_effects(rhs)
            }
            hir::ExprKind::Block(block) => self.hir_block_contains_hir_compat_only_effects(block),
            hir::ExprKind::Closure(closure) => {
                self.hir_expr_contains_hir_compat_only_effects(&closure.body)
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.hir_expr_contains_hir_compat_only_effects(cond)
                    || self.hir_expr_contains_hir_compat_only_effects(then_branch)
                    || else_branch
                        .as_ref()
                        .is_some_and(|expr| self.hir_expr_contains_hir_compat_only_effects(expr))
            }
            hir::ExprKind::When { subject, arms } => {
                self.hir_expr_contains_hir_compat_only_effects(subject)
                    || arms.iter().any(|arm| {
                        arm.guard.as_ref().is_some_and(|guard| {
                            self.hir_expr_contains_hir_compat_only_effects(guard)
                        }) || self.hir_expr_contains_hir_compat_only_effects(&arm.body)
                    })
            }
            hir::ExprKind::MemberAccess { receiver, .. } => {
                self.hir_expr_contains_hir_compat_only_effects(receiver)
            }
            hir::ExprKind::Call { callee, args } => {
                self.hir_expr_contains_hir_compat_only_effects(callee)
                    || args.iter().any(|arg| match arg {
                        hir::CallArg::Positional(expr) => {
                            self.hir_expr_contains_hir_compat_only_effects(expr)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.hir_expr_contains_hir_compat_only_effects(value)
                        }
                    })
            }
            hir::ExprKind::Perform { .. } | hir::ExprKind::Handle(_) => true,
        }
    }

    fn canonical_mir_fun(&self, fun: &hir::FunDecl) -> Option<&'a mir::FunDecl> {
        let pass_view = self.materialized_pass_view?;
        if pass_view.callable_body_is_overridden(&fun.fqn)
            || pass_view.owner_of_callable(&fun.fqn).is_some()
        {
            return pass_view.callable(&fun.fqn);
        }
        self.raw_non_generic_candidate_body(fun)
    }

    fn raw_non_generic_candidate_body(&self, fun: &hir::FunDecl) -> Option<&'a mir::FunDecl> {
        self.materialized_pass_view?
            .materialized()
            .caller_side_pass_candidate_bodies()
            .iter()
            .find(|candidate| {
                candidate.fqn == fun.fqn
                    && candidate.body.is_some()
                    && self.raw_non_generic_candidate_matches_published_scope(candidate)
            })
    }

    fn raw_non_generic_candidate_matches_published_scope(&self, fun: &mir::FunDecl) -> bool {
        let Some(body) = fun.body.as_ref() else {
            return false;
        };
        body.blocks.iter().any(|block| {
            block.stmts.iter().any(|stmt| {
                let mir::StatementKind::Assign { value, .. } = &stmt.kind else {
                    return false;
                };
                match value {
                    mir::Rvalue::PatternMatch { .. } | mir::Rvalue::PatternExtract { .. } => true,
                    mir::Rvalue::MakeTuple { .. }
                    | mir::Rvalue::TupleGet { .. }
                    | mir::Rvalue::MakeClosure { .. } => true,
                    mir::Rvalue::Call {
                        kind: mir::CallKind::Closure { .. },
                        ..
                    } => true,
                    mir::Rvalue::TopLevelRef(mir::TopLevelRef { fqn }) => {
                        self.object_inits.contains_key(fqn)
                            || self.top_level_consts.contains_key(fqn)
                            || self.top_level_immutable_values.contains_key(fqn)
                            || self.top_level_vars.contains_key(fqn)
                    }
                    _ => false,
                }
            })
        })
    }

    fn scan_mir_fun(&mut self, fun: &mir::FunDecl) {
        let Some(body) = fun.body.as_ref() else {
            return;
        };
        self.scan_mir_body(body);
    }

    fn mir_fun_requires_hir_compat_scan(&self, fun: &mir::FunDecl) -> bool {
        let Some(body) = fun.body.as_ref() else {
            return true;
        };
        body.blocks.iter().any(|block| {
            block
                .stmts
                .iter()
                .any(|stmt| self.mir_statement_requires_hir_compat_scan(stmt))
                || self.mir_terminator_requires_hir_compat_scan(&block.terminator.kind)
        })
    }

    fn mir_statement_requires_hir_compat_scan(&self, stmt: &mir::Statement) -> bool {
        match &stmt.kind {
            mir::StatementKind::Nop => false,
            mir::StatementKind::Assign { value, .. } => {
                self.mir_rvalue_requires_hir_compat_scan(value)
            }
            mir::StatementKind::Todo(_) => true,
        }
    }

    fn mir_rvalue_requires_hir_compat_scan(&self, value: &mir::Rvalue) -> bool {
        match value {
            mir::Rvalue::Use(_)
            | mir::Rvalue::TopLevelRef(_)
            | mir::Rvalue::Unary { .. }
            | mir::Rvalue::Binary { .. }
            | mir::Rvalue::MakeTuple { .. }
            | mir::Rvalue::TupleGet { .. }
            | mir::Rvalue::MakeClosure { .. }
            | mir::Rvalue::PatternMatch { .. }
            | mir::Rvalue::PatternExtract { .. } => false,
            mir::Rvalue::Call { kind, .. } => self.mir_call_kind_requires_hir_compat_scan(kind),
            mir::Rvalue::UnresolvedName { .. }
            | mir::Rvalue::TypeCheck { .. }
            | mir::Rvalue::Cast { .. }
            | mir::Rvalue::MemberAccess { .. }
            | mir::Rvalue::CaptureBoxNew { .. }
            | mir::Rvalue::CaptureBoxGet { .. }
            | mir::Rvalue::CaptureBoxSet { .. }
            | mir::Rvalue::PerformResult { .. }
            | mir::Rvalue::Todo(_) => true,
        }
    }

    fn mir_call_kind_requires_hir_compat_scan(&self, kind: &mir::CallKind) -> bool {
        match kind {
            mir::CallKind::Direct { callee_fqn } => self
                .fun_index
                .get(callee_fqn)
                .is_some_and(|fun| fun.body.is_none()),
            mir::CallKind::Closure { .. } => false,
            mir::CallKind::FunValue { .. }
            | mir::CallKind::Virtual { .. }
            | mir::CallKind::Interface { .. }
            | mir::CallKind::Resume { .. } => true,
        }
    }

    fn mir_terminator_requires_hir_compat_scan(&self, kind: &mir::TerminatorKind) -> bool {
        match kind {
            mir::TerminatorKind::Return { value } => value.is_none(),
            mir::TerminatorKind::Goto { .. } | mir::TerminatorKind::Unreachable => false,
            mir::TerminatorKind::CondBr { cond, .. } => {
                !matches!(cond, mir::Operand::Local(_) | mir::Operand::Const(_))
            }
            mir::TerminatorKind::ResumeUnwind
            | mir::TerminatorKind::Perform { .. }
            | mir::TerminatorKind::Handle { .. }
            | mir::TerminatorKind::Todo(_) => true,
        }
    }

    fn scan_mir_body(&mut self, body: &mir::Body) {
        match body.reachable_blocks() {
            Ok(blocks) => {
                for block_id in blocks {
                    let Some(block) = body.blocks.get(block_id.as_u32() as usize) else {
                        continue;
                    };
                    self.scan_mir_block(block);
                }
            }
            Err(_) => {
                for block in &body.blocks {
                    self.scan_mir_block(block);
                }
            }
        }
    }

    fn scan_mir_block(&mut self, block: &mir::BasicBlock) {
        for stmt in &block.stmts {
            if let mir::StatementKind::Assign { value, .. } = &stmt.kind {
                self.scan_mir_rvalue(value);
            }
        }
        self.scan_mir_terminator(&block.terminator);
    }

    fn scan_mir_terminator(&mut self, terminator: &mir::Terminator) {
        if let mir::TerminatorKind::Perform { args, .. } = &terminator.kind {
            for arg in args {
                self.scan_mir_operand(&arg.value);
            }
        }
    }

    fn scan_mir_operand(&mut self, _operand: &mir::Operand) {
        // MIR operands are already locals/constants; any nested top-level refs or calls are
        // represented by earlier statements and scanned through their rvalues.
    }

    fn scan_mir_rvalue(&mut self, value: &mir::Rvalue) {
        match value {
            mir::Rvalue::Use(operand)
            | mir::Rvalue::Unary { operand, .. }
            | mir::Rvalue::TypeCheck { value: operand, .. }
            | mir::Rvalue::Cast { value: operand, .. }
            | mir::Rvalue::TupleGet { tuple: operand, .. }
            | mir::Rvalue::CaptureBoxNew { value: operand }
            | mir::Rvalue::CaptureBoxGet {
                box_operand: operand,
            }
            | mir::Rvalue::PatternMatch {
                subject: operand, ..
            }
            | mir::Rvalue::PatternExtract {
                subject: operand, ..
            } => {
                self.scan_mir_operand(operand);
            }
            mir::Rvalue::PerformResult { .. }
            | mir::Rvalue::UnresolvedName { .. }
            | mir::Rvalue::Todo(_) => {}
            mir::Rvalue::TopLevelRef(mir::TopLevelRef { fqn }) => {
                self.scan_top_level_value_ref(fqn)
            }
            mir::Rvalue::Binary { lhs, rhs, .. } => {
                self.scan_mir_operand(lhs);
                self.scan_mir_operand(rhs);
            }
            mir::Rvalue::MemberAccess { receiver, .. } => {
                self.scan_mir_operand(receiver);
            }
            mir::Rvalue::Call { kind, args } => {
                self.scan_mir_call_kind(kind);
                for arg in args {
                    self.scan_mir_operand(&arg.value);
                }
            }
            mir::Rvalue::MakeTuple { elements } => {
                for element in elements {
                    self.scan_mir_operand(element);
                }
            }
            mir::Rvalue::CaptureBoxSet { box_operand, value } => {
                self.scan_mir_operand(box_operand);
                self.scan_mir_operand(value);
            }
            mir::Rvalue::MakeClosure { env, fn_ptr } => {
                self.scan_mir_operand(env);
                self.enqueue_fun(fn_ptr.clone());
            }
        }
    }

    fn scan_mir_call_kind(&mut self, kind: &mir::CallKind) {
        match kind {
            mir::CallKind::Direct { callee_fqn } => self.enqueue_fun(callee_fqn.clone()),
            mir::CallKind::Closure { callee, fn_ptr } => {
                self.scan_mir_operand(callee);
                self.enqueue_fun(fn_ptr.clone());
            }
            mir::CallKind::FunValue { callee } => self.scan_mir_operand(callee),
            mir::CallKind::Virtual { receiver, .. } | mir::CallKind::Interface { receiver, .. } => {
                self.scan_mir_operand(receiver);
            }
            mir::CallKind::Resume { continuation, .. } => self.scan_mir_operand(continuation),
        }
    }

    fn scan_block(&mut self, block: &hir::Block) {
        for stmt in &block.stmts {
            self.scan_stmt(stmt);
        }
    }

    fn scan_stmt(&mut self, stmt: &hir::Stmt) {
        match &stmt.kind {
            hir::StmtKind::Empty => {}
            hir::StmtKind::Expr(expr) => self.scan_expr(expr),
            hir::StmtKind::Val(decl) => {
                if let Some(init) = decl.init.as_ref() {
                    self.scan_expr(init);
                }
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                self.scan_expr(lhs);
                self.scan_expr(rhs);
            }
            hir::StmtKind::Return { value } => {
                if let Some(expr) = value.as_ref() {
                    self.scan_expr(expr);
                }
            }
            hir::StmtKind::While { cond, body } => {
                self.scan_expr(cond);
                self.scan_block(body);
            }
            hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
        }
    }

    fn scan_expr(&mut self, expr: &hir::Expr) {
        match &expr.kind {
            hir::ExprKind::Missing | hir::ExprKind::Todo(_) => {}
            hir::ExprKind::Literal(_) | hir::ExprKind::UnresolvedIdent { .. } => {}
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                self.scan_top_level_value_ref(fqn);
            }
            hir::ExprKind::VarRef(hir::ValueRef::Local { .. }) => {}
            hir::ExprKind::StructLit { fields, .. } => {
                for f in fields {
                    self.scan_expr(&f.value);
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for e in elements {
                    self.scan_expr(e);
                }
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for p in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = p {
                        self.scan_expr(expr);
                    }
                }
            }
            hir::ExprKind::Unary { expr: inner, .. } => self.scan_expr(inner),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.scan_expr(lhs);
                self.scan_expr(rhs);
            }
            hir::ExprKind::TypeCheck { expr, .. } | hir::ExprKind::Cast { expr, .. } => {
                self.scan_expr(expr);
            }
            hir::ExprKind::Block(block) => self.scan_block(block),
            hir::ExprKind::Call { callee, args } => {
                // 顶层函数调用：收集 callee fqn。
                if let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind {
                    if self.fun_index.contains_key(fqn) {
                        self.enqueue_fun(fqn.clone());
                    } else {
                        self.scan_expr(callee);
                    }
                } else {
                    // callee 也可能是 `helper().member` / `foo()()` 这类复合表达式；
                    // 需要继续扫描 callee，避免漏掉其中嵌套的顶层函数或顶层 const 引用。
                    self.scan_expr(callee);
                }

                // constructor call：调用 span 会在 HIR side table 中出现已选 ctor 绑定。
                self.enqueue_ctor_call_site(expr.span);

                for arg in args {
                    self.scan_call_arg(arg);
                }
            }
            hir::ExprKind::Closure(c) => self.scan_expr(&c.body),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.scan_expr(cond);
                self.scan_expr(then_branch);
                if let Some(e) = else_branch.as_ref() {
                    self.scan_expr(e);
                }
            }
            hir::ExprKind::When { subject, arms } => {
                self.scan_expr(subject);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        self.scan_expr(guard);
                    }
                    self.scan_expr(&arm.body);
                }
            }
            hir::ExprKind::MemberAccess { receiver, .. } => self.scan_expr(receiver),
            hir::ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(e) => self.scan_expr(e),
                        hir::CallArg::Named { value, .. } => self.scan_expr(value),
                    }
                }
            }
            hir::ExprKind::Handle(h) => {
                self.scan_block(&h.body);
                for arm in &h.arms {
                    self.scan_expr(&arm.body);
                }
                if let Some(finally) = h.finally.as_ref() {
                    self.scan_block(finally);
                }
            }
        }
    }

    fn scan_class_init_steps(&mut self, class: &hir::ClassInit) {
        self.with_source_path(class.source_path.as_path(), |this| {
            for step in &class.steps {
                match step {
                    hir::ClassInitStep::PropertyInit { init, .. } => this.scan_expr(init),
                    hir::ClassInitStep::InitBlock { block } => this.scan_block(block),
                }
            }
        });
    }

    fn scan_ctor(&mut self, class_fqn: &str, ctor_span: Option<Span>) {
        let Some(class) = self.class_inits.get(class_fqn).cloned() else {
            return;
        };

        // T1508b：vtable 虚调用需要确保“可达的 class”其 vtable 实现成员也会被后端声明/生成。
        // - class ctor 可达 ⇒ 该 class 的对象可能被分配并参与动态分发；
        // - 因此这里把 vtable slots 指向的实现成员（impl_member_fqn）加入可达集合。
        self.enqueue_vtable_impls(class_fqn);

        // T1508c：interface dispatch 同样依赖 itable entries 中的目标成员可达（含默认方法）。
        self.enqueue_itable_impls(class_fqn);

        // class init steps（property initializer / init blocks）对所有构造路径都可达：只扫描一次。
        if self.scanned_class_init_steps.insert(class.fqn.clone()) {
            self.scan_class_init_steps(&class);
        }

        self.with_source_path(class.source_path.as_path(), |this| {
            let ctor = this.pick_ctor_by_call_target(&class, ctor_span);

            // delegation / super ctor args
            match ctor {
                Some(ctor) if ctor.kind == hir::ClassCtorKind::Secondary => {
                    if let Some(deleg) = ctor.delegation.as_ref() {
                        for arg in &deleg.args {
                            this.scan_call_arg(arg);
                        }
                        if let Some(call) = deleg.call.as_ref() {
                            this.enqueue_ctor(call.class_fqn.clone(), call.ctor_span);
                        } else {
                            match deleg.kind {
                                ast::CtorDelegationKind::This => {
                                    this.enqueue_ctor(class.fqn.clone(), None);
                                }
                                ast::CtorDelegationKind::Super => {
                                    if let Some(super_fqn) = class.super_class_fqn.as_deref() {
                                        this.enqueue_ctor(super_fqn.to_string(), None);
                                    }
                                }
                            }
                        }
                    } else {
                        // secondary ctor（无 delegation）：走 class header 的 super ctor args。
                        for arg in &class.super_ctor_args {
                            this.scan_call_arg(arg);
                        }
                        if let Some(call) = class.super_ctor_call.as_ref() {
                            this.enqueue_ctor(call.class_fqn.clone(), call.ctor_span);
                        } else if let Some(super_fqn) = class.super_class_fqn.as_deref() {
                            this.enqueue_ctor(super_fqn.to_string(), None);
                        }
                    }

                    // secondary ctor body
                    if let Some(body) = ctor.body.as_ref() {
                        this.scan_block(body);
                    }
                }
                _ => {
                    // primary ctor（或隐式 0-参 primary ctor）：走 class header 的 super ctor args。
                    for arg in &class.super_ctor_args {
                        this.scan_call_arg(arg);
                    }
                    if let Some(call) = class.super_ctor_call.as_ref() {
                        this.enqueue_ctor(call.class_fqn.clone(), call.ctor_span);
                    } else if let Some(super_fqn) = class.super_class_fqn.as_deref() {
                        this.enqueue_ctor(super_fqn.to_string(), None);
                    }
                }
            }

            if let Some(ctor) = ctor {
                for param in &ctor.params {
                    if let Some(default_value) = param.default_value.as_ref() {
                        this.scan_expr(default_value);
                    }
                }
            }
        });
    }
}
