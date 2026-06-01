//! Body rewriting: per-method substitution of locals, operands, rvalues, statements, terminators, patterns, transport contracts, and direct-call/site-binding metadata. Also includes the repair_* passes that fix up result types after rewriting.

use super::*;

impl MirInstanceMaterializer {
    pub(super) fn repair_direct_call_result_types(&mut self, body: &mut Body) {
        let mut updates = Vec::new();
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                let StatementKind::Assign { target, value } = &mut stmt.kind else {
                    continue;
                };
                let Rvalue::Call {
                    kind: CallKind::Direct { callee_fqn, .. },
                    transport,
                    ..
                } = value
                else {
                    continue;
                };
                if let Some(result_ty) = self
                    .materialized_direct_call_result_tys
                    .get(callee_fqn)
                    .copied()
                {
                    if type_contains_param(&self.types, result_ty) {
                        continue;
                    }
                    transport.result.source_ty = result_ty;
                    if let Some(aggregate_return) = &mut transport.aggregate_return {
                        aggregate_return.source_ty = result_ty;
                    }
                    updates.push((*target, result_ty));
                }
            }
        }
        for (target, result_ty) in updates {
            if let Some(local) = body.locals.get_mut(target.as_u32() as usize) {
                local.ty = result_ty;
            }
        }

        let locals = body.locals.clone();
        let mut member_updates = Vec::new();
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                let StatementKind::Assign {
                    target,
                    value:
                        Rvalue::MemberAccess {
                            receiver, member, ..
                        },
                } = &mut stmt.kind
                else {
                    continue;
                };
                let receiver_ty = operand_type(&self.types, self.builtins, &locals, receiver)
                    .unwrap_or(member.receiver_ty);
                if type_contains_param(&self.types, member.receiver_ty)
                    && !type_contains_param(&self.types, receiver_ty)
                {
                    member.receiver_ty = receiver_ty;
                }
                if let Some(result_ty) = self.member_value_result_ty(receiver_ty, member) {
                    if type_contains_param(&self.types, result_ty) {
                        continue;
                    }
                    member_updates.push((*target, result_ty));
                }
            }
        }
        for (target, result_ty) in member_updates {
            if let Some(local) = body.locals.get_mut(target.as_u32() as usize) {
                local.ty = result_ty;
            }
        }
    }

    pub(super) fn repair_member_store_receiver_types(&mut self, body: &mut Body) {
        let locals = body.locals.clone();
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                let StatementKind::StoreMember {
                    receiver, member, ..
                } = &mut stmt.kind
                else {
                    continue;
                };
                let Some(receiver_ty) = operand_type(&self.types, self.builtins, &locals, receiver)
                else {
                    continue;
                };
                if receiver_ty == member.receiver_ty
                    || type_contains_param(&self.types, receiver_ty)
                {
                    continue;
                }
                if nominal_type_fqn(&self.types, receiver_ty)
                    == nominal_type_fqn(&self.types, member.receiver_ty)
                {
                    member.receiver_ty = receiver_ty;
                }
            }
        }
    }

    pub(super) fn repair_array_call_transport_types(&mut self, body: &mut Body) {
        let locals = body.locals.clone();
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                let StatementKind::Assign { target, value } = &mut stmt.kind else {
                    continue;
                };
                let Rvalue::Call {
                    args, transport, ..
                } = value
                else {
                    continue;
                };
                let Some(array) = transport.array.as_mut() else {
                    continue;
                };
                let authoritative_array_ty = match array.operation {
                    super::super::ArrayTransportOperation::Get
                    | super::super::ArrayTransportOperation::Set => args
                        .first()
                        .and_then(|arg| {
                            operand_type(&self.types, self.builtins, &locals, &arg.value)
                        })
                        .filter(|ty| !type_contains_param(&self.types, *ty)),
                    super::super::ArrayTransportOperation::BuilderBuildArray
                    | super::super::ArrayTransportOperation::BuilderBuildMutableArray => locals
                        .get(target.as_u32() as usize)
                        .map(|decl| decl.ty)
                        .filter(|ty| !type_contains_param(&self.types, *ty))
                        .or_else(|| {
                            let result_ty = transport.result.source_ty;
                            (!type_contains_param(&self.types, result_ty)).then_some(result_ty)
                        }),
                    super::super::ArrayTransportOperation::BuilderPush => args
                        .first()
                        .and_then(|arg| {
                            operand_type(&self.types, self.builtins, &locals, &arg.value)
                        })
                        .filter(|ty| !type_contains_param(&self.types, *ty)),
                    super::super::ArrayTransportOperation::BuilderNew => None,
                };
                if let Some(array_ty) = authoritative_array_ty {
                    array.array_ty = array_ty;
                }
                let element_ty = match array.operation {
                    super::super::ArrayTransportOperation::Get => locals
                        .get(target.as_u32() as usize)
                        .map(|decl| decl.ty)
                        .filter(|ty| !type_contains_param(&self.types, *ty))
                        .or_else(|| {
                            let result_ty = transport.result.source_ty;
                            (!type_contains_param(&self.types, result_ty)).then_some(result_ty)
                        })
                        .or_else(|| {
                            if type_contains_param(&self.types, array.array_ty) {
                                None
                            } else {
                                self.materialized_array_element_ty(array.array_ty)
                            }
                        }),
                    super::super::ArrayTransportOperation::Set => args
                        .last()
                        .and_then(|arg| {
                            operand_type(&self.types, self.builtins, &locals, &arg.value)
                        })
                        .filter(|ty| !type_contains_param(&self.types, *ty))
                        .or_else(|| {
                            if type_contains_param(&self.types, array.array_ty) {
                                None
                            } else {
                                self.materialized_array_element_ty(array.array_ty)
                            }
                        }),
                    super::super::ArrayTransportOperation::BuilderBuildArray
                    | super::super::ArrayTransportOperation::BuilderBuildMutableArray => {
                        if type_contains_param(&self.types, array.array_ty) {
                            None
                        } else {
                            self.materialized_array_element_ty(array.array_ty)
                        }
                    }
                    super::super::ArrayTransportOperation::BuilderPush => args
                        .get(1)
                        .and_then(|arg| {
                            operand_type(&self.types, self.builtins, &locals, &arg.value)
                        })
                        .filter(|ty| !type_contains_param(&self.types, *ty))
                        .or_else(|| {
                            if type_contains_param(&self.types, array.array_ty) {
                                None
                            } else {
                                self.materialized_array_element_ty(array.array_ty)
                            }
                        })
                        .or_else(|| {
                            if type_contains_param(&self.types, array.element_ty) {
                                None
                            } else {
                                Some(array.element_ty)
                            }
                        }),
                    super::super::ArrayTransportOperation::BuilderNew => None,
                };
                let Some(element_ty) = element_ty else {
                    continue;
                };
                array.element_ty = element_ty;
                self.refresh_value_transport_contract(
                    &mut array.element,
                    element_ty,
                    Some(array.array_ty),
                );
            }
        }
    }

    pub(super) fn repair_closure_capture_transport_targets(&mut self, body: &mut Body) {
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                let StatementKind::Assign {
                    value: Rvalue::MakeClosure { env_contract, .. },
                    ..
                } = &mut stmt.kind
                else {
                    continue;
                };
                if type_contains_param(&self.types, env_contract.env_ty) {
                    continue;
                }
                for capture in &mut env_contract.captures {
                    let source_ty = capture.transport.source_ty;
                    if type_contains_param(&self.types, source_ty) {
                        continue;
                    }
                    self.refresh_value_transport_contract(
                        &mut capture.transport,
                        source_ty,
                        Some(env_contract.env_ty),
                    );
                }
            }
        }
    }

    pub(super) fn repair_handle_payload_metadata_types(&mut self, body: &mut Body) {
        for block in &mut body.blocks {
            let TerminatorKind::Handle { arms, .. } = &mut block.terminator.kind else {
                continue;
            };
            for arm in arms {
                if arm.payload_component_tys.len() != arm.binder_count
                    || arm
                        .payload_component_tys
                        .iter()
                        .any(|ty| type_contains_param(&self.types, *ty))
                {
                    continue;
                }
                arm.payload_tuple_ty = materialized_payload_tuple_ty_from_components(
                    &mut self.types,
                    self.builtins.unit,
                    &arm.payload_component_tys,
                );
            }
        }
    }

    pub(super) fn materialized_array_element_ty(&self, array_ty: TypeId) -> Option<TypeId> {
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(array_ty) else {
            return None;
        };
        if matches!(
            nominal.fqn.as_str(),
            "scoop.core.Array"
                | "scoop.core.MutableArray"
                | "scoop.core.List"
                | "scoop.core.MutableList"
        ) {
            nominal.args.first().copied()
        } else {
            None
        }
    }

    pub(super) fn repair_materialized_generic_transport_call_args(&mut self, body: &mut Body) {
        let mut transport_sources: HashMap<LocalId, (Operand, TypeId)> = HashMap::new();
        for block in &body.blocks {
            for stmt in &block.stmts {
                let StatementKind::Assign {
                    target,
                    value: Rvalue::Transport { value, transport },
                } = &stmt.kind
                else {
                    continue;
                };
                let Some(boxing) = transport.boxing.as_ref() else {
                    continue;
                };
                if !type_contains_param(&self.types, transport.source_ty)
                    && boxing
                        .target_ty
                        .is_some_and(|ty| type_contains_param(&self.types, ty))
                {
                    transport_sources.insert(*target, (value.clone(), transport.source_ty));
                }
            }
        }

        let mut fixes: HashMap<LocalId, (Operand, TypeId)> = HashMap::new();
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                let StatementKind::Assign {
                    value: Rvalue::Call { args, .. },
                    ..
                } = &mut stmt.kind
                else {
                    continue;
                };
                for arg in args {
                    let Operand::Local(local) = &arg.value else {
                        continue;
                    };
                    let local = *local;
                    let Some((source_operand, source_ty)) = transport_sources.get(&local).cloned()
                    else {
                        continue;
                    };
                    arg.value = source_operand.clone();
                    fixes.insert(local, (source_operand, source_ty));
                }
            }
        }

        if fixes.is_empty() {
            return;
        }
        for (local, (_, source_ty)) in &fixes {
            if let Some(decl) = body.locals.get_mut(local.as_u32() as usize) {
                decl.ty = *source_ty;
            }
        }
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                let StatementKind::Assign { target, value } = &mut stmt.kind else {
                    continue;
                };
                let Some((source_operand, _)) = fixes.get(target).cloned() else {
                    continue;
                };
                *value = Rvalue::Use(source_operand);
            }
        }
    }

    pub(super) fn repair_transport_target_local_types(&mut self, body: &mut Body) {
        let mut updates = Vec::new();
        for block in &body.blocks {
            for stmt in &block.stmts {
                let StatementKind::Assign {
                    target,
                    value: Rvalue::Transport { transport, .. },
                } = &stmt.kind
                else {
                    continue;
                };
                let Some(target_ty) = transport
                    .boxing
                    .as_ref()
                    .and_then(|boxing| boxing.target_ty)
                else {
                    continue;
                };
                if type_contains_param(&self.types, target_ty) {
                    continue;
                }
                updates.push((*target, target_ty));
            }
        }
        for (target, target_ty) in updates {
            if let Some(local) = body.locals.get_mut(target.as_u32() as usize) {
                local.ty = target_ty;
            }
        }
    }

    pub(super) fn repair_perform_payload_metadata_types(&mut self, body: &mut Body) {
        for block in &mut body.blocks {
            let TerminatorKind::Perform { metadata, args, .. } = &mut block.terminator.kind else {
                continue;
            };
            if args.len() != metadata.payload_component_tys.len()
                || metadata
                    .payload_component_tys
                    .iter()
                    .any(|ty| type_contains_param(&self.types, *ty))
            {
                continue;
            }
            metadata.payload_tuple_ty = materialized_payload_tuple_ty_from_components(
                &mut self.types,
                self.builtins.unit,
                &metadata.payload_component_tys,
            );
            let payload_tuple_ty = metadata.payload_tuple_ty;
            for (transport, &component_ty) in metadata
                .payload_transport
                .iter_mut()
                .zip(metadata.payload_component_tys.iter())
            {
                self.refresh_value_transport_contract(transport, component_ty, payload_tuple_ty);
            }
        }
    }

    pub(super) fn refresh_value_transport_contract(
        &mut self,
        transport: &mut ValueTransportMetadata,
        source_ty: TypeId,
        boxing_target_ty: Option<TypeId>,
    ) {
        transport.source_ty = source_ty;
        transport.requirements =
            super::super::lower::mir_transport_requirements(&self.types, source_ty);
        if let Some(boxing) = &mut transport.boxing {
            boxing.source_ty = source_ty;
            if let Some(target_ty) = boxing_target_ty {
                boxing.target_ty = Some(target_ty);
            }
        }
    }

    pub(super) fn repair_unused_unresolved_compiler_temporaries(&mut self, body: &mut Body) {
        let referenced = collect_materialized_local_references(body);
        let mut fixed = HashSet::new();
        for (index, local) in body.locals.iter_mut().enumerate() {
            let local_id = LocalId::from_raw(index as u32);
            if local.source == LocalSourceKind::CompilerTemporary
                && !referenced.contains(&local_id)
                && type_contains_param(&self.types, local.ty)
            {
                local.ty = self.builtins.unit;
                fixed.insert(local_id);
            }
        }
        if fixed.is_empty() {
            return;
        }
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                let StatementKind::Assign { target, value } = &mut stmt.kind else {
                    continue;
                };
                if fixed.contains(target) {
                    *value = Rvalue::Use(Operand::Const(ConstValue::Unit));
                }
            }
        }
    }

    pub(super) fn member_value_result_ty(
        &mut self,
        receiver_ty: TypeId,
        member: &MemberAccessMetadata,
    ) -> Option<TypeId> {
        let fqn = match member.resolved.as_ref()? {
            MemberTarget::Value { fqn } | MemberTarget::ExtensionValue { fqn } => fqn,
            MemberTarget::Fun { .. } | MemberTarget::ExtensionFun { .. } => return None,
        };
        let info = self.member_value_tys.get(fqn)?.clone();
        let mut substitution = InstanceSubstitution::default();
        match self.types.kind(receiver_ty).clone() {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
            | TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == info.owner_fqn =>
            {
                for (name, ty) in info.owner_type_param_names.iter().zip(nominal.args.iter()) {
                    substitution.type_params.insert(name.clone(), *ty);
                }
                if let Some(row) = nominal.eff {
                    let mut names = HashSet::new();
                    collect_effect_row_param_names_in_type(&self.types, info.ty, &mut names);
                    for name in names {
                        substitution.effect_params.insert(name, row.clone());
                    }
                }
            }
            _ if info.owner_type_param_names.is_empty() => {}
            _ => return None,
        }
        Some(substitute_type_and_effect_params(
            &mut self.types,
            info.ty,
            &substitution,
        ))
    }

    pub(super) fn rewrite_body_blocks(
        &mut self,
        body: &mut Body,
        substitution: &InstanceSubstitution,
        template_source_path: &Path,
        template_root_fqn: &str,
        instance_root_fqn: &str,
        block_indices: Option<Vec<usize>>,
    ) -> MaterializeResult<()> {
        for local in &mut body.locals {
            local.ty = substitute_type_and_effect_params(&mut self.types, local.ty, substitution);
        }
        self.elide_unused_generic_top_level_refs(body);
        let locals = body.locals.clone();
        let ctx = RewriteContext {
            locals: &locals,
            substitution,
            template_source_path,
            template_root_fqn,
            instance_root_fqn,
        };
        self.materialize_function_value_top_level_refs(body, &ctx, block_indices.as_deref())?;
        if let Some(block_indices) = block_indices {
            for block_idx in block_indices {
                let Some(block) = body.blocks.get_mut(block_idx) else {
                    continue;
                };
                self.rewrite_block(BasicBlockId::from_raw(block_idx as u32), block, &ctx)?;
            }
        } else {
            for (block_idx, block) in body.blocks.iter_mut().enumerate() {
                self.rewrite_block(BasicBlockId::from_raw(block_idx as u32), block, &ctx)?;
            }
        }
        Ok(())
    }

    pub(super) fn elide_unused_generic_top_level_refs(&self, body: &mut Body) {
        let referenced = collect_materialized_local_references(body);
        for block in &mut body.blocks {
            for stmt in &mut block.stmts {
                let StatementKind::Assign { target, value } = &mut stmt.kind else {
                    continue;
                };
                if referenced.contains(target) {
                    continue;
                }
                let Rvalue::TopLevelRef(top) = value else {
                    continue;
                };
                if !self.roots_by_fqn.contains_key(&top.fqn) {
                    continue;
                }
                let Some(local) = body.locals.get_mut(target.as_u32() as usize) else {
                    continue;
                };
                if local.source != LocalSourceKind::CompilerTemporary {
                    continue;
                }
                local.ty = self.builtins.unit;
                *value = Rvalue::Use(Operand::Const(ConstValue::Unit));
            }
        }
    }

    pub(super) fn materialize_function_value_top_level_refs(
        &mut self,
        body: &mut Body,
        ctx: &RewriteContext<'_>,
        block_indices: Option<&[usize]>,
    ) -> MaterializeResult<()> {
        let selected_blocks = block_indices
            .map(|indices| indices.to_vec())
            .unwrap_or_else(|| (0..body.blocks.len()).collect());
        let mut top_refs: HashMap<LocalId, TopLevelRef> = HashMap::new();
        let mut patches: HashMap<LocalId, InstanceKey> = HashMap::new();

        for &block_idx in &selected_blocks {
            let Some(block) = body.blocks.get(block_idx) else {
                continue;
            };
            for stmt in &block.stmts {
                let StatementKind::Assign { target, value } = &stmt.kind else {
                    continue;
                };
                match value {
                    Rvalue::TopLevelRef(top)
                        if top.stable_template_key.is_some()
                            || self.roots_by_fqn.contains_key(&top.fqn) =>
                    {
                        top_refs.insert(*target, top.clone());
                    }
                    Rvalue::Call {
                        kind:
                            CallKind::FunValue {
                                callee: Operand::Local(callee),
                            },
                        ..
                    } => {
                        let Some(top_ref) = top_refs.get(callee).cloned() else {
                            continue;
                        };
                        if let Some(instance_key) = self
                            .instance_key_from_top_level_ref_identity(&top_ref, ctx.substitution)?
                        {
                            patches.insert(*callee, instance_key);
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut replacements = HashMap::new();
        for (local, instance_key) in patches {
            let instance_fqn = self.instance_display_fqn(&instance_key);
            let fun_ty = self.instance_fun_ty(&instance_key);
            self.enqueue(instance_key);
            replacements.insert(local, (instance_fqn, fun_ty));
        }
        if replacements.is_empty() {
            return Ok(());
        }

        for (local, (_, fun_ty)) in &replacements {
            if let Some(fun_ty) = fun_ty
                && let Some(decl) = body.locals.get_mut(local.as_u32() as usize)
            {
                decl.ty = *fun_ty;
            }
        }
        for &block_idx in &selected_blocks {
            let Some(block) = body.blocks.get_mut(block_idx) else {
                continue;
            };
            for stmt in &mut block.stmts {
                let StatementKind::Assign {
                    target,
                    value: Rvalue::TopLevelRef(top),
                } = &mut stmt.kind
                else {
                    continue;
                };
                if let Some((instance_fqn, _)) = replacements.get(target) {
                    top.fqn = instance_fqn.clone();
                }
            }
        }
        Ok(())
    }

    pub(super) fn rewrite_block(
        &mut self,
        block_id: BasicBlockId,
        block: &mut super::super::BasicBlock,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        for stmt in &mut block.stmts {
            self.rewrite_statement(stmt, block_id, ctx)?;
        }
        self.rewrite_terminator(&mut block.terminator, block_id, ctx)
    }

    pub(super) fn rewrite_statement(
        &mut self,
        stmt: &mut Statement,
        block_id: BasicBlockId,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        match &mut stmt.kind {
            StatementKind::Assign { target, value } => {
                let result_ty = ctx
                    .locals
                    .get(target.as_u32() as usize)
                    .map(|local| local.ty);
                self.rewrite_rvalue(stmt.span, block_id, value, result_ty, ctx)?
            }
            StatementKind::StoreMember {
                receiver,
                member,
                value,
                value_ty,
                continuation_route,
            } => {
                *receiver = self.rewrite_operand(receiver.clone());
                self.rewrite_member_access_metadata(member, ctx);
                *value = self.rewrite_operand(value.clone());
                *value_ty =
                    substitute_type_and_effect_params(&mut self.types, *value_ty, ctx.substitution);
                if let crate::mir::StoredContinuationRoutePublication::Unique(route) =
                    continuation_route
                {
                    route.source_ty = substitute_type_and_effect_params(
                        &mut self.types,
                        route.source_ty,
                        ctx.substitution,
                    );
                }
            }
            StatementKind::StoreTopLevelVar {
                value, value_ty, ..
            } => {
                *value = self.rewrite_operand(value.clone());
                *value_ty =
                    substitute_type_and_effect_params(&mut self.types, *value_ty, ctx.substitution);
            }
            StatementKind::Todo(reason) => {
                return Err(materialize_err(MirMaterializeError::MaterializedTodo {
                    fqn: ctx.instance_root_fqn.to_string(),
                    block: Some(block_id),
                    span: stmt.span,
                    category: MirPlaceholderCategory::Statement,
                    reason: reason.clone(),
                }));
            }
            StatementKind::Nop => {}
        }
        Ok(())
    }

    pub(super) fn rewrite_terminator(
        &mut self,
        terminator: &mut Terminator,
        block_id: BasicBlockId,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        self.rewrite_unwind_action(terminator.span, block_id, &terminator.unwind, ctx)?;
        match &mut terminator.kind {
            TerminatorKind::Perform { metadata, args, .. } => {
                self.rewrite_perform_metadata(metadata, ctx.substitution);
                for arg in args {
                    arg.value = self.rewrite_operand(arg.value.clone());
                }
            }
            TerminatorKind::Handle { metadata, arms, .. } => {
                self.rewrite_handle_metadata(metadata, ctx.substitution);
                for arm in arms {
                    self.rewrite_handler_arm(arm, ctx.substitution);
                }
            }
            TerminatorKind::CondBr { cond, .. } => {
                *cond = self.rewrite_operand(cond.clone());
            }
            TerminatorKind::Return { value } => {
                *value = value.take().map(|operand| self.rewrite_operand(operand));
            }
            TerminatorKind::ResumeUnwind
            | TerminatorKind::Goto { .. }
            | TerminatorKind::Unreachable => {}
            TerminatorKind::Todo(reason) => {
                return Err(materialize_err(MirMaterializeError::MaterializedTodo {
                    fqn: ctx.instance_root_fqn.to_string(),
                    block: Some(block_id),
                    span: terminator.span,
                    category: MirPlaceholderCategory::Terminator,
                    reason: reason.clone(),
                }));
            }
        }
        Ok(())
    }

    pub(super) fn rewrite_unwind_action(
        &mut self,
        span: Span,
        block_id: BasicBlockId,
        unwind: &UnwindAction,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        match unwind {
            UnwindAction::Todo(reason) => {
                Err(materialize_err(MirMaterializeError::MaterializedTodo {
                    fqn: ctx.instance_root_fqn.to_string(),
                    block: Some(block_id),
                    span,
                    category: MirPlaceholderCategory::UnwindAction,
                    reason: reason.clone(),
                }))
            }
            UnwindAction::NoUnwind | UnwindAction::Propagate | UnwindAction::Cleanup { .. } => {
                Ok(())
            }
        }
    }

    pub(super) fn rewrite_handle_metadata(
        &mut self,
        metadata: &mut HandleMetadata,
        substitution: &InstanceSubstitution,
    ) {
        metadata.result_ty =
            substitute_type_and_effect_params(&mut self.types, metadata.result_ty, substitution);
        metadata.body_result_ty = substitute_type_and_effect_params(
            &mut self.types,
            metadata.body_result_ty,
            substitution,
        );
        metadata.finally_result_ty = metadata
            .finally_result_ty
            .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution));
    }

    pub(super) fn rewrite_handler_arm(
        &mut self,
        arm: &mut HandlerArm,
        substitution: &InstanceSubstitution,
    ) {
        arm.handled_effect_ty =
            substitute_type_and_effect_params(&mut self.types, arm.handled_effect_ty, substitution);
        arm.payload_tuple_ty = arm
            .payload_tuple_ty
            .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution));
        for ty in &mut arm.payload_component_tys {
            *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
        }
        arm.body_ty = substitute_type_and_effect_params(&mut self.types, arm.body_ty, substitution);
    }

    pub(super) fn rewrite_rvalue(
        &mut self,
        stmt_span: Span,
        block_id: BasicBlockId,
        value: &mut Rvalue,
        result_ty: Option<TypeId>,
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        match value {
            Rvalue::Use(operand) => *operand = self.rewrite_operand(operand.clone()),
            Rvalue::Transport {
                value: operand,
                transport,
            } => {
                *operand = self.rewrite_operand(operand.clone());
                self.rewrite_value_transport(transport, ctx.substitution);
                if transport.boxing.is_none() {
                    *value = Rvalue::Use(operand.clone());
                }
            }
            Rvalue::TopLevelRef(top) => {
                if let Some(rewritten) = rewrite_family_symbol_name(
                    &top.fqn,
                    ctx.template_root_fqn,
                    ctx.instance_root_fqn,
                ) {
                    top.fqn = rewritten;
                } else {
                    self.materialize_top_level_ref_target(
                        &mut top.fqn,
                        &mut top.stable_template_key,
                        &mut top.stable_instance_key,
                        &top.generic_type_args,
                        &top.generic_eff_args,
                        DirectCallRewriteContext {
                            template_source_path: ctx.template_source_path,
                            caller_fqn: ctx.instance_root_fqn,
                            block_id,
                            call_span: stmt_span,
                            locals: ctx.locals,
                            substitution: ctx.substitution,
                        },
                    )?;
                }
                top.hidden_effects.terms = top
                    .hidden_effects
                    .terms
                    .iter()
                    .map(|ty| {
                        substitute_type_and_effect_params(&mut self.types, *ty, ctx.substitution)
                    })
                    .collect();
            }
            Rvalue::UnresolvedName { .. } => {}
            Rvalue::TypeCheck {
                value,
                test_ty,
                metadata,
                ..
            } => {
                *value = self.rewrite_operand(value.clone());
                *test_ty =
                    substitute_type_and_effect_params(&mut self.types, *test_ty, ctx.substitution);
                self.rewrite_type_test_metadata(metadata, ctx.substitution);
            }
            Rvalue::Cast {
                value,
                target_ty,
                metadata,
                ..
            } => {
                *value = self.rewrite_operand(value.clone());
                *target_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    *target_ty,
                    ctx.substitution,
                );
                self.rewrite_cast_metadata(metadata, ctx.substitution);
            }
            Rvalue::SizeOf { value_ty } => {
                *value_ty =
                    substitute_type_and_effect_params(&mut self.types, *value_ty, ctx.substitution);
            }
            Rvalue::KindOf { value_ty } => {
                *value_ty =
                    substitute_type_and_effect_params(&mut self.types, *value_ty, ctx.substitution);
            }
            Rvalue::AlignOf { value_ty } => {
                *value_ty =
                    substitute_type_and_effect_params(&mut self.types, *value_ty, ctx.substitution);
            }
            Rvalue::DescOf { value_ty } => {
                *value_ty =
                    substitute_type_and_effect_params(&mut self.types, *value_ty, ctx.substitution);
            }
            Rvalue::TypeMetadataLiteral(metadata) => {
                metadata.source_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    metadata.source_ty,
                    ctx.substitution,
                );
            }
            Rvalue::MemberAccess {
                receiver, member, ..
            } => {
                *receiver = self.rewrite_operand(receiver.clone());
                self.rewrite_member_access_metadata(member, ctx);
            }
            Rvalue::Call {
                kind,
                args,
                transport,
                ..
            } => {
                for arg in args.iter_mut() {
                    arg.value = self.rewrite_operand(arg.value.clone());
                }
                self.rewrite_call_kind(stmt_span, block_id, kind, args, ctx)?;
                self.rewrite_call_transport(transport, ctx.substitution);
            }
            Rvalue::EnumVariant {
                enum_ty,
                args,
                payload,
                ..
            } => {
                *enum_ty =
                    substitute_type_and_effect_params(&mut self.types, *enum_ty, ctx.substitution);
                for arg in args.iter_mut() {
                    arg.value = self.rewrite_operand(arg.value.clone());
                }
                self.rewrite_aggregate_transport(payload, ctx.substitution);
            }
            Rvalue::ClassCtor {
                class_fqn,
                args,
                hidden_effects,
                ..
            } => {
                if let Some(result_ty) = result_ty
                    && let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(result_ty)
                {
                    *class_fqn = nominal.fqn.clone();
                }
                for arg in args.iter_mut() {
                    arg.value = self.rewrite_operand(arg.value.clone());
                }
                hidden_effects.terms = hidden_effects
                    .terms
                    .iter()
                    .map(|ty| {
                        substitute_type_and_effect_params(&mut self.types, *ty, ctx.substitution)
                    })
                    .collect();
            }
            Rvalue::MakeTuple {
                elements,
                transport,
            } => {
                for element in elements.iter_mut() {
                    *element = self.rewrite_operand(element.clone());
                }
                self.rewrite_aggregate_transport(transport, ctx.substitution);
            }
            Rvalue::StructLit { fields, transport } => {
                for field in fields.iter_mut() {
                    field.value = self.rewrite_operand(field.value.clone());
                }
                self.rewrite_aggregate_transport(transport, ctx.substitution);
            }
            Rvalue::InterpolatedString { parts, .. } => {
                for part in parts.iter_mut() {
                    if let super::InterpolatedStringPart::Expr { value, ty, .. } = part {
                        *value = self.rewrite_operand(value.clone());
                        *ty = substitute_type_and_effect_params(
                            &mut self.types,
                            *ty,
                            ctx.substitution,
                        );
                    }
                }
            }
            Rvalue::TupleGet { tuple, .. } => *tuple = self.rewrite_operand(tuple.clone()),
            Rvalue::PatternMatch { subject, pattern } => {
                *subject = self.rewrite_operand(subject.clone());
                self.rewrite_pattern(pattern, ctx.substitution);
            }
            Rvalue::PatternExtract { subject, path } => {
                *subject = self.rewrite_operand(subject.clone());
                let _ = path;
            }
            Rvalue::MakeClosure {
                env,
                fn_ptr,
                env_contract,
            } => {
                *env = self.rewrite_operand(env.clone());
                if let Some(rewritten) =
                    rewrite_family_symbol_name(fn_ptr, ctx.template_root_fqn, ctx.instance_root_fqn)
                {
                    *fn_ptr = rewritten;
                }
                self.rewrite_closure_env_contract(env_contract, ctx.substitution);
            }
            Rvalue::PerformResult { effect_ty, .. } => {
                *effect_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    *effect_ty,
                    ctx.substitution,
                );
            }
            Rvalue::Todo(reason) => {
                return Err(materialize_err(MirMaterializeError::MaterializedTodo {
                    fqn: ctx.instance_root_fqn.to_string(),
                    block: Some(block_id),
                    span: stmt_span,
                    category: MirPlaceholderCategory::Rvalue,
                    reason: reason.clone(),
                }));
            }
        }
        Ok(())
    }

    pub(super) fn rewrite_call_kind(
        &mut self,
        call_span: Span,
        block_id: BasicBlockId,
        kind: &mut CallKind,
        args: &mut [CallArg],
        ctx: &RewriteContext<'_>,
    ) -> MaterializeResult<()> {
        let direct_ctx = DirectCallRewriteContext {
            template_source_path: ctx.template_source_path,
            caller_fqn: ctx.instance_root_fqn,
            block_id,
            call_span,
            locals: ctx.locals,
            substitution: ctx.substitution,
        };
        match kind {
            CallKind::Direct {
                callee_fqn,
                stable_template_key,
                stable_instance_key,
                generic_type_args,
                generic_eff_args,
            } => {
                if let Some(rewritten) = rewrite_family_symbol_name(
                    callee_fqn,
                    ctx.template_root_fqn,
                    ctx.instance_root_fqn,
                ) {
                    *callee_fqn = rewritten;
                    return Ok(());
                }
                self.materialize_direct_call_target(
                    callee_fqn,
                    DirectCallStableIdentity {
                        stable_template_key,
                        stable_instance_key,
                        generic_type_args,
                        generic_eff_args,
                    },
                    args,
                    direct_ctx,
                )?;
            }
            CallKind::Closure { callee, fn_ptr } => {
                *callee = self.rewrite_operand(callee.clone());
                if let Some(rewritten) =
                    rewrite_family_symbol_name(fn_ptr, ctx.template_root_fqn, ctx.instance_root_fqn)
                {
                    *fn_ptr = rewritten;
                }
            }
            CallKind::FunValue { callee } | CallKind::FunPtr { callee } => {
                *callee = self.rewrite_operand(callee.clone())
            }
            CallKind::Virtual { receiver, dispatch } => {
                *receiver = self.rewrite_operand(receiver.clone());
                dispatch.receiver_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    dispatch.receiver_ty,
                    ctx.substitution,
                );
                self.materialize_dispatch_call_candidates(
                    crate::hir::DispatchCallKind::Virtual,
                    receiver,
                    dispatch,
                    args,
                    direct_ctx,
                )?;
            }
            CallKind::Interface { receiver, dispatch } => {
                *receiver = self.rewrite_operand(receiver.clone());
                dispatch.receiver_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    dispatch.receiver_ty,
                    ctx.substitution,
                );
                self.materialize_dispatch_call_candidates(
                    crate::hir::DispatchCallKind::Interface,
                    receiver,
                    dispatch,
                    args,
                    direct_ctx,
                )?;
            }
            CallKind::Resume {
                continuation,
                resume,
            } => {
                *continuation = self.rewrite_operand(continuation.clone());
                resume.continuation_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    resume.continuation_ty,
                    ctx.substitution,
                );
                resume.resume_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    resume.resume_ty,
                    ctx.substitution,
                );
                resume.answer_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    resume.answer_ty,
                    ctx.substitution,
                );
                resume.return_ty = substitute_type_and_effect_params(
                    &mut self.types,
                    resume.return_ty,
                    ctx.substitution,
                );
                resume.out_effects = substitute_type_and_effect_params_in_effect_row(
                    &mut self.types,
                    &resume.out_effects,
                    ctx.substitution,
                );
                resume.runtime_error_effect_ty = resume.runtime_error_effect_ty.map(|ty| {
                    substitute_type_and_effect_params(&mut self.types, ty, ctx.substitution)
                });
            }
        }
        Ok(())
    }

    fn materialize_dispatch_call_candidates(
        &mut self,
        kind: crate::hir::DispatchCallKind,
        receiver: &Operand,
        dispatch: &DispatchMetadata,
        args: &[CallArg],
        ctx: DirectCallRewriteContext<'_>,
    ) -> MaterializeResult<()> {
        if !dispatch.stable_candidate_keys.is_empty() {
            let stable_candidate_count = dispatch.stable_candidate_keys.len();
            for stable_key in &dispatch.stable_candidate_keys {
                if let Some(instance_key) =
                    self.instance_key_from_stable_instance_key(stable_key)?
                {
                    let instance_fqn = self.instance_display_fqn(&instance_key);
                    self.record_dispatch_devirtualization_target(
                        &instance_fqn,
                        instance_fqn.clone(),
                        ctx,
                    );
                    if stable_candidate_count == 1 {
                        self.record_dispatch_devirtualization_target(
                            &dispatch.member_fqn,
                            instance_fqn.clone(),
                            ctx,
                        );
                    }
                    self.enqueue(instance_key);
                }
            }
            return Ok(());
        }
        if let Some(stable_template_key) = dispatch.stable_template_key.as_deref()
            && let Some(instance_key) = self.instance_key_from_stable_template_args(
                stable_template_key,
                &dispatch.generic_type_args,
                &dispatch.generic_eff_args,
                ctx.substitution,
            )?
        {
            let instance_fqn = self.instance_display_fqn(&instance_key);
            self.record_dispatch_devirtualization_target(
                &dispatch.member_fqn,
                instance_fqn.clone(),
                ctx,
            );
            self.enqueue(instance_key);
            return Ok(());
        }
        let candidates = match kind {
            crate::hir::DispatchCallKind::Virtual => self.virtual_dispatch_candidate_fqns(
                dispatch.receiver_ty,
                &dispatch.member_name,
                args.len(),
            ),
            crate::hir::DispatchCallKind::Interface => self.interface_dispatch_candidate_fqns(
                dispatch.receiver_ty,
                &dispatch.owner_fqn,
                &dispatch.member_name,
                args.len(),
            ),
        };
        if candidates.is_empty() {
            return Ok(());
        }

        let direct_args = Self::dispatch_direct_call_args(ctx.call_span, receiver, args);
        for candidate_fqn in candidates {
            if let Some(reachable_callee) = self.resolve_non_generic_direct_callee(
                ctx.template_source_path,
                ctx.call_span,
                &candidate_fqn,
                &direct_args,
                ctx.locals,
            ) {
                let pass_fqn = self.pass_visible_non_generic_callable_fqn(
                    reachable_callee.source_path.as_path(),
                    &reachable_callee.fun,
                );
                self.record_dispatch_devirtualization_target(&candidate_fqn, pass_fqn, ctx);
                let mut discovered = Vec::new();
                self.scan_reachable_non_generic_fun(&reachable_callee, &mut discovered)?;
                for instance in discovered {
                    self.enqueue(instance);
                }
            }
        }
        Ok(())
    }

    fn record_dispatch_devirtualization_target(
        &mut self,
        candidate_fqn: &str,
        canonical_fqn: String,
        ctx: DirectCallRewriteContext<'_>,
    ) {
        self.dispatch_devirtualization_targets.insert(
            DispatchDevirtualizationTargetKey::new(
                ctx.caller_fqn,
                ctx.block_id,
                ctx.call_span,
                candidate_fqn,
            ),
            canonical_fqn,
        );
    }

    fn dispatch_direct_call_args(
        call_span: Span,
        receiver: &Operand,
        args: &[CallArg],
    ) -> Vec<CallArg> {
        let mut direct_args = Vec::with_capacity(args.len() + 1);
        direct_args.push(CallArg {
            span: call_span,
            name: None,
            value: receiver.clone(),
        });
        direct_args.extend(args.iter().cloned());
        direct_args
    }

    pub(super) fn instance_key_from_stable_template_args(
        &mut self,
        stable_template_key: &StableTemplateKey,
        type_args: &[TypeId],
        eff_args: &[EffectRow],
        substitution: &InstanceSubstitution,
    ) -> MaterializeResult<Option<InstanceKey>> {
        let Some(template) = self.resolve_stable_request_template(stable_template_key) else {
            return Ok(None);
        };
        let type_args = type_args
            .iter()
            .copied()
            .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution))
            .collect::<Vec<_>>();
        let eff_args = eff_args
            .iter()
            .map(|row| {
                substitute_type_and_effect_params_in_effect_row(&mut self.types, row, substitution)
            })
            .collect::<Vec<_>>();
        if !instance_request_is_concrete(&self.types, &type_args, &eff_args) {
            return Ok(None);
        }
        Ok(Some(InstanceKey {
            template,
            type_args,
            eff_args,
        }))
    }

    pub(super) fn materialize_direct_call_target(
        &mut self,
        callee_fqn: &mut String,
        identity: DirectCallStableIdentity<'_>,
        args: &[CallArg],
        ctx: DirectCallRewriteContext<'_>,
    ) -> MaterializeResult<()> {
        if let Some(stable_key) = identity.stable_instance_key.as_deref()
            && let Some(instance_key) = self.instance_key_from_stable_instance_key(stable_key)?
        {
            let instance_fqn = self.instance_display_fqn(&instance_key);
            if let Some(return_ty) = self.instance_return_ty(&instance_key)
                && !type_contains_param(&self.types, return_ty)
            {
                self.materialized_direct_call_result_tys
                    .insert(instance_fqn.clone(), return_ty);
            }
            *callee_fqn = instance_fqn;
            *identity.stable_template_key = Some(Box::new(stable_key.template().clone()));
            self.enqueue(instance_key);
            return Ok(());
        }

        if let Some(stable_key) = identity.stable_template_key.as_deref()
            && let Some(instance_key) = self.instance_key_from_stable_template_args(
                stable_key,
                identity.generic_type_args,
                identity.generic_eff_args,
                ctx.substitution,
            )?
        {
            let instance_fqn = self.instance_display_fqn(&instance_key);
            if let Some(return_ty) = self.instance_return_ty(&instance_key)
                && !type_contains_param(&self.types, return_ty)
            {
                self.materialized_direct_call_result_tys
                    .insert(instance_fqn.clone(), return_ty);
            }
            *callee_fqn = instance_fqn;
            let stable_key = self.stable_instance_key(&instance_key);
            *identity.stable_template_key = Some(Box::new(stable_key.template().clone()));
            *identity.stable_instance_key = Some(Box::new(stable_key));
            self.enqueue(instance_key);
            return Ok(());
        }

        let site_key = (ctx.template_source_path.to_path_buf(), ctx.call_span);
        if let Some(binding) = self.call_bindings.get(&site_key).cloned()
            && let Some(instance_key) =
                self.instance_key_from_site_binding(&binding, ctx.substitution)
        {
            let instance_fqn = self.instance_display_fqn(&instance_key);
            if let Some(return_ty) = self.instance_return_ty(&instance_key)
                && !type_contains_param(&self.types, return_ty)
            {
                self.materialized_direct_call_result_tys
                    .insert(instance_fqn.clone(), return_ty);
            }
            *callee_fqn = instance_fqn;
            let stable_key = self.stable_instance_key(&instance_key);
            *identity.stable_template_key = Some(Box::new(stable_key.template().clone()));
            *identity.stable_instance_key = Some(Box::new(stable_key));
            self.enqueue(instance_key);
            return Ok(());
        }
        if let Some(binding) =
            self.value_ref_binding_for_site(ctx.template_source_path, ctx.call_span)
            && let Some(instance_key) =
                self.instance_key_from_site_binding(&binding, ctx.substitution)
        {
            let instance_fqn = self.instance_display_fqn(&instance_key);
            if let Some(return_ty) = self.instance_return_ty(&instance_key)
                && !type_contains_param(&self.types, return_ty)
            {
                self.materialized_direct_call_result_tys
                    .insert(instance_fqn.clone(), return_ty);
            }
            *callee_fqn = instance_fqn;
            let stable_key = self.stable_instance_key(&instance_key);
            *identity.stable_template_key = Some(Box::new(stable_key.template().clone()));
            *identity.stable_instance_key = Some(Box::new(stable_key));
            self.enqueue(instance_key);
            return Ok(());
        }

        if let Some(reachable_callee) = self.resolve_bound_non_generic_fun_call(
            ctx.template_source_path,
            ctx.call_span,
            callee_fqn,
        ) {
            let pass_fqn = self.pass_visible_non_generic_callable_fqn(
                reachable_callee.source_path.as_path(),
                &reachable_callee.fun,
            );
            if !type_contains_param(&self.types, reachable_callee.fun.return_ty) {
                self.materialized_direct_call_result_tys
                    .insert(pass_fqn.clone(), reachable_callee.fun.return_ty);
            }
            *callee_fqn = pass_fqn;
            let mut discovered = Vec::new();
            self.scan_reachable_non_generic_fun(&reachable_callee, &mut discovered)?;
            for instance in discovered {
                self.enqueue(instance);
            }
            return Ok(());
        }
        if is_canonical_array_member_intrinsic_fqn(callee_fqn) {
            return Ok(());
        }
        if self.roots_by_fqn.contains_key(callee_fqn) {
            return Err(materialize_err(
                MirMaterializeError::MaterializedMissingCallTarget {
                    fqn: ctx.caller_fqn.to_string(),
                    block: Some(ctx.block_id),
                    span: ctx.call_span,
                    callee_fqn: callee_fqn.clone(),
                },
            ));
        }
        if let Some(reachable_callee) = self.resolve_non_generic_direct_callee(
            ctx.template_source_path,
            ctx.call_span,
            callee_fqn,
            args,
            ctx.locals,
        ) {
            let pass_fqn = self.pass_visible_non_generic_callable_fqn(
                reachable_callee.source_path.as_path(),
                &reachable_callee.fun,
            );
            if !type_contains_param(&self.types, reachable_callee.fun.return_ty) {
                self.materialized_direct_call_result_tys
                    .insert(pass_fqn.clone(), reachable_callee.fun.return_ty);
            }
            *callee_fqn = pass_fqn;
            let mut discovered = Vec::new();
            self.scan_reachable_non_generic_fun(&reachable_callee, &mut discovered)?;
            for instance in discovered {
                self.enqueue(instance);
            }
        }
        Ok(())
    }

    pub(super) fn materialize_top_level_ref_target(
        &mut self,
        fqn: &mut String,
        stable_template_key: &mut Option<Box<StableTemplateKey>>,
        stable_instance_key: &mut Option<Box<StableInstanceKey>>,
        generic_type_args: &[TypeId],
        generic_eff_args: &[EffectRow],
        ctx: DirectCallRewriteContext<'_>,
    ) -> MaterializeResult<()> {
        if let Some(stable_key) = stable_instance_key.as_deref()
            && let Some(instance_key) = self.instance_key_from_stable_instance_key(stable_key)?
        {
            *fqn = self.instance_display_fqn(&instance_key);
            *stable_template_key = Some(Box::new(stable_key.template().clone()));
            self.enqueue(instance_key);
            return Ok(());
        }
        if let Some(stable_key) = stable_template_key.as_deref()
            && let Some(instance_key) = self.instance_key_from_stable_template_args(
                stable_key,
                generic_type_args,
                generic_eff_args,
                ctx.substitution,
            )?
        {
            *fqn = self.instance_display_fqn(&instance_key);
            let stable_key = self.stable_instance_key(&instance_key);
            *stable_template_key = Some(Box::new(stable_key.template().clone()));
            *stable_instance_key = Some(Box::new(stable_key));
            self.enqueue(instance_key);
            return Ok(());
        }
        if let Some(binding) =
            self.value_ref_binding_for_site(ctx.template_source_path, ctx.call_span)
            && let Some(instance_key) =
                self.instance_key_from_site_binding(&binding, ctx.substitution)
        {
            *fqn = self.instance_display_fqn(&instance_key);
            self.enqueue(instance_key);
            return Ok(());
        }
        if is_canonical_array_member_intrinsic_fqn(fqn) {
            return Ok(());
        }
        if let Some(reachable_fun) = self.resolve_non_generic_top_level_ref_target(
            ctx.template_source_path,
            ctx.call_span,
            fqn,
        ) {
            *fqn = self.pass_visible_non_generic_callable_fqn(
                reachable_fun.source_path.as_path(),
                &reachable_fun.fun,
            );
            let mut discovered = Vec::new();
            self.scan_reachable_non_generic_fun(&reachable_fun, &mut discovered)?;
            for instance in discovered {
                self.enqueue(instance);
            }
            return Ok(());
        }
        if self.roots_by_fqn.contains_key(fqn) {
            return Err(materialize_err(
                MirMaterializeError::MaterializedMissingCallTarget {
                    fqn: ctx.caller_fqn.to_string(),
                    block: Some(ctx.block_id),
                    span: ctx.call_span,
                    callee_fqn: fqn.clone(),
                },
            ));
        }
        Ok(())
    }

    pub(super) fn instance_key_from_top_level_ref_identity(
        &mut self,
        top_ref: &TopLevelRef,
        substitution: &InstanceSubstitution,
    ) -> MaterializeResult<Option<InstanceKey>> {
        if let Some(stable_key) = top_ref.stable_instance_key.as_deref() {
            return self.instance_key_from_stable_instance_key(stable_key);
        }
        let Some(stable_template_key) = top_ref.stable_template_key.as_deref() else {
            return Ok(None);
        };
        self.instance_key_from_stable_template_args(
            stable_template_key,
            &top_ref.generic_type_args,
            &top_ref.generic_eff_args,
            substitution,
        )
    }

    pub(super) fn instance_key_from_site_binding(
        &mut self,
        binding: &SiteInstanceBinding,
        substitution: &InstanceSubstitution,
    ) -> Option<InstanceKey> {
        let type_args = binding
            .type_args
            .iter()
            .copied()
            .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution))
            .collect::<Vec<_>>();
        let eff_args = binding
            .eff_args
            .iter()
            .map(|row| {
                substitute_type_and_effect_params_in_effect_row(&mut self.types, row, substitution)
            })
            .collect::<Vec<_>>();
        if (type_args.is_empty() && eff_args.is_empty())
            || !instance_request_is_concrete(&self.types, &type_args, &eff_args)
        {
            return None;
        }
        Some(InstanceKey {
            template: binding.template.clone(),
            type_args,
            eff_args,
        })
    }

    pub(super) fn value_ref_binding_for_site(
        &self,
        template_source_path: &Path,
        span: Span,
    ) -> Option<SiteInstanceBinding> {
        let exact_key = (template_source_path.to_path_buf(), span);
        if let Some(binding) = self.value_ref_bindings.get(&exact_key) {
            return Some(binding.clone());
        }
        let mut found: Option<(Span, &SiteInstanceBinding)> = None;
        for ((source_path, binding_span), binding) in &self.value_ref_bindings {
            if source_path != template_source_path
                || binding_span.start > span.start
                || span.end > binding_span.end
            {
                continue;
            }
            let Some((found_span, found_binding)) = found else {
                found = Some((*binding_span, binding));
                continue;
            };
            if found_binding != binding {
                return None;
            }
            if binding_span.end - binding_span.start < found_span.end - found_span.start {
                found = Some((*binding_span, binding));
            }
        }
        found.map(|(_, binding)| binding.clone())
    }

    pub(super) fn instance_return_ty(&mut self, instance: &InstanceKey) -> Option<TypeId> {
        let signature = self.template_signatures.get(&instance.template)?.clone();
        let substitution = self.build_instance_substitution_for_signature(&signature, instance);
        Some(substitute_type_and_effect_params(
            &mut self.types,
            signature.return_ty,
            &substitution,
        ))
    }

    pub(super) fn instance_fun_ty(&mut self, instance: &InstanceKey) -> Option<TypeId> {
        let signature = self.template_signatures.get(&instance.template)?.clone();
        let substitution = self.build_instance_substitution_for_signature(&signature, instance);
        Some(substitute_type_and_effect_params(
            &mut self.types,
            signature.fun_ty,
            &substitution,
        ))
    }

    #[cfg(test)]
    pub(super) fn lookup_site_instance_binding(
        &self,
        template_source_path: &Path,
        call_span: Span,
    ) -> Option<&SiteInstanceBinding> {
        let key = (template_source_path.to_path_buf(), call_span);
        self.call_bindings
            .get(&key)
            .or_else(|| self.value_ref_bindings.get(&key))
            .or_else(|| self.lookup_enclosed_site_instance_binding(template_source_path, call_span))
    }

    #[cfg(test)]
    pub(super) fn lookup_enclosed_site_instance_binding(
        &self,
        template_source_path: &Path,
        enclosing_span: Span,
    ) -> Option<&SiteInstanceBinding> {
        lookup_overlapping_site_instance_binding(
            &self.call_bindings,
            template_source_path,
            enclosing_span,
        )
        .or_else(|| {
            lookup_overlapping_site_instance_binding(
                &self.value_ref_bindings,
                template_source_path,
                enclosing_span,
            )
        })
    }

    pub(super) fn rewrite_member_access_metadata(
        &mut self,
        member: &mut MemberAccessMetadata,
        ctx: &RewriteContext<'_>,
    ) {
        member.receiver_ty = substitute_type_and_effect_params(
            &mut self.types,
            member.receiver_ty,
            ctx.substitution,
        );
        member.hidden_effects.terms = member
            .hidden_effects
            .terms
            .iter()
            .map(|ty| substitute_type_and_effect_params(&mut self.types, *ty, ctx.substitution))
            .collect();
        if let Some(target) = &mut member.resolved {
            match target {
                MemberTarget::Fun { fqn } | MemberTarget::ExtensionFun { fqn } => {
                    if let Some(rewritten) = rewrite_family_symbol_name(
                        fqn,
                        ctx.template_root_fqn,
                        ctx.instance_root_fqn,
                    ) {
                        *fqn = rewritten;
                    }
                }
                MemberTarget::Value { .. } | MemberTarget::ExtensionValue { .. } => {}
            }
        }
    }

    pub(super) fn rewrite_type_test_metadata(
        &mut self,
        metadata: &mut RuntimeTypeTestMetadata,
        substitution: &InstanceSubstitution,
    ) {
        metadata.source_ty =
            substitute_type_and_effect_params(&mut self.types, metadata.source_ty, substitution);
        metadata.target_ty =
            substitute_type_and_effect_params(&mut self.types, metadata.target_ty, substitution);
        self.rewrite_descriptor_key(&mut metadata.descriptor, substitution);
        self.rewrite_parameterized_match(&mut metadata.parameterized, substitution);
    }

    pub(super) fn rewrite_cast_metadata(
        &mut self,
        metadata: &mut RuntimeCastMetadata,
        substitution: &InstanceSubstitution,
    ) {
        self.rewrite_type_test_metadata(&mut metadata.test, substitution);
        match &mut metadata.result {
            RuntimeCastResult::Target { ty } => {
                *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
            }
            RuntimeCastResult::Option { option_ty, some_ty } => {
                *option_ty =
                    substitute_type_and_effect_params(&mut self.types, *option_ty, substitution);
                *some_ty =
                    substitute_type_and_effect_params(&mut self.types, *some_ty, substitution);
            }
        }
    }

    pub(super) fn rewrite_pattern_type_test_metadata(
        &mut self,
        metadata: &mut RuntimePatternTypeTestMetadata,
        substitution: &InstanceSubstitution,
    ) {
        metadata.subject_ty =
            substitute_type_and_effect_params(&mut self.types, metadata.subject_ty, substitution);
        metadata.target_ty =
            substitute_type_and_effect_params(&mut self.types, metadata.target_ty, substitution);
        self.rewrite_descriptor_key(&mut metadata.descriptor, substitution);
        self.rewrite_parameterized_match(&mut metadata.parameterized, substitution);
    }

    pub(super) fn rewrite_descriptor_key(
        &mut self,
        descriptor: &mut RuntimeTypeDescriptorKey,
        substitution: &InstanceSubstitution,
    ) {
        descriptor.ty =
            substitute_type_and_effect_params(&mut self.types, descriptor.ty, substitution);
    }

    pub(super) fn rewrite_parameterized_match(
        &mut self,
        parameterized: &mut RuntimeTypeParameterizedMatch,
        substitution: &InstanceSubstitution,
    ) {
        match parameterized {
            RuntimeTypeParameterizedMatch::None => {}
            RuntimeTypeParameterizedMatch::Nominal {
                type_args,
                effect_arg,
            } => {
                for ty in type_args {
                    *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
                }
                *effect_arg = effect_arg.as_ref().map(|row| {
                    substitute_type_and_effect_params_in_effect_row(
                        &mut self.types,
                        row,
                        substitution,
                    )
                });
            }
            RuntimeTypeParameterizedMatch::Function {
                receiver,
                params,
                return_ty,
                effects,
                ..
            } => {
                *receiver = receiver
                    .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution));
                for ty in params {
                    *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
                }
                *return_ty =
                    substitute_type_and_effect_params(&mut self.types, *return_ty, substitution);
                *effects = substitute_type_and_effect_params_in_effect_row(
                    &mut self.types,
                    effects,
                    substitution,
                );
            }
            RuntimeTypeParameterizedMatch::Option { payload_ty } => {
                *payload_ty =
                    substitute_type_and_effect_params(&mut self.types, *payload_ty, substitution);
            }
            RuntimeTypeParameterizedMatch::Tuple { element_tys } => {
                for ty in element_tys {
                    *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
                }
            }
            RuntimeTypeParameterizedMatch::Union { variants } => {
                for ty in variants {
                    *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
                }
            }
            RuntimeTypeParameterizedMatch::StarProjection { read_ty } => {
                *read_ty =
                    substitute_type_and_effect_params(&mut self.types, *read_ty, substitution);
            }
        }
    }

    pub(super) fn rewrite_pattern(
        &mut self,
        pattern: &mut Pattern,
        substitution: &InstanceSubstitution,
    ) {
        match pattern {
            Pattern::Is { ty, metadata } => {
                *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
                self.rewrite_pattern_type_test_metadata(metadata, substitution);
            }
            Pattern::Bind { ty, .. } => {
                *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
            }
            Pattern::Or { pats } => {
                for pat in pats {
                    self.rewrite_pattern(pat, substitution);
                }
            }
            Pattern::Tuple { elements } | Pattern::Variant { args: elements, .. } => {
                for pat in elements {
                    self.rewrite_pattern(pat, substitution);
                }
            }
            Pattern::Else
            | Pattern::Wildcard
            | Pattern::Rest
            | Pattern::IntLit { .. }
            | Pattern::CharLit { .. }
            | Pattern::StringLit { .. }
            | Pattern::BoolLit { .. } => {}
        }
    }

    pub(super) fn rewrite_perform_metadata(
        &mut self,
        metadata: &mut PerformMetadata,
        substitution: &InstanceSubstitution,
    ) {
        metadata.effect_ty =
            substitute_type_and_effect_params(&mut self.types, metadata.effect_ty, substitution);
        metadata.result_ty =
            substitute_type_and_effect_params(&mut self.types, metadata.result_ty, substitution);
        metadata.payload_tuple_ty = metadata
            .payload_tuple_ty
            .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution));
        for ty in &mut metadata.payload_component_tys {
            *ty = substitute_type_and_effect_params(&mut self.types, *ty, substitution);
        }
        for transport in &mut metadata.payload_transport {
            self.rewrite_value_transport(transport, substitution);
        }
    }

    pub(super) fn rewrite_value_transport(
        &mut self,
        transport: &mut ValueTransportMetadata,
        substitution: &InstanceSubstitution,
    ) {
        transport.source_ty =
            substitute_type_and_effect_params(&mut self.types, transport.source_ty, substitution);
        transport.requirements =
            super::super::lower::mir_transport_requirements(&self.types, transport.source_ty);
        if let Some(boxing) = &mut transport.boxing {
            boxing.source_ty =
                substitute_type_and_effect_params(&mut self.types, boxing.source_ty, substitution);
            boxing.target_ty = boxing
                .target_ty
                .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution));
        }
        if let Some(boxing) = &mut transport.boxing
            && matches!(
                boxing.reason,
                super::super::MirBoxingReason::AnyErasure
                    | super::super::MirBoxingReason::RefErasure
            )
            && let Some(target_ty) = boxing.target_ty
        {
            match super::super::lower::erasure_boxing_reason(
                self.builtins,
                &self.types,
                boxing.source_ty,
                target_ty,
            ) {
                Some(reason) => boxing.reason = reason,
                None => transport.boxing = None,
            }
        }
    }

    pub(super) fn rewrite_aggregate_transport(
        &mut self,
        transport: &mut AggregateTransportMetadata,
        substitution: &InstanceSubstitution,
    ) {
        transport.aggregate_ty = substitute_type_and_effect_params(
            &mut self.types,
            transport.aggregate_ty,
            substitution,
        );
        for field in &mut transport.fields {
            field.ty = substitute_type_and_effect_params(&mut self.types, field.ty, substitution);
            self.rewrite_value_transport(&mut field.transport, substitution);
        }
    }

    pub(super) fn rewrite_closure_env_contract(
        &mut self,
        contract: &mut ClosureEnvTransportMetadata,
        substitution: &InstanceSubstitution,
    ) {
        contract.env_ty =
            substitute_type_and_effect_params(&mut self.types, contract.env_ty, substitution);
        for capture in &mut contract.captures {
            self.rewrite_value_transport(&mut capture.transport, substitution);
        }
    }

    pub(super) fn rewrite_call_transport(
        &mut self,
        transport: &mut CallTransportMetadata,
        substitution: &InstanceSubstitution,
    ) {
        self.rewrite_value_transport(&mut transport.result, substitution);
        if let Some(aggregate_return) = &mut transport.aggregate_return {
            self.rewrite_value_transport(aggregate_return, substitution);
        }
        if let Some(array) = &mut transport.array {
            self.rewrite_array_transport(array, substitution);
        }
        if let Some(gc) = &mut transport.gc {
            self.rewrite_gc_intrinsic_transport(gc, substitution);
        }
    }

    pub(super) fn rewrite_gc_intrinsic_transport(
        &mut self,
        gc: &mut GcIntrinsicTransportMetadata,
        substitution: &InstanceSubstitution,
    ) {
        gc.subject_ty =
            substitute_type_and_effect_params(&mut self.types, gc.subject_ty, substitution);
        gc.token_ty = gc
            .token_ty
            .map(|ty| substitute_type_and_effect_params(&mut self.types, ty, substitution));
        self.rewrite_value_transport(&mut gc.subject, substitution);
    }

    pub(super) fn rewrite_array_transport(
        &mut self,
        array: &mut ArrayElementTransportMetadata,
        substitution: &InstanceSubstitution,
    ) {
        array.array_ty =
            substitute_type_and_effect_params(&mut self.types, array.array_ty, substitution);
        array.element_ty =
            substitute_type_and_effect_params(&mut self.types, array.element_ty, substitution);
        self.rewrite_value_transport(&mut array.element, substitution);
    }

    pub(super) fn rewrite_operand(&mut self, operand: Operand) -> Operand {
        operand
    }
}
