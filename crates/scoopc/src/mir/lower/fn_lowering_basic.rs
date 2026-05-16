//! FnLowering construction, locals, basic statement lowering.

#![allow(dead_code)]

use super::*;

impl<'a> FnLowering<'a> {

    /// 创建一个新的函数 lowering builder。
    pub(in crate::mir::lower) fn new(
        builtins: BuiltinTypes,
        types: &'a mut TypeStore,
        facts: &'a MirLoweringFacts,
        top_level_fun_return_tys: HashMap<String, TypeId>,
        top_level_fun_param_tys: HashMap<String, Vec<TypeId>>,
        owner_fqn: String,
        source_path: std::path::PathBuf,
    ) -> Self {
        Self {
            builtins,
            types,
            facts,
            top_level_fun_return_tys,
            top_level_fun_param_tys,
            owner_fqn,
            source_path,
            current_return_ty: builtins.unit,
            body: Body::new_empty(),
            current_bb: BasicBlockId(0),
            next_temp: 0,
            next_site_id: 0,
            symbol_locals: HashMap::new(),
            value_origins: HashMap::new(),
            boxed_symbols: HashSet::new(),
            cleanup_scopes: Vec::new(),
            loop_stack: Vec::new(),
            nested_funs: Vec::new(),
        }
    }

