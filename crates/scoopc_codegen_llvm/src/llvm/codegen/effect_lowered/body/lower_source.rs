//! Source-statement lowering: effect-neutral statement passes plus the published call statement entry, the per-local load/store helpers, and the local runtime-error case lookups used when a call yields a runtime error.

use super::*;

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) fn lir_statement_for_source_position(
        &self,
        owner_state: StateId,
        source_slice: crate::effect_lowered::ir::LateLoweredStateSlice,
        source_statement_index: u32,
        context: &str,
    ) -> Result<(&'a LirStatement, LirStatementIndex), LlvmEmitError> {
        let classification = self
            .callable
            .source_statement_classifications()
            .iter()
            .find(|classification| {
                classification.state_id() == owner_state
                    && classification.source_slice() == source_slice
                    && classification.statement_index() == source_statement_index
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "{context} source bb{} stmt{} 未映射到 state st{} 的 LIR statement",
                    source_slice.block_id().as_u32(),
                    source_statement_index,
                    owner_state.as_u32(),
                ))
            })?;
        let LirBodyAnchor::Statement { state, statement } = classification.anchor() else {
            return Err(frontend_error(format!(
                "{context} source bb{} stmt{} 映射到非 statement LIR anchor {:?}",
                source_slice.block_id().as_u32(),
                source_statement_index,
                classification.anchor(),
            )));
        };
        if state != owner_state {
            return Err(frontend_error(format!(
                "{context} source bb{} stmt{} LIR anchor state 漂移：classification=st{} owner=st{}",
                source_slice.block_id().as_u32(),
                source_statement_index,
                state.as_u32(),
                owner_state.as_u32(),
            )));
        }
        let state_body = self
            .callable
            .state_graph()
            .state(owner_state)
            .ok_or_else(|| {
                frontend_error(format!(
                    "{context} source bb{} stmt{} 引用缺失 owner state st{}",
                    source_slice.block_id().as_u32(),
                    source_statement_index,
                    owner_state.as_u32(),
                ))
            })?;
        let stmt = state_body
            .statements()
            .get(statement.as_u32() as usize)
            .ok_or_else(|| {
                frontend_error(format!(
                    "{context} source bb{} stmt{} 映射到越界 LIR statement{} in st{}",
                    source_slice.block_id().as_u32(),
                    source_statement_index,
                    statement.as_u32(),
                    owner_state.as_u32(),
                ))
            })?;
        Ok((stmt, statement))
    }

    pub(super) fn local_runtime_error_runtime_for_call(
        &self,
        site_id: SiteId,
        contract: &LateLoweredConsumedRuntimeErrorCase,
    ) -> Result<LocalRuntimeErrorRuntime, LlvmEmitError> {
        let published =
            self.abi
                .call_local_runtime_error_contract(self.abi_step_schema, site_id, contract)?;
        if published.owner_step_schema() != self.abi_step_schema || published.site_id() != site_id {
            return Err(frontend_error(format!(
                "body verifier 发现 local runtime-error contract identity 漂移：layout=(s{}, site={}) expected=(s{}, site={})",
                published.owner_step_schema().as_u32(),
                published.site_id().as_u32(),
                self.abi_step_schema.as_u32(),
                site_id.as_u32()
            )));
        }
        if published.payload_abi().is_elided() {
            return Err(frontend_error(format!(
                "body verifier 发现 call site {} 的 local runtime-error payload ABI 被错误 elide",
                site_id.as_u32()
            )));
        }
        let runtime_entry = match published.terminal_action() {
            LocalRuntimeErrorTerminalAction::RuntimeFatal { runtime_entry } => runtime_entry,
        };
        Ok(LocalRuntimeErrorRuntime {
            site_id,
            input_case_tag: published.input_case_tag(),
            payload_tuple_ty: published.payload_tuple_ty(),
            target_state: published.target_state(),
            runtime_symbol: runtime_entry.symbol_name().to_string(),
            runtime_param_count: runtime_entry.param_count(),
        })
    }

    pub(super) fn local_runtime_error_runtime_for_target_state(
        &self,
        target_state: StateId,
        payload_tuple_ty: TypeId,
        terminal_action: crate::effect_lowered::ir::LateLoweredLocalRuntimeErrorTerminalAction,
    ) -> Result<LocalRuntimeErrorRuntime, LlvmEmitError> {
        let mut selected = None::<LocalRuntimeErrorRuntime>;
        for boundary in self.callable.boundary_map().entries() {
            let LateLoweredBoundarySource::Site {
                site_id,
                kind: BoundarySiteKind::Call,
            } = boundary.source()
            else {
                continue;
            };
            let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering() else {
                continue;
            };
            let Some(contract) = lowering.consumed_runtime_error_case() else {
                continue;
            };
            if contract.target_state() != target_state {
                continue;
            }
            if contract.payload_tuple_ty() != payload_tuple_ty
                || contract.terminal_action() != terminal_action
            {
                return Err(frontend_error(format!(
                    "body verifier 发现 LocalRuntimeError st{} 与 call site {} consumed contract 漂移",
                    target_state.as_u32(),
                    site_id.as_u32()
                )));
            }
            let runtime = self.local_runtime_error_runtime_for_call(site_id, contract)?;
            if let Some(existing) = &selected {
                return Err(frontend_error(format!(
                    "body verifier 发现 LocalRuntimeError st{} 被多个 call site 消费：{} 与 {}",
                    target_state.as_u32(),
                    existing.site_id.as_u32(),
                    site_id.as_u32()
                )));
            }
            selected = Some(runtime);
        }
        selected.ok_or_else(|| {
            frontend_error(format!(
                "body verifier 发现 LocalRuntimeError st{} 缺少对应 consumed runtime-error case contract",
                target_state.as_u32()
            ))
        })
    }

    pub(super) fn lower_effect_neutral_statement(
        &mut self,
        stmt: &LirStatement,
    ) -> Result<(), LlvmEmitError> {
        let codegen = &mut *self.codegen;
        let program = self.program;
        let plain_call_sites = self.callable.plain_abi().map(|plain| plain.call_sites());
        let source_types = self.source_types;
        let lir_body = self.lir_body;
        let body = self.mir_fun.body.as_ref().unwrap_or_else(|| {
            panic!(
                "CallableEmitter::new validated source body for `{}`",
                self.callable.root_fqn()
            )
        });
        let slots = &self.slots;
        let abi = self.abi;
        let used_locals = &self.used_locals;
        ValuePrimitives::new(
            codegen,
            program,
            plain_call_sites,
            source_types,
            body,
            slots,
            abi,
        )
        .lower_effect_neutral_statement(stmt, lir_body, used_locals)
    }

    pub(super) fn lower_published_call_statement(
        &mut self,
        stmt: &LirStatement,
    ) -> Result<bool, LlvmEmitError> {
        let LirStatementKind::Assign {
            target,
            value:
                LirRvalue::Call {
                    site_id,
                    kind,
                    args,
                    ..
                },
        } = &stmt.kind
        else {
            return Ok(false);
        };
        if !matches!(
            kind,
            LirCallKind::Closure { .. }
                | LirCallKind::FunValue { .. }
                | LirCallKind::FunPtr { .. }
                | LirCallKind::Virtual { .. }
                | LirCallKind::Interface { .. }
        ) {
            return Ok(false);
        }
        let Some(layout) = self
            .abi
            .dynamic_invoke_layout(self.abi_step_schema, *site_id)
        else {
            return Ok(false);
        };
        let args_payload = self.pack_call_args_for_invoke(
            stmt.span,
            layout.invoke_args_tuple_ty(),
            args,
            "dynamic_call",
        )?;
        let carrier = self.lower_dynamic_call_carrier(stmt.span, kind, layout)?;
        let step = self.emit_dynamic_invoke_step(layout, carrier, args_payload)?;
        self.store_no_outward_call_complete(
            stmt.span,
            *site_id,
            layout.return_step_schema(),
            step,
            *target,
        )?;
        Ok(true)
    }

    pub(super) fn load_local_value(
        &mut self,
        span: crate::span::Span,
        local: LocalId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.value_primitives().load_local(span, local)
    }

    pub(super) fn store_local_value(
        &mut self,
        span: crate::span::Span,
        local: LocalId,
        value: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.value_primitives().store_local(span, local, value)
    }

    pub(super) fn store_loaded_raw_local(
        &mut self,
        span: crate::span::Span,
        local: LocalId,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.value_primitives()
            .store_loaded_raw_local(span, local, raw)
    }
}
