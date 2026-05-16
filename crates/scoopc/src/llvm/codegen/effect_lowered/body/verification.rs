//! Body / state-graph / boundary contract verification. Every published artifact (state graph, boundary, frame layout, completion payload binding) is checked against the late-lowered contract before any LLVM IR is emitted.

use super::*;

impl<'cg, 'a, 'ctx> RefactorCallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn verify_body_contract(&self) -> Result<(), LlvmEmitError> {
        self.verify_state_graph_contract()?;
        self.verify_frame_contract()?;
        self.verify_boundary_contracts()?;
        Ok(())
    }

    pub(super) fn verify_state_graph_contract(&self) -> Result<(), LlvmEmitError> {
        if self.callable.state_graph().states().is_empty() {
            return Err(frontend_error(format!(
                "refactor body verifier 发现 callable `{}` 没有 state graph body",
                self.callable.root_fqn()
            )));
        }
        self.verify_state_exists(self.callable.state_graph().entry_state(), "entry")?;
        self.verify_state_exists(self.callable.state_graph().complete_state(), "complete")?;
        if let Some(cleanup_state) = self.callable.state_graph().cleanup_state() {
            self.verify_state_exists(cleanup_state, "cleanup")?;
        }
        if let Some(drop_state) = self.callable.state_graph().drop_state() {
            self.verify_state_exists(drop_state, "drop")?;
        }

        let mut seen_states = BTreeSet::new();
        for state in self.callable.state_graph().states() {
            if !seen_states.insert(state.state_id()) {
                return Err(frontend_error(format!(
                    "refactor body verifier 发现 callable `{}` 重复发布 state st{}",
                    self.callable.root_fqn(),
                    state.state_id().as_u32()
                )));
            }
            self.verify_state_exists(state.state_id(), "state block")?;
            for successor in state.successors() {
                self.verify_state_exists(*successor, "state successor")?;
            }
            self.verify_state_source_slices(state)?;
            self.verify_state_terminator_contract(state)?;
        }
        Ok(())
    }

    pub(super) fn verify_state_source_slices(
        &self,
        state: &LateLoweredState,
    ) -> Result<(), LlvmEmitError> {
        for slice in state.source_slices() {
            let block = self
                .body
                .blocks
                .get(slice.block_id().as_u32() as usize)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor body verifier 发现 callable `{}` state st{} source slice 指向缺失 block bb{}",
                        self.callable.root_fqn(),
                        state.state_id().as_u32(),
                        slice.block_id().as_u32()
                    ))
                })?;
            if slice.start_statement_index() > slice.end_statement_index()
                || slice.end_statement_index() as usize > block.stmts.len()
            {
                return Err(frontend_error(format!(
                    "refactor body verifier 发现 callable `{}` state st{} source slice [{}..{}) 越界于 bb{}（stmt_count={}）",
                    self.callable.root_fqn(),
                    state.state_id().as_u32(),
                    slice.start_statement_index(),
                    slice.end_statement_index(),
                    slice.block_id().as_u32(),
                    block.stmts.len()
                )));
            }
            for stmt_index in slice.start_statement_index()..slice.end_statement_index() {
                let classification = self
                    .callable
                    .source_statement_classification(*slice, stmt_index)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor body verifier 发现 callable `{}` state st{} source-slice bb{} stmt{} 缺少 classification",
                            self.callable.root_fqn(),
                            state.state_id().as_u32(),
                            slice.block_id().as_u32(),
                            stmt_index
                        ))
                    })?;
                self.verify_source_statement_classification(classification.kind())?;
            }
        }
        Ok(())
    }

    pub(super) fn verify_source_statement_classification(
        &self,
        kind: LateLoweredSourceStatementClassificationKind,
    ) -> Result<(), LlvmEmitError> {
        match kind {
            LateLoweredSourceStatementClassificationKind::EffectNeutralValue
            | LateLoweredSourceStatementClassificationKind::ElidedUnreachable => Ok(()),
            LateLoweredSourceStatementClassificationKind::Unsupported { reason } => {
                Err(frontend_error(format!(
                    "refactor body verifier 发现 source statement classified unsupported: {reason}；unsupported classification 必须在 late-lowered handoff 前被拒绝或改写为 explicit elide/skip contract"
                )))
            }
            LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor {
                boundary_id,
            } => self
                .verify_boundary_exists(boundary_id, "statement classification anchor")
                .map(|_| ()),
            LateLoweredSourceStatementClassificationKind::ResumePayloadInjection {
                boundary_id,
                resume_state,
                consumer_local,
            } => {
                self.verify_state_exists(resume_state, "resume payload classification state")?;
                self.verify_local_exists(consumer_local, "resume payload classification local")?;
                let binding = self
                    .callable
                    .frame_schema()
                    .resume_payload_binding(boundary_id)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor body verifier 发现 boundary bd{} resume payload classification 缺少 frame binding",
                            boundary_id.as_u32()
                        ))
                    })?;
                if binding.resume_state() != resume_state
                    || binding.consumer_local() != consumer_local
                {
                    return Err(frontend_error(format!(
                        "refactor body verifier 发现 boundary bd{} resume payload classification 与 frame binding 漂移",
                        boundary_id.as_u32()
                    )));
                }
                Ok(())
            }
            LateLoweredSourceStatementClassificationKind::BoundaryResultInjection {
                boundary_id,
                resume_state,
                result_local,
            } => {
                self.verify_boundary_exists(boundary_id, "boundary result classification")?;
                self.verify_state_exists(resume_state, "boundary result classification state")?;
                self.verify_local_exists(result_local, "boundary result classification local")
            }
            LateLoweredSourceStatementClassificationKind::CompletionPayloadInjection {
                return_state,
                complete_state,
            } => {
                self.verify_state_exists(return_state, "completion payload classification return")?;
                self.verify_state_exists(
                    complete_state,
                    "completion payload classification complete",
                )
            }
            LateLoweredSourceStatementClassificationKind::HandleSyntheticCarrierBinder {
                state_id,
                ..
            } => self.verify_state_exists(state_id, "handle synthetic carrier binder state"),
        }
    }

    pub(super) fn verify_state_terminator_contract(
        &self,
        state: &LateLoweredState,
    ) -> Result<(), LlvmEmitError> {
        match state.terminator() {
            LateLoweredStateTerminator::Goto { target } => {
                self.verify_state_exists(*target, "goto target")
            }
            LateLoweredStateTerminator::Branch {
                cond_local,
                then_state,
                else_state,
            } => {
                self.verify_local_exists(*cond_local, "branch condition local")?;
                self.verify_state_exists(*then_state, "branch then target")?;
                self.verify_state_exists(*else_state, "branch else target")
            }
            LateLoweredStateTerminator::Return { payload_source, .. } => {
                let binding = self
                    .abi
                    .completion_payload_binding_for_state(self.abi_step_schema, state.state_id())?;
                self.abi
                    .completion_payload_binding_layout(self.abi_step_schema, binding.binding())?;
                if binding.payload_source() != payload_source
                    && !self.completion_payload_binding_feeds_return(
                        state,
                        binding.payload_source(),
                        payload_source,
                    )
                {
                    return Err(frontend_error(format!(
                        "refactor body verifier 发现 callable `{}` abi schema s{} return state st{} completion payload source 漂移：terminator={:?} binding={:?}",
                        self.callable.root_fqn(),
                        self.abi_step_schema.as_u32(),
                        state.state_id().as_u32(),
                        payload_source,
                        binding.payload_source()
                    )));
                }
                self.verify_completion_payload_source(payload_source)
            }
            LateLoweredStateTerminator::Suspend {
                boundary_ids,
                resume_state,
                local_runtime_error_states,
                cleanup_state,
                drop_state,
            } => {
                self.verify_state_exists(*resume_state, "suspend resume state")?;
                for boundary_id in boundary_ids {
                    let boundary = self.verify_boundary_exists(*boundary_id, "suspend boundary")?;
                    if boundary.owner_state() != state.state_id() {
                        return Err(frontend_error(format!(
                            "refactor body verifier 发现 suspend state st{} 引用 boundary bd{}，但 boundary owner 是 st{}",
                            state.state_id().as_u32(),
                            boundary.boundary_id().as_u32(),
                            boundary.owner_state().as_u32()
                        )));
                    }
                    if boundary.resume_state() != *resume_state {
                        return Err(frontend_error(format!(
                            "refactor body verifier 发现 suspend state st{} boundary bd{} resume state 漂移：terminator=st{} boundary=st{}",
                            state.state_id().as_u32(),
                            boundary.boundary_id().as_u32(),
                            resume_state.as_u32(),
                            boundary.resume_state().as_u32()
                        )));
                    }
                }
                for local_state in local_runtime_error_states {
                    self.verify_state_exists(*local_state, "local runtime-error target")?;
                }
                self.verify_suspend_primary_boundary_contract(state, boundary_ids)?;
                if let Some(cleanup_state) = cleanup_state {
                    self.verify_state_exists(*cleanup_state, "suspend cleanup state")?;
                }
                if let Some(drop_state) = drop_state {
                    self.verify_state_exists(*drop_state, "suspend drop state")?;
                }
                Ok(())
            }
            LateLoweredStateTerminator::HandleDispatch {
                site_id,
                body_state,
                arm_states,
                finally_state,
                exit_state,
                drop_state,
                contract,
                boundary_ids,
            } => {
                self.abi
                    .handle_dispatch_layout(self.abi_step_schema, *site_id, contract)?;
                self.verify_state_exists(*body_state, "handle body state")?;
                for arm_state in arm_states {
                    self.verify_state_exists(*arm_state, "handle arm state")?;
                }
                if let Some(finally_state) = finally_state {
                    self.verify_state_exists(*finally_state, "handle finally state")?;
                }
                self.verify_state_exists(*exit_state, "handle exit state")?;
                if let Some(drop_state) = drop_state {
                    self.verify_state_exists(*drop_state, "handle drop state")?;
                }
                for boundary_id in boundary_ids {
                    self.verify_boundary_exists(*boundary_id, "handle boundary")?;
                }
                Ok(())
            }
            LateLoweredStateTerminator::LocalRuntimeError {
                payload_tuple_ty,
                terminal_action,
            } => {
                let runtime = self.local_runtime_error_runtime_for_target_state(
                    state.state_id(),
                    *payload_tuple_ty,
                    *terminal_action,
                )?;
                if runtime.target_state != state.state_id() {
                    return Err(frontend_error(format!(
                        "refactor body verifier 发现 LocalRuntimeError st{} target contract 漂移为 st{}",
                        state.state_id().as_u32(),
                        runtime.target_state.as_u32()
                    )));
                }
                Ok(())
            }
            LateLoweredStateTerminator::ResumeUnwind => self.verify_resume_unwind_contract(state),
            LateLoweredStateTerminator::Unreachable => Ok(()),
            LateLoweredStateTerminator::Abandon => self.verify_abandon_contract(state),
        }
    }

    pub(super) fn completion_payload_binding_feeds_return(
        &self,
        state: &LateLoweredState,
        binding_source: &LateLoweredCompletionPayloadSource,
        return_source: &LateLoweredCompletionPayloadSource,
    ) -> bool {
        let Some((binding_local, return_local)) =
            completion_payload_local_pair(binding_source, return_source)
        else {
            return false;
        };
        for &slice in state.source_slices() {
            let Some(block) = self.body.blocks.get(slice.block_id().as_u32() as usize) else {
                continue;
            };
            let start = slice.start_statement_index() as usize;
            let end = slice.end_statement_index() as usize;
            for stmt in &block.stmts[start..end] {
                if matches!(
                    &stmt.kind,
                    mir::StatementKind::Assign {
                        target,
                        value: mir::Rvalue::Use(mir::Operand::Local(source)),
                    } if *target == return_local && *source == binding_local
                ) {
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn verify_suspend_primary_boundary_contract(
        &self,
        state: &LateLoweredState,
        boundary_ids: &[BoundaryId],
    ) -> Result<(), LlvmEmitError> {
        let mut primary_count = 0usize;
        let mut runtime_count = 0usize;
        for boundary_id in boundary_ids {
            let boundary = self.verify_boundary_exists(*boundary_id, "suspend primary boundary")?;
            match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::RuntimeError(_)) => runtime_count += 1,
                Some(_) => primary_count += 1,
                None => {
                    return Err(frontend_error(format!(
                        "refactor suspend state st{} boundary bd{} 缺少 published lowering",
                        state.state_id().as_u32(),
                        boundary_id.as_u32()
                    )));
                }
            }
        }
        if primary_count > 1 || (primary_count == 0 && runtime_count > 1) {
            return Err(frontend_error(format!(
                "refactor suspend state st{} 发布了多义 primary boundary：non_runtime={} runtime_error={}，backend 不能用 find() 静默选择",
                state.state_id().as_u32(),
                primary_count,
                runtime_count
            )));
        }
        Ok(())
    }

    pub(super) fn verify_resume_unwind_contract(
        &self,
        state: &LateLoweredState,
    ) -> Result<(), LlvmEmitError> {
        if state.role() != LateLoweredStateRole::Cleanup || !state.successors().is_empty() {
            return Err(frontend_error(format!(
                "refactor ResumeUnwind state st{} 不是 terminal cleanup state，缺少 published unwind payload / cleanup continuation contract",
                state.state_id().as_u32()
            )));
        }
        self.verify_resume_unwind_source(state)?;
        let origin = self.resume_unwind_cleanup_origin(state.state_id()).ok_or_else(|| {
            frontend_error(format!(
                "refactor ResumeUnwind state st{} 未由 Suspend cleanup_state 的 published cleanup continuation route 到达，不能作为普通 CFG placeholder",
                state.state_id().as_u32()
            ))
        })?;
        self.verify_resume_unwind_handle_contract(state.state_id(), origin)
    }

    pub(super) fn verify_resume_unwind_source(
        &self,
        state: &LateLoweredState,
    ) -> Result<(), LlvmEmitError> {
        if state.source_slices().is_empty() {
            return Err(frontend_error(format!(
                "refactor ResumeUnwind state st{} 缺少 canonical MIR cleanup source slice",
                state.state_id().as_u32()
            )));
        }
        for slice in state.source_slices() {
            let block = self
                .body
                .blocks
                .get(slice.block_id().as_u32() as usize)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor ResumeUnwind state st{} source slice 指向缺失 block bb{}",
                        state.state_id().as_u32(),
                        slice.block_id().as_u32()
                    ))
                })?;
            if !block.is_cleanup || !slice.includes_terminator() {
                return Err(frontend_error(format!(
                    "refactor ResumeUnwind state st{} source slice bb{} 未发布 cleanup terminator contract",
                    state.state_id().as_u32(),
                    slice.block_id().as_u32()
                )));
            }
            if !matches!(block.terminator.kind, mir::TerminatorKind::ResumeUnwind) {
                return Err(frontend_error(format!(
                    "refactor ResumeUnwind state st{} source slice bb{} terminator 不是 canonical MIR ResumeUnwind",
                    state.state_id().as_u32(),
                    slice.block_id().as_u32()
                )));
            }
        }
        Ok(())
    }

    pub(super) fn resume_unwind_cleanup_origin(
        &self,
        resume_unwind_state: StateId,
    ) -> Option<RefactorResumeUnwindOrigin<'_>> {
        let mut found = None;
        for state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::Suspend {
                boundary_ids,
                resume_state,
                cleanup_state: Some(cleanup_state),
                ..
            } = state.terminator()
            else {
                continue;
            };
            if !self.cleanup_route_reaches_resume_unwind(*cleanup_state, resume_unwind_state) {
                continue;
            }
            let origin = RefactorResumeUnwindOrigin {
                suspend_state: state.state_id(),
                cleanup_state: *cleanup_state,
                resume_state: *resume_state,
                boundary_ids,
            };
            if found.replace(origin).is_some() {
                return None;
            }
        }
        found
    }

    pub(super) fn cleanup_route_reaches_resume_unwind(
        &self,
        start: StateId,
        target: StateId,
    ) -> bool {
        let mut current = start;
        let mut seen = BTreeSet::new();
        loop {
            if current == target {
                return true;
            }
            if !seen.insert(current) {
                return false;
            }
            let Some(state) = self.callable.state_graph().state(current) else {
                return false;
            };
            if state.role() != LateLoweredStateRole::Cleanup {
                return false;
            }
            match state.terminator() {
                LateLoweredStateTerminator::Goto { target } => current = *target,
                LateLoweredStateTerminator::ResumeUnwind => return false,
                _ => return false,
            }
        }
    }

    pub(super) fn verify_resume_unwind_handle_contract(
        &self,
        state_id: StateId,
        origin: RefactorResumeUnwindOrigin<'_>,
    ) -> Result<(), LlvmEmitError> {
        self.verify_state_exists(origin.cleanup_state, "ResumeUnwind cleanup route start")?;
        if origin.boundary_ids.is_empty() {
            return Err(frontend_error(format!(
                "refactor ResumeUnwind state st{} 的 cleanup continuation 来自 st{}，但 Suspend 缺少 boundary ids",
                state_id.as_u32(),
                origin.suspend_state.as_u32()
            )));
        }
        for boundary_id in origin.boundary_ids {
            let boundary = self.verify_boundary_exists(*boundary_id, "ResumeUnwind boundary")?;
            if boundary.owner_state() != origin.suspend_state
                || boundary.resume_state() != origin.resume_state
            {
                return Err(frontend_error(format!(
                    "refactor ResumeUnwind state st{} boundary bd{} 的 origin/resume-state contract 漂移：origin=st{} resume=st{} boundary_owner=st{} boundary_resume=st{}",
                    state_id.as_u32(),
                    boundary_id.as_u32(),
                    origin.suspend_state.as_u32(),
                    origin.resume_state.as_u32(),
                    boundary.owner_state().as_u32(),
                    boundary.resume_state().as_u32(),
                )));
            }
        }

        let mut matched_handle = None::<(usize, SiteId)>;
        for handle_state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } = handle_state.terminator()
            else {
                continue;
            };
            if matches!(
                contract.state_region(origin.suspend_state),
                LateLoweredHandleStateRegion::OutsideHandle
            ) {
                continue;
            }
            if contract.finally_complete_target().is_none() || !contract.needs_completion_state() {
                continue;
            }
            let layout =
                self.abi
                    .handle_dispatch_layout(self.abi_step_schema, *site_id, contract)?;
            let _ = layout
                .completion_tag_value(LateLoweredHandlePendingCompletion::ContinueToExit)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor ResumeUnwind state st{} HandleDispatch site{} 缺少 ContinueToExit completion tag",
                        state_id.as_u32(),
                        site_id.as_u32()
                    ))
                })?;
            let _ = layout
                .completion_tag_value(LateLoweredHandlePendingCompletion::ReturnFromFunction)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor ResumeUnwind state st{} HandleDispatch site{} 缺少 ReturnFromFunction completion tag",
                        state_id.as_u32(),
                        site_id.as_u32()
                    ))
                })?;
            for origin in contract.pending_completion_origins() {
                if let Some(transport) = contract.pending_payload_transport(origin.completion()) {
                    let _ = layout
                        .pending_payload_transport_layout(transport.completion())
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "refactor ResumeUnwind state st{} HandleDispatch site{} pending payload transport {:?} 缺少 ABI layout",
                                state_id.as_u32(),
                                site_id.as_u32(),
                                transport.completion()
                            ))
                        })?;
                }
                let _ = layout.pending_completion_origin_tag_value(*origin).ok_or_else(|| {
                    frontend_error(format!(
                        "refactor ResumeUnwind state st{} HandleDispatch site{} pending origin {:?} 缺少 completion tag",
                        state_id.as_u32(),
                        site_id.as_u32(),
                        origin
                    ))
                })?;
            }
            let depth = self.handle_dispatch_nesting_depth(handle_state.state_id());
            match matched_handle {
                None => matched_handle = Some((depth, *site_id)),
                Some((matched_depth, _)) if depth > matched_depth => {
                    matched_handle = Some((depth, *site_id));
                }
                Some((matched_depth, _)) if depth < matched_depth => {}
                Some(_) => {
                    return Err(frontend_error(format!(
                        "refactor ResumeUnwind state st{} 命中多个同层 HandleDispatch cleanup/unwind contract",
                        state_id.as_u32()
                    )));
                }
            }
        }

        matched_handle.map(|_| ()).ok_or_else(|| {
            frontend_error(format!(
                "refactor ResumeUnwind state st{} 缺少 enclosing HandleDispatch pending completion contract",
                state_id.as_u32()
            ))
        })
    }

    pub(super) fn verify_abandon_contract(
        &self,
        state: &LateLoweredState,
    ) -> Result<(), LlvmEmitError> {
        if self.callable.state_graph().drop_state() != Some(state.state_id())
            || state.role() != LateLoweredStateRole::Drop
            || !state.source_slices().is_empty()
        {
            return Err(frontend_error(format!(
                "refactor Abandon state st{} 只能作为 published drop_state 的空 Drop state 终止，不能作为普通 CFG fallback",
                state.state_id().as_u32()
            )));
        }
        Ok(())
    }

    pub(super) fn verify_frame_contract(&self) -> Result<(), LlvmEmitError> {
        if self.frame_layout.step_schema() != self.abi_step_schema {
            return Err(frontend_error(format!(
                "refactor body verifier 发现 frame layout step schema 漂移：layout=s{} abi=s{}",
                self.frame_layout.step_schema().as_u32(),
                self.abi_step_schema.as_u32()
            )));
        }
        for slot in self.callable.frame_schema().slots() {
            if self
                .frame_layout
                .field_index_for_slot(slot.slot_id())
                .is_none()
            {
                return Err(frontend_error(format!(
                    "refactor body verifier 发现 frame slot fs{} 缺少 ABI field layout",
                    slot.slot_id().as_u32()
                )));
            }
            if let Some(local) = frame_slot_local(slot.kind()) {
                self.verify_local_exists(local, "frame slot local")?;
            }
            for write_state in slot.write_points() {
                self.verify_state_exists(*write_state, "frame slot write point")?;
            }
            for read_state in slot.read_points() {
                self.verify_state_exists(*read_state, "frame slot read point")?;
            }
        }
        for binding in self.callable.frame_schema().resume_payload_bindings() {
            self.abi
                .resume_payload_binding_layout(self.abi_step_schema, binding)?;
            self.verify_boundary_exists(binding.boundary_id(), "resume payload binding boundary")?;
            self.verify_state_exists(binding.resume_state(), "resume payload binding state")?;
            self.verify_local_exists(binding.consumer_local(), "resume payload binding local")?;
            if let Some(frame_slot) = binding.consumer_frame_slot()
                && self.frame_layout.field_index_for_slot(frame_slot).is_none()
            {
                return Err(frontend_error(format!(
                    "refactor body verifier 发现 resume payload binding bd{} 的 frame slot fs{} 缺少 ABI field layout",
                    binding.boundary_id().as_u32(),
                    frame_slot.as_u32()
                )));
            }
        }
        for binding in self.callable.frame_schema().completion_payload_bindings() {
            let published = self.abi.completion_payload_binding_for_state(
                self.abi_step_schema,
                binding.return_state(),
            )?;
            self.abi
                .completion_payload_binding_layout(self.abi_step_schema, published.binding())?;
            if published.binding() != binding {
                let state = self
                    .callable
                    .state_graph()
                    .state(binding.return_state())
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor body verifier 发现 completion payload binding return state st{} 不存在",
                            binding.return_state().as_u32()
                        ))
                    })?;
                if !self.completion_payload_binding_feeds_return(
                    state,
                    published.binding().payload_source(),
                    binding.payload_source(),
                ) {
                    return Err(frontend_error(format!(
                        "refactor body verifier 发现 callable `{}` return state st{} completion payload binding 与 ABI contract 漂移：published={:?} binding={:?}",
                        self.callable.root_fqn(),
                        binding.return_state().as_u32(),
                        published.binding(),
                        binding,
                    )));
                }
            }
            self.verify_state_exists(binding.return_state(), "completion payload return state")?;
            self.verify_completion_payload_source(binding.payload_source())?;
        }
        Ok(())
    }

    pub(super) fn verify_boundary_contracts(&self) -> Result<(), LlvmEmitError> {
        let mut seen_boundaries = BTreeSet::new();
        for boundary in self.callable.boundary_map().entries() {
            if !seen_boundaries.insert(boundary.boundary_id()) {
                return Err(frontend_error(format!(
                    "refactor body verifier 发现 callable `{}` 重复发布 boundary bd{}",
                    self.callable.root_fqn(),
                    boundary.boundary_id().as_u32()
                )));
            }
            self.verify_state_exists(boundary.owner_state(), "boundary owner state")?;
            self.verify_state_exists(boundary.resume_state(), "boundary resume state")?;
            let lowering = boundary.lowering().ok_or_else(|| {
                frontend_error(format!(
                    "refactor body verifier 发现 boundary bd{} 缺少 published lowering",
                    boundary.boundary_id().as_u32()
                ))
            })?;
            self.verify_boundary_source_consumption(boundary)?;
            match lowering {
                LateLoweredBoundaryLowering::Call(lowering) => {
                    let source = boundary_site(boundary, "Call")?;
                    self.abi.call_boundary_operand_layout(
                        self.abi_step_schema,
                        source,
                        lowering.operand_contract(),
                    )?;
                    self.abi
                        .call_target_layout(self.abi_step_schema, source, lowering.facts())?;
                    if let Some(carrier) = lowering.operand_contract().carrier_source() {
                        self.verify_operand_source(carrier)?;
                    }
                    for arg in lowering.operand_contract().arg_sources() {
                        self.verify_operand_source(arg)?;
                    }
                    if let Some(contract) = lowering.consumed_runtime_error_case() {
                        let runtime =
                            self.local_runtime_error_runtime_for_call(source, contract)?;
                        self.verify_state_exists(
                            runtime.target_state,
                            "call local runtime-error target",
                        )?;
                    }
                }
                LateLoweredBoundaryLowering::ClassCtor(lowering) => {
                    let _source = boundary_site(boundary, "ClassCtor")?;
                    self.verify_local_exists(lowering.result_local(), "class ctor result local")?;
                    for emission in lowering.emitted_steps() {
                        self.verify_step_case_payload_contract(
                            emission.case_tag(),
                            emission.payload_tuple_ty(),
                        )?;
                    }
                }
                LateLoweredBoundaryLowering::Perform(lowering) => {
                    let source = boundary_site(boundary, "Perform")?;
                    self.abi.perform_boundary_operand_layout(
                        self.abi_step_schema,
                        source,
                        lowering.operand_contract(),
                    )?;
                    for payload_source in lowering.operand_contract().payload_sources() {
                        self.verify_operand_source(payload_source)?;
                    }
                    self.verify_step_case_payload_contract(
                        lowering.emitted_step().case_tag(),
                        lowering.emitted_step().payload_tuple_ty(),
                    )?;
                }
                LateLoweredBoundaryLowering::Resume(lowering) => {
                    let source = boundary_site(boundary, "Resume")?;
                    self.abi.resume_boundary_operand_layout(
                        self.abi_step_schema,
                        source,
                        lowering.operand_contract(),
                    )?;
                    self.verify_operand_source(lowering.operand_contract().continuation_source())?;
                    for arg in lowering.operand_contract().arg_sources() {
                        self.verify_operand_source(arg)?;
                    }
                    let surface = self
                        .abi
                        .surface_resume_layout(lowering.facts().continuation_schema())
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "refactor body verifier 缺少 continuation schema k{} 的 surface resume ABI",
                                lowering.facts().continuation_schema().as_u32()
                            ))
                        })?;
                    self.verify_source_value_layout(
                        surface.resume_tuple_ty(),
                        "surface resume tuple",
                    )?;
                }
                LateLoweredBoundaryLowering::RuntimeError(lowering) => {
                    self.verify_step_case_payload_contract(
                        lowering.emitted_step().case_tag(),
                        lowering.emitted_step().payload_tuple_ty(),
                    )?;
                }
                LateLoweredBoundaryLowering::Handle(lowering) => {
                    for emission in lowering.outward_emissions() {
                        self.verify_step_case_payload_contract(
                            emission.case_tag(),
                            emission.payload_tuple_ty(),
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    pub(super) fn verify_boundary_source_consumption(
        &self,
        boundary: &LateLoweredBoundary,
    ) -> Result<(), LlvmEmitError> {
        let Some(consumption) = boundary_source_consumption(boundary) else {
            return Ok(());
        };
        match consumption {
            LateLoweredBoundarySourceConsumption::Statement {
                source_slice,
                statement_index,
                ..
            } => {
                let classification = self
                    .callable
                    .source_statement_classification(source_slice, statement_index)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor body verifier 发现 boundary bd{} statement anchor bb{} stmt{} 缺少 classification",
                            boundary.boundary_id().as_u32(),
                            source_slice.block_id().as_u32(),
                            statement_index
                        ))
                    })?;
                match classification.kind() {
                    LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor {
                        boundary_id,
                    } if boundary_id == boundary.boundary_id() => Ok(()),
                    other => Err(frontend_error(format!(
                        "refactor body verifier 发现 boundary bd{} statement anchor classification 漂移：{:?}",
                        boundary.boundary_id().as_u32(),
                        other
                    ))),
                }
            }
            LateLoweredBoundarySourceConsumption::Terminator { source_slice } => {
                if !source_slice.includes_terminator() {
                    return Err(frontend_error(format!(
                        "refactor body verifier 发现 boundary bd{} terminator anchor 所在 source slice 没有包含 terminator",
                        boundary.boundary_id().as_u32()
                    )));
                }
                Ok(())
            }
        }
    }

    pub(super) fn verify_step_case_payload_contract(
        &self,
        case_tag: CaseTag,
        payload_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        self.step_layout.case_layout(case_tag).ok_or_else(|| {
            frontend_error(format!(
                "refactor body verifier 发现 step schema s{} 缺少 case c{} layout",
                self.abi_step_schema.as_u32(),
                case_tag.as_u32()
            ))
        })?;
        self.verify_source_value_layout(payload_ty, "step case payload")?;
        Ok(())
    }

    pub(super) fn verify_completion_payload_source(
        &self,
        source: &LateLoweredCompletionPayloadSource,
    ) -> Result<(), LlvmEmitError> {
        if source.is_unit() && self.source_ty_is_unit(source.source_ty()) {
            return Ok(());
        }
        self.verify_source_value_layout(source.source_ty(), "completion payload source")?;
        if let Some(operand) = source.operand_source() {
            self.verify_operand_source(operand)?;
        }
        Ok(())
    }

    pub(super) fn verify_operand_source(
        &self,
        source: &LateLoweredOperandSource,
    ) -> Result<(), LlvmEmitError> {
        self.verify_source_value_layout(source.source_ty(), "operand source")?;
        if let LateLoweredOperandValueSource::Local(local) = source.value() {
            self.verify_local_exists(*local, "operand source local")?;
        }
        Ok(())
    }

    pub(super) fn verify_state_exists(
        &self,
        state_id: StateId,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        if self.state_blocks.contains_key(&state_id) {
            Ok(())
        } else {
            Err(frontend_error(format!(
                "refactor body verifier 发现 {label} 引用缺失 state st{}",
                state_id.as_u32()
            )))
        }
    }

    pub(super) fn verify_boundary_exists(
        &self,
        boundary_id: BoundaryId,
        label: &str,
    ) -> Result<&LateLoweredBoundary, LlvmEmitError> {
        self.callable
            .boundary_map()
            .boundary(boundary_id)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor body verifier 发现 {label} 引用缺失 boundary bd{}",
                    boundary_id.as_u32()
                ))
            })
    }

    pub(super) fn verify_local_exists(
        &self,
        local: LocalId,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        if self.slots.get(local.as_u32() as usize).is_some() {
            Ok(())
        } else {
            Err(frontend_error(format!(
                "refactor body verifier 发现 {label} 引用缺失 local l{}",
                local.as_u32()
            )))
        }
    }

    pub(super) fn verify_source_value_layout(
        &self,
        ty: TypeId,
        label: &str,
    ) -> Result<(), LlvmEmitError> {
        if self.source_ty_is_unit(ty) {
            return Ok(());
        }
        self.abi.source_value_layout(ty).map(|_| ()).map_err(|err| {
            frontend_error(format!(
                "refactor body verifier 发现 {label} t{} 缺少 ABI value lowering contract：{err}",
                ty.as_u32()
            ))
        })
    }

    pub(super) fn source_ty_is_unit(&self, ty: TypeId) -> bool {
        matches!(
            self.source_types.kind(ty),
            TypeKind::Value(ValueTypeKind::Unit)
        )
    }

    pub(super) fn source_ty_is_runtime_error(&self, ty: TypeId) -> bool {
        matches!(
            self.source_types.kind(ty),
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                | TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.RuntimeError"
        )
    }
}