    /// 把一个 HIR 函数声明降到 MIR（当前阶段仅关注 body 的 CFG 形态）。
    pub(in crate::mir::lower) fn lower_fun(mut self, fun: &hir::FunDecl) -> (FunDecl, Vec<FunDecl>) {
        self.current_return_ty = fun.return_ty;
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
            if fun.return_ty == self.builtins.unit {
                self.lower_block_as_stmt(block);
                self.finish_function(fun.span);
            } else {
                let body_result = self.lower_block_as_expr(block);
                if !self.current_is_terminated() {
                    let value =
                        self.operand_for_current_return_ty(fun.span, Operand::Local(body_result));
                    self.set_terminator(
                        self.current_bb,
                        fun.span,
                        TerminatorKind::Return { value: Some(value) },
                    );
                }
            }
            self.assign_deferred_class_ctor_site_ids();
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
    pub(in crate::mir::lower) fn push_block(&mut self, span: Span) -> BasicBlockId {
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

    pub(in crate::mir::lower) fn push_cleanup_block(&mut self, span: Span) -> BasicBlockId {
        let bb = self.push_block(span);
        self.body.blocks[bb.as_usize()].is_cleanup = true;
        bb
    }

    pub(in crate::mir::lower) fn fresh_site_id(&mut self) -> SiteId {
        let site_id = SiteId::from_raw(self.next_site_id);
        self.next_site_id = self
            .next_site_id
            .checked_add(1)
            .expect("too many MIR site ids in one body");
        site_id
    }

    pub(in crate::mir::lower) fn assign_deferred_class_ctor_site_ids(&mut self) {
        for block in &mut self.body.blocks {
            for stmt in &mut block.stmts {
                let StatementKind::Assign {
                    value: Rvalue::ClassCtor { site_id, .. },
                    ..
                } = &mut stmt.kind
                else {
                    continue;
                };
                if site_id.as_u32() != u32::MAX {
                    continue;
                }
                *site_id = SiteId::from_raw(self.next_site_id);
                self.next_site_id = self
                    .next_site_id
                    .checked_add(1)
                    .expect("too many MIR site ids in one body");
            }
        }
    }

    /// 在当前 basic block 末尾追加一条语句。
    pub(in crate::mir::lower) fn push_stmt(&mut self, span: Span, kind: StatementKind) {
        let bb = self.current_bb;
        self.body.blocks[bb.as_usize()]
            .stmts
            .push(Statement { span, kind });
    }

    /// 覆盖指定 basic block 的 terminator。
    pub(in crate::mir::lower) fn set_terminator_with_unwind(
        &mut self,
        bb: BasicBlockId,
        span: Span,
        kind: TerminatorKind,
        unwind: UnwindAction,
    ) {
        self.body.blocks[bb.as_usize()].terminator = Terminator { span, kind, unwind };
    }

    /// 覆盖指定 basic block 的 terminator（默认 `NoUnwind`）。
    pub(in crate::mir::lower) fn set_terminator(&mut self, bb: BasicBlockId, span: Span, kind: TerminatorKind) {
        self.set_terminator_with_unwind(bb, span, kind, UnwindAction::NoUnwind);
    }

    /// 当前 basic block 是否已经被 terminator 结束。
    pub(in crate::mir::lower) fn current_is_terminated(&self) -> bool {
        let bb = self.current_bb;
        !matches!(
            self.body.blocks[bb.as_usize()].terminator.kind,
            TerminatorKind::Todo(msg) if msg == UNTERMINATED
        )
    }

    /// 当前 block 若只是被占位式 effect terminator 截断，则为后续语句分配一个新的 continuation block。
    ///
    /// 说明：
    /// - 现阶段 `TerminatorKind::Handle` / `TerminatorKind::Perform` 仍未展开成真实 CFG；
    /// - 但某些语义糖或后续 lowering 形状会在 `handle { ... }` 之后继续出现普通 direct call，
    ///   并且恢复路径也仍需要在 generic MIR 中保形；
    /// - 若这里直接停止，generic MIR materializer 将看不到这些后续 call-site；
    /// - 因此仅当终止原因是占位式 `Handle` / `Perform` 时，允许把后续语句接到一个新的孤立 block 中继续保形。
    pub(in crate::mir::lower) fn continue_after_placeholder_effect_terminator_if_needed(&mut self, next_span: Span) -> bool {
        if self.facts.uses_typed_contracts() {
            return !self.current_is_terminated();
        }
        if !self.current_is_terminated() {
            return true;
        }
        if !matches!(
            self.body.blocks[self.current_bb.as_usize()].terminator.kind,
            TerminatorKind::Handle { .. } | TerminatorKind::Perform { .. }
        ) {
            return false;
        }
        self.current_bb = self.push_block(next_span);
        true
    }

    pub(in crate::mir::lower) fn with_cleanup_scope_len<T>(&mut self, len: usize, f: impl FnOnce(&mut Self) -> T) -> T {
        let mut tail = self.cleanup_scopes.split_off(len);
        let result = f(self);
        self.cleanup_scopes.append(&mut tail);
        result
    }

    pub(in crate::mir::lower) fn lower_cleanup_block_to_target(
        &mut self,
        cleanup_bb: BasicBlockId,
        cleanup: &hir::Block,
        target: BasicBlockId,
        outer_cleanup_len: usize,
    ) {
        let saved_bb = self.current_bb;
        self.current_bb = cleanup_bb;
        self.with_cleanup_scope_len(outer_cleanup_len, |this| {
            this.lower_block_as_stmt(cleanup);
        });
        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                cleanup.span,
                TerminatorKind::Goto { target },
            );
        }
        self.current_bb = saved_bb;
    }

    pub(in crate::mir::lower) fn build_cleanup_route(
        &mut self,
        target: BasicBlockId,
        min_cleanup_depth: usize,
    ) -> BasicBlockId {
        let mut next_target = target;
        for scope_index in (min_cleanup_depth..self.cleanup_scopes.len()).rev() {
            let cleanup = self.cleanup_scopes[scope_index].finally.clone();
            let cleanup_bb = self.push_cleanup_block(cleanup.span);
            self.lower_cleanup_block_to_target(cleanup_bb, &cleanup, next_target, scope_index);
            next_target = cleanup_bb;
        }
        next_target
    }

    pub(in crate::mir::lower) fn build_perform_unwind_action(&mut self, span: Span) -> UnwindAction {
        let Some(scope) = self.cleanup_scopes.last().cloned() else {
            return UnwindAction::Propagate;
        };

        let resume_unwind_bb = self.push_cleanup_block(span);
        self.set_terminator(resume_unwind_bb, span, TerminatorKind::ResumeUnwind);

        let cleanup_bb = self.push_cleanup_block(scope.finally.span);
        self.lower_cleanup_block_to_target(
            cleanup_bb,
            &scope.finally,
            resume_unwind_bb,
            self.cleanup_scopes.len() - 1,
        );
        UnwindAction::Cleanup { target: cleanup_bb }
    }

    /// 若函数尾部没有显式 terminator，则默认补一个 `return`（保持 body 可验证/可 dump）。
    pub(in crate::mir::lower) fn finish_function(&mut self, span: Span) {
        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Return { value: None },
            );
        }
    }

    /// 分配一个具名 local（用于参数与 `val/var` 声明）。
    pub(in crate::mir::lower) fn push_named_local(&mut self, span: Span, name: &str, ty: TypeId) -> LocalId {
        self.body.push_local(LocalDecl {
            span,
            name: Some(name.to_string()),
            ty,
            source: LocalSourceKind::SourceLocal,
        })
    }

    pub(in crate::mir::lower) fn local_for_assign_decl_span(&self, decl_span: Span, name: &str) -> Option<LocalId> {
        self.body
            .locals
            .iter()
            .enumerate()
            .filter_map(|(idx, local)| {
                (local.name.as_deref() == Some(name)
                    && local.span.start <= decl_span.start
                    && decl_span.end <= local.span.end)
                    .then_some((idx, local.span))
            })
            .min_by_key(|(_, span)| span.end.saturating_sub(span.start))
            .map(|(idx, _)| LocalId::from_raw(idx as u32))
    }

    pub(in crate::mir::lower) fn resolve_assign_local(
        &self,
        id: hir::SymbolId,
        name: &str,
        decl_span: Span,
    ) -> Option<LocalId> {
        self.symbol_locals
            .get(&id)
            .copied()
            .or_else(|| self.local_for_assign_decl_span(decl_span, name))
    }

    /// 分配一个临时 local（用于表达式求值与 if/when merge）。
    pub(in crate::mir::lower) fn push_temp_local(&mut self, span: Span, ty: TypeId) -> LocalId {
        let name = format!("tmp{}", self.next_temp);
        self.next_temp += 1;
        self.body.push_local(LocalDecl {
            span,
            name: Some(name),
            ty,
            source: LocalSourceKind::CompilerTemporary,
        })
    }

    /// 生成 `target = value` 赋值语句。
    pub(in crate::mir::lower) fn assign(&mut self, span: Span, target: LocalId, value: Rvalue) {
        self.record_value_origin(target, &value);
        self.push_stmt(span, StatementKind::Assign { target, value });
    }

    pub(in crate::mir::lower) fn value_erasure_transport(
        &self,
        source_ty: TypeId,
        target_ty: TypeId,
    ) -> Option<ValueTransportMetadata> {
        value_erasure_transport(self.builtins, self.types, self.facts, source_ty, target_ty)
    }

    pub(in crate::mir::lower) fn transporting_use_rvalue(&self, value: Operand, target_ty: TypeId) -> Rvalue {
        let source_ty = self.operand_ty(&value);
        if let Some(transport) = self.value_erasure_transport(source_ty, target_ty) {
            Rvalue::Transport { value, transport }
        } else {
            Rvalue::Use(value)
        }
    }

    pub(in crate::mir::lower) fn assign_use_to_local(&mut self, span: Span, target: LocalId, value: Operand) {
        let target_ty = self.body.locals[target.as_u32() as usize].ty;
        let rvalue = self.transporting_use_rvalue(value, target_ty);
        self.assign(span, target, rvalue);
    }

    pub(in crate::mir::lower) fn operand_for_target_ty(&mut self, span: Span, value: Operand, target_ty: TypeId) -> Operand {
        let source_ty = self.operand_ty(&value);
        let Some(transport) = self.value_erasure_transport(source_ty, target_ty) else {
            return value;
        };
        let tmp = self.push_temp_local(span, target_ty);
        self.assign(span, tmp, Rvalue::Transport { value, transport });
        Operand::Local(tmp)
    }

    pub(in crate::mir::lower) fn operand_for_current_return_ty(&mut self, span: Span, value: Operand) -> Operand {
        self.operand_for_target_ty(span, value, self.current_return_ty)
    }

    pub(in crate::mir::lower) fn is_function_value_ty(&self, ty: TypeId) -> bool {
        matches!(self.types.kind(ty), TypeKind::Ref(RefTypeKind::Function(_)))
    }

    pub(in crate::mir::lower) fn is_funptr_value_ty(&self, ty: TypeId) -> bool {
        matches!(
            self.types.kind(ty),
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.unsafe.FunPtr" && nominal.args.len() == 1
        )
    }

    pub(in crate::mir::lower) fn is_callable_value_ty(&self, ty: TypeId) -> bool {
        self.is_function_value_ty(ty) || self.is_funptr_value_ty(ty)
    }

    pub(in crate::mir::lower) fn value_origin_from_operand(&self, operand: &Operand) -> Option<ValueOrigin> {
        match operand {
            Operand::Local(local) => self.value_origins.get(local).cloned(),
            Operand::Const(_) => None,
        }
    }

    pub(in crate::mir::lower) fn classify_value_assignment(&self, target: LocalId, value: &Rvalue) -> Option<ValueOrigin> {
        let target_ty = self.body.locals[target.as_u32() as usize].ty;
        match value {
            Rvalue::MakeClosure { fn_ptr, .. } => Some(ValueOrigin::Closure {
                fn_ptr: fn_ptr.clone(),
            }),
            Rvalue::TopLevelRef(TopLevelRef { fqn, .. }) => {
                Some(ValueOrigin::TopLevelRef { fqn: fqn.clone() })
            }
            Rvalue::MemberAccess { member, .. } => Some(ValueOrigin::MemberAccess {
                member: member.clone(),
            }),
            Rvalue::UnresolvedName { name } => {
                Some(ValueOrigin::UnresolvedName { name: name.clone() })
            }
            Rvalue::Transport { value, .. } => self.value_origin_from_operand(value),
            Rvalue::Use(operand) => self.value_origin_from_operand(operand).or_else(|| {
                self.is_callable_value_ty(target_ty)
                    .then_some(ValueOrigin::UnknownCallable)
            }),
            _ => self
                .is_callable_value_ty(target_ty)
                .then_some(ValueOrigin::UnknownCallable),
        }
    }

    pub(in crate::mir::lower) fn merge_value_origin(
        current: Option<ValueOrigin>,
        next: Option<ValueOrigin>,
    ) -> Option<ValueOrigin> {
        match (current, next) {
            (None, None) => None,
            (_, None) => None,
            (None, Some(origin)) => Some(origin),
            (Some(left), Some(right)) if left == right => Some(left),
            (Some(_), Some(_)) => Some(ValueOrigin::UnknownCallable),
        }
    }

    pub(in crate::mir::lower) fn record_value_origin(&mut self, target: LocalId, value: &Rvalue) {
        let next = self.classify_value_assignment(target, value);
        let merged = Self::merge_value_origin(self.value_origins.get(&target).cloned(), next);
        match merged {
            Some(origin) => {
                self.value_origins.insert(target, origin);
            }
            None => {
                self.value_origins.remove(&target);
            }
        }
    }

    /// 把一个 block 作为“语句块”来 lower（顺序执行；最后表达式结果被丢弃）。
    pub(in crate::mir::lower) fn lower_block_as_stmt(&mut self, block: &hir::Block) {
        for stmt in &block.stmts {
            if !self.continue_after_placeholder_effect_terminator_if_needed(stmt.span) {
                break;
            }
            self.lower_stmt(stmt);
        }
    }

    /// 把一个 block 作为“表达式块”来 lower，并返回 block 的结果 local。
    pub(in crate::mir::lower) fn lower_block_as_expr(&mut self, block: &hir::Block) -> LocalId {
        let mut result: Option<LocalId> = None;
        for (idx, stmt) in block.stmts.iter().enumerate() {
            if !self.continue_after_placeholder_effect_terminator_if_needed(stmt.span) {
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
    pub(in crate::mir::lower) fn lower_stmt(&mut self, stmt: &hir::Stmt) {
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
                let return_value = if let Some(expr) = value {
                    let result = self.lower_expr_to_local(expr);
                    if self.current_is_terminated() {
                        return;
                    }
                    Some(self.operand_for_current_return_ty(stmt.span, Operand::Local(result)))
                } else {
                    None
                };

                if self.cleanup_scopes.is_empty() {
                    self.set_terminator(
                        self.current_bb,
                        stmt.span,
                        TerminatorKind::Return {
                            value: return_value,
                        },
                    );
                    return;
                }

                let return_bb = self.push_block(stmt.span);
                self.set_terminator(
                    return_bb,
                    stmt.span,
                    TerminatorKind::Return {
                        value: return_value,
                    },
                );
                let cleanup_target = self.build_cleanup_route(return_bb, 0);
                self.set_terminator(
                    self.current_bb,
                    stmt.span,
                    TerminatorKind::Goto {
                        target: cleanup_target,
                    },
                );
            }
            hir::StmtKind::Todo(kind) => self.push_stmt(stmt.span, StatementKind::Todo(kind)),
        }
    }

    /// 降低一个 `while` 语句：构造 loop CFG，并为 `break/continue` 建立跳转目标。
    pub(in crate::mir::lower) fn lower_while_stmt(&mut self, span: Span, cond: &hir::Expr, body: &hir::Block) {
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
            cleanup_depth: self.cleanup_scopes.len(),
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
    pub(in crate::mir::lower) fn lower_break_stmt(&mut self, span: Span) {
        let Some(ctx) = self.loop_stack.last().copied() else {
            panic!("typecheck must reject `break` outside loops before MIR lowering: {span:?}");
        };
        if self.cleanup_scopes.len() == ctx.cleanup_depth {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Goto {
                    target: ctx.break_target,
                },
            );
            return;
        }
        let cleanup_target = self.build_cleanup_route(ctx.break_target, ctx.cleanup_depth);
        self.set_terminator(
            self.current_bb,
            span,
            TerminatorKind::Goto {
                target: cleanup_target,
            },
        );
    }

    /// 降低 `continue`：跳转到当前 loop 的 cond block。
    pub(in crate::mir::lower) fn lower_continue_stmt(&mut self, span: Span) {
        let Some(ctx) = self.loop_stack.last().copied() else {
            panic!("typecheck must reject `continue` outside loops before MIR lowering: {span:?}");
        };
        if self.cleanup_scopes.len() == ctx.cleanup_depth {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Goto {
                    target: ctx.continue_target,
                },
            );
            return;
        }
        let cleanup_target = self.build_cleanup_route(ctx.continue_target, ctx.cleanup_depth);
        self.set_terminator(
            self.current_bb,
            span,
            TerminatorKind::Goto {
                target: cleanup_target,
            },
        );
    }

    /// 降低一个 `val/var` 声明：分配 local，并 lower initializer（若存在）。
    pub(in crate::mir::lower) fn lower_val_decl(&mut self, decl: &hir::ValDecl) {
        let id = decl.id.unwrap_or_else(|| {
            panic!(
                "typed HIR local declaration must have a symbol id: {:?}",
                decl.span
            )
        });

        let name = decl.name.as_deref().unwrap_or("<anon>");
        // `var` 若被 closure 捕获，需要在本函数内以 box 形式存储，保证后续读写别名一致（T0714）。
        if decl.mutable && self.boxed_symbols.contains(&id) {
            let box_ty = self.capture_box_ty(decl.ty);
            let local = self.push_named_local(decl.span, name, box_ty);
            self.symbol_locals.insert(id, local);

            if let Some(init) = &decl.init {
                let value = self.lower_expr_to_local(init);
                if self.current_is_terminated() {
                    return;
                }
                self.assign(
                    decl.span,
                    local,
                    Rvalue::CaptureBoxNew {
                        value: Operand::Local(value),
                        contract: self.capture_box_contract(box_ty, decl.ty),
                    },
                );
            } else {
                panic!(
                    "typecheck must reject captured mutable locals without initializer before MIR lowering: {:?}",
                    decl.span
                );
            }
            return;
        }

        let local = self.push_named_local(decl.span, name, decl.ty);
        self.symbol_locals.insert(id, local);

        if let Some(init) = &decl.init {
            let value = self.lower_expr_to_local(init);
            if self.current_is_terminated() {
                return;
            }
            self.assign_use_to_local(decl.span, local, Operand::Local(value));
        }
    }

    /// 降低一个赋值语句。
    pub(in crate::mir::lower) fn lower_assign_stmt(&mut self, span: Span, lhs: &hir::Expr, rhs: &hir::Expr) {
        self.lower_assign_stmt_with_place_contract(span, lhs, rhs);
    }

    pub(in crate::mir::lower) fn lower_assign_stmt_with_place_contract(
        &mut self,
        span: Span,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) {
        let Some(contract) = self
            .facts
            .assign_place_contract(self.source_path.as_path(), span)
            .cloned()
        else {
            panic!("typed HIR assignment must have a place contract before MIR lowering: {span:?}");
        };

        match &contract.kind {
            hir::AssignPlaceKind::Local {
                id,
                name,
                decl_span,
            } => {
                // explicit MIR instance lowering may re-lower the same source body in a fresh
                // HIR context, so the contract's SymbolId can drift while its source decl span
                // remains authoritative for the current body.
                let target = self
                    .resolve_assign_local(*id, name, *decl_span)
                    .unwrap_or_else(|| {
                        panic!(
                            "assignment place contract references an unallocated local: {id:?} ({name} @ {decl_span:?})"
                        )
                    });

                let value = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return;
                }
                if self.boxed_symbols.contains(id) {
                    let tmp = self.push_temp_local(span, self.builtins.unit);
                    self.assign(
                        span,
                        tmp,
                        Rvalue::CaptureBoxSet {
                            box_operand: Operand::Local(target),
                            value: Operand::Local(value),
                            contract: self.capture_box_contract(
                                self.body.locals[target.as_u32() as usize].ty,
                                self.body.locals[value.as_u32() as usize].ty,
                            ),
                        },
                    );
                } else {
                    self.assign_use_to_local(span, target, Operand::Local(value));
                }
            }
            hir::AssignPlaceKind::TopLevel { fqn, .. } => {
                let value_local = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return;
                }
                let value = self.operand_for_target_ty(
                    span,
                    Operand::Local(value_local),
                    contract.value_ty,
                );
                self.push_stmt(
                    span,
                    StatementKind::StoreTopLevelVar {
                        fqn: fqn.clone(),
                        value,
                        value_ty: contract.value_ty,
                    },
                );
            }
            hir::AssignPlaceKind::Member {
                receiver_ty,
                member_name,
                resolved,
                ..
            } => {
                let hir::ExprKind::MemberAccess { receiver, .. } = &lhs.kind else {
                    panic!(
                        "member assignment place contract must match a member-access lhs: {span:?}"
                    );
                };
                let receiver_local = self.lower_expr_to_local(receiver);
                if self.current_is_terminated() {
                    return;
                }
                let value_local = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return;
                }
                let value = self.operand_for_target_ty(
                    span,
                    Operand::Local(value_local),
                    contract.value_ty,
                );
                self.push_stmt(
                    span,
                    StatementKind::StoreMember {
                        receiver: Operand::Local(receiver_local),
                        member: self.assign_place_member_metadata(
                            member_name,
                            *receiver_ty,
                            resolved.as_ref(),
                        ),
                        value,
                        value_ty: contract.value_ty,
                        continuation_route: self.extract_stored_continuation_route(rhs),
                    },
                );
            }
        }
    }

    pub(in crate::mir::lower) fn assign_place_member_metadata(
        &self,
        member_name: &str,
        receiver_ty: TypeId,
        resolved: Option<&hir::MemberRef>,
    ) -> MemberAccessMetadata {
        let resolved = resolved.map(|resolved| match resolved {
            hir::MemberRef::Value { fqn, .. } => MemberTarget::Value { fqn: fqn.clone() },
            hir::MemberRef::Fun { fqn, .. } => MemberTarget::Fun { fqn: fqn.clone() },
            hir::MemberRef::ExtensionValue { fqn, .. } => {
                MemberTarget::ExtensionValue { fqn: fqn.clone() }
            }
            hir::MemberRef::ExtensionFun { fqn, .. } => {
                MemberTarget::ExtensionFun { fqn: fqn.clone() }
            }
        });
        let hidden_effects = match &resolved {
            Some(MemberTarget::Value { fqn }) => self.facts.object_member_hidden_effects(fqn),
            _ => EffectRow::pure(),
        };
        MemberAccessMetadata {
            name: member_name.to_string(),
            receiver_ty,
            resolved,
            hidden_effects,
        }
    }

    pub(in crate::mir::lower) fn extract_stored_continuation_route(
        &self,
        expr: &hir::Expr,
    ) -> StoredContinuationRoutePublication {
        match self.try_extract_stored_continuation_route(expr) {
            Ok(Some(route)) => StoredContinuationRoutePublication::Unique(route),
            Ok(None) => StoredContinuationRoutePublication::None,
            Err(StoredContinuationRouteError::Ambiguous) => {
                StoredContinuationRoutePublication::Ambiguous
            }
            Err(StoredContinuationRouteError::MissingSourceLocal) => {
                StoredContinuationRoutePublication::None
            }
        }
    }

    pub(in crate::mir::lower) fn try_extract_stored_continuation_route(
        &self,
        expr: &hir::Expr,
    ) -> Result<Option<StoredContinuationValueRoute>, StoredContinuationRouteError> {
        if continuation_contract_from_type(self.types, expr.ty).is_some() {
            match &expr.kind {
                hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                    let Some(local) = self.symbol_locals.get(id).copied() else {
                        return Err(StoredContinuationRouteError::MissingSourceLocal);
                    };
                    let source_ty = self.body.locals[local.as_u32() as usize].ty;
                    return Ok(Some(StoredContinuationValueRoute {
                        source_local: local,
                        source_ty,
                        path: Vec::new(),
                    }));
                }
                hir::ExprKind::Call { args, .. } => {
                    if let Some(binding) = self
                        .facts
                        .top_level_fun_call_binding(self.source_path.as_path(), expr.span)
                        && let Some(param_index) =
                            self.facts.continuation_identity_return_param(&binding.fqn)
                        && let Some(arg) = args.get(param_index)
                    {
                        let arg_expr = match arg {
                            hir::CallArg::Positional(value) => value,
                            hir::CallArg::Named { value, .. } => value,
                        };
                        return self.try_extract_stored_continuation_route(arg_expr);
                    }
                    return Ok(None);
                }
                _ => return Ok(None),
            }
        }

        let hir::ExprKind::Call { callee, args } = &expr.kind else {
            return Ok(None);
        };
        let hir::ExprKind::UnresolvedIdent { name } = &callee.kind else {
            return Ok(None);
        };

        let mut found: Option<(usize, StoredContinuationValueRoute)> = None;
        for (field_index, arg) in args.iter().enumerate() {
            let arg_expr = match arg {
                hir::CallArg::Positional(expr) => expr,
                hir::CallArg::Named { value, .. } => value,
            };
            let Some(mut route) = self.try_extract_stored_continuation_route(arg_expr)? else {
                continue;
            };
            if found.is_some() {
                return Err(StoredContinuationRouteError::Ambiguous);
            }
            route.path.insert(
                0,
                PatternBindingStep::VariantField {
                    variant: name.clone(),
                    field_index,
                },
            );
            found = Some((field_index, route));
        }

        Ok(found.map(|(_, route)| route))
    }

    /// 把一个 HIR 表达式降为“产生值的 local”，并返回该 local。
    ///
    /// 说明：当前阶段优先保证 CFG 形态正确，因此表达式求值本身常以 `Todo` 占位。
    pub(in crate::mir::lower) fn lower_expr_to_local(&mut self, expr: &hir::Expr) -> LocalId {
        match &expr.kind {
            hir::ExprKind::Missing => {
                let tmp = self.push_temp_local(expr.span, expr.ty);
                self.assign(expr.span, tmp, Rvalue::Todo("missing expr"));
                tmp
            }
            hir::ExprKind::UnresolvedIdent { name } => {
                self.lower_unresolved_ident(expr.span, expr.ty, name)
            }
            hir::ExprKind::Todo(kind) => {
                let tmp = self.push_temp_local(expr.span, expr.ty);
                self.assign(expr.span, tmp, Rvalue::Todo(kind));
                tmp
            }
            hir::ExprKind::Literal(lit) => self.lower_literal(expr.span, expr.ty, lit),
            hir::ExprKind::ClassLiteral(class_lit) => {
                self.lower_class_literal_expr(expr.span, expr.ty, class_lit)
            }
            hir::ExprKind::VarRef(v) => self.lower_var_ref(expr.span, expr.ty, v),
            hir::ExprKind::StructLit { fields, .. } => {
                self.lower_struct_lit_expr(expr.span, expr.ty, fields)
            }
            hir::ExprKind::TupleLit { elements } => {
                self.lower_tuple_lit_expr(expr.span, expr.ty, elements)
            }
            hir::ExprKind::InterpolatedString { raw, parts } => {
                self.lower_interpolated_string_expr(expr.span, expr.ty, *raw, parts)
            }
            hir::ExprKind::Unary {
                op, expr: operand, ..
            } => self.lower_unary_expr(expr.span, expr.ty, *op, operand),
            hir::ExprKind::Binary { lhs, op, rhs, .. } => {
                self.lower_binary_expr(expr.span, expr.ty, lhs, *op, rhs)
            }
            hir::ExprKind::TypeCheck {
                expr: value,
                op,
                target_ty: test_ty,
                ..
            } => self.lower_type_check_expr(expr.span, expr.ty, value, *op, *test_ty),
            hir::ExprKind::Cast {
                expr: value,
                op,
                target_ty,
                ..
            } => self.lower_cast_expr(expr.span, expr.ty, value, *op, *target_ty),
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
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.lower_member_access_expr(expr.span, expr.ty, receiver, member)
            }
            hir::ExprKind::Call { callee, args } => {
                self.lower_call_expr(expr.span, expr.ty, callee, args)
            }
            hir::ExprKind::Perform {
                effect_ty,
                op,
                args,
            } => self.lower_perform_expr(expr.span, expr.ty, *effect_ty, op, args),
            hir::ExprKind::Handle(handle) => self.lower_handle_expr(expr.span, expr.ty, handle),
        }
    }
}
