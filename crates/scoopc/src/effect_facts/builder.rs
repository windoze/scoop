use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::ast;
use crate::mir::{
    BasicBlockId, Body as MirBody, CallArg, CallKind, ConstValue, File as MirFile,
    FunDecl as MirFunDecl, HandleMetadata, HandlerArm, InstanceKey, Item as MirItem,
    MaterializedMir, Operand, PerformMetadata, ResumeMetadata, Rvalue, SiteId, StatementKind,
    TemplateKey, TerminatorKind, UnwindAction,
};
use crate::resolve::{FunOverload, Index};
use crate::session::Session;
use crate::source::SourceFile;
use crate::ty::{
    EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore, ValueTypeKind,
};
use crate::typecheck::{TypeEnv, TypeLowering, TypeSymbol};

use super::{
    BlockEffectFacts, BodyEffectFacts, BodyEffectSolverFacts, CallSiteEffectFacts, CallSiteKind,
    CallSiteTarget, CallableAbiKind, CallableEffectFacts, CaseSet, CaseTag, ConcreteOpKey,
    ContinuationSchema, ContinuationSchemaId, EffectFactsError, EffectPrecision,
    HandleArmEffectFacts, HandleSiteEffectFacts, HandleSiteSolverFacts, ImplPlan,
    MaterializedEffectFacts, MirSnapshotBinding, NestedHandleClassification,
    PerformSiteEffectFacts, ResumeSiteEffectFacts, SiteEffectFacts, StepCaseFact, StepSchema,
    StepSchemaId,
};

/// 从 canonical materialized MIR snapshot 生成 P4 facts 容器。
#[derive(Debug)]
pub struct MaterializedEffectFactsBuilder<'a> {
    session: &'a Session,
    source: &'a SourceFile,
    materialized: &'a mut MaterializedMir,
    compiler_continuation_runtime_error_callables: HashSet<InstanceKey>,
}

#[derive(Debug, Clone)]
struct SurfaceCallableContract {
    declared_row: EffectRow,
}

impl EffectFactsTypeContext {
    fn concrete_effect_op_contract_for_site(
        &self,
        types: &mut TypeStore,
        effect_ty: TypeId,
        op_fqn: &str,
    ) -> Result<ConcreteEffectOpContract, EffectFactsError> {
        let (effect_fqn, effect_type_args) = lower_effect_nominal_identity(types, effect_ty)?;
        let effect_sym = self.env.type_symbol(&effect_fqn).ok_or_else(|| {
            EffectFactsError::MissingEffectTypeSymbol {
                effect_fqn: effect_fqn.clone(),
            }
        })?;
        let op = effect_op_overloads(&self.index, &effect_fqn)
            .into_iter()
            .find_map(|(candidate_fqn, overload)| (candidate_fqn == op_fqn).then_some(overload))
            .ok_or_else(|| EffectFactsError::MalformedEffectOpSignature {
                op_fqn: op_fqn.to_string(),
                detail: "missing effect op overload",
            })?;
        self.lower_effect_op_contract(
            types,
            &effect_fqn,
            &effect_type_args,
            op_fqn,
            &op,
            effect_sym,
        )
    }

    fn known_callable_instance_key(
        &self,
        fqn: &str,
        explicit_arg_count: usize,
        has_receiver: bool,
    ) -> Option<InstanceKey> {
        let overload = self.select_fun_overload(fqn, explicit_arg_count, has_receiver)?;
        Some(InstanceKey {
            template: TemplateKey {
                fqn: fqn.to_string(),
                source_path: overload.symbol.decl_file.clone(),
                decl_span: overload.symbol.span,
            },
            type_args: Vec::new(),
            eff_args: Vec::new(),
        })
    }

    fn virtual_dispatch_candidate_fqns(
        &self,
        types: &TypeStore,
        receiver_ty: TypeId,
        member_name: &str,
        explicit_arg_count: usize,
    ) -> Vec<String> {
        let Some(receiver_fqn) = nominal_type_fqn(types, receiver_ty) else {
            return Vec::new();
        };
        let mut targets = BTreeSet::new();
        for class_fqn in self.descendants_and_self(receiver_fqn) {
            if let Some(slot) = self
                .class_vtables
                .get(class_fqn.as_str())
                .and_then(|slots| {
                    slots.iter().find(|slot| {
                        slot.name == member_name && slot.params_len == explicit_arg_count as u32
                    })
                })
            {
                targets.insert(slot.impl_member_fqn.clone());
            } else if class_fqn == receiver_fqn {
                targets.insert(format!("{class_fqn}.{member_name}"));
            }
        }
        targets.into_iter().collect()
    }

    fn interface_dispatch_candidate_fqns(
        &self,
        types: &TypeStore,
        receiver_ty: TypeId,
        owner_fqn: &str,
        member_name: &str,
        explicit_arg_count: usize,
    ) -> Vec<String> {
        let Some(interface) = self.interfaces.get(owner_fqn) else {
            return Vec::new();
        };
        let mut matching_slots = interface.method_slots.iter().filter(|slot| {
            slot.name == member_name && slot.params_len == explicit_arg_count as u32
        });
        let Some(slot) = matching_slots.next() else {
            return Vec::new();
        };
        if matching_slots.next().is_some() {
            return Vec::new();
        }

        let mut targets = BTreeSet::new();
        if let Some(receiver_fqn) = nominal_type_fqn(types, receiver_ty)
            && let Some(entries) = self.class_itables.get(receiver_fqn)
        {
            collect_interface_slot_targets(entries, owner_fqn, slot.slot as usize, &mut targets);
        }
        if targets.is_empty() {
            for entries in self.class_itables.values() {
                collect_interface_slot_targets(
                    entries,
                    owner_fqn,
                    slot.slot as usize,
                    &mut targets,
                );
            }
        }
        targets.into_iter().collect()
    }

    fn descendants_and_self(&self, root: &str) -> BTreeSet<String> {
        let mut seen = BTreeSet::from([root.to_string()]);
        let mut stack = vec![root.to_string()];
        while let Some(current) = stack.pop() {
            if let Some(children) = self.direct_subclasses.get(&current) {
                for child in children {
                    if seen.insert(child.clone()) {
                        stack.push(child.clone());
                    }
                }
            }
        }
        seen
    }

    fn select_fun_overload(
        &self,
        fqn: &str,
        explicit_arg_count: usize,
        has_receiver: bool,
    ) -> Option<&FunOverload> {
        let mut matches = self.matching_fun_overloads(fqn, explicit_arg_count, has_receiver);
        let first = matches.pop()?;
        matches.is_empty().then_some(first)
    }

    fn matching_fun_overloads(
        &self,
        fqn: &str,
        explicit_arg_count: usize,
        has_receiver: bool,
    ) -> Vec<&FunOverload> {
        let Some(entry) = self.index.by_fqn.get(fqn) else {
            return Vec::new();
        };
        let overloads = &entry.fun;
        let exact = overloads
            .iter()
            .filter(|overload| {
                overload.sig.params.len() == explicit_arg_count
                    && overload.sig.receiver.is_some() == has_receiver
            })
            .collect::<Vec<_>>();
        if !exact.is_empty() {
            return exact;
        }

        if !has_receiver {
            return overloads
                .iter()
                .filter(|overload| {
                    overload.sig.receiver.is_some()
                        && overload.sig.params.len().saturating_add(1) == explicit_arg_count
                })
                .collect();
        }

        // class/interface member fun 在索引里不一定把 owner 记作显式 receiver；对 declaration-only
        // member fallback surface contract，允许按 owner-qualified FQN + 参数个数直接匹配。
        overloads
            .iter()
            .filter(|overload| overload.sig.params.len() == explicit_arg_count)
            .collect()
    }

    fn surface_callable_contract(
        &self,
        types: &mut TypeStore,
        fqn: &str,
        explicit_arg_count: usize,
        has_receiver: bool,
    ) -> Option<SurfaceCallableContract> {
        let builtins = types.intern_builtins();
        let mut terms = Vec::new();
        let mut saw_match = false;
        for overload in self.matching_fun_overloads(fqn, explicit_arg_count, has_receiver) {
            saw_match = true;
            let decl_source = self.env.source(&overload.symbol.decl_file)?;
            let file_ctx = self.env.file_type_context(&overload.symbol.decl_file)?;
            let mut lower = TypeLowering::new_with_ctx(
                decl_source,
                &self.index,
                &self.env,
                types,
                builtins,
                file_ctx.pkg_prefix.clone(),
                file_ctx.imports.clone(),
            );
            let declared_row = lower
                .lower_effect_row_expr_in_decl_file_with_scopes(
                    &overload.symbol.decl_file,
                    std::iter::empty::<(String, TypeId)>(),
                    std::iter::empty::<(String, EffectRow)>(),
                    overload.sig.effects.as_ref(),
                )
                .ok()?;
            terms.extend(declared_row.terms);
        }
        if !saw_match {
            return None;
        }
        Some(SurfaceCallableContract {
            declared_row: EffectRow::new(terms),
        })
    }
}

impl<'a> EffectFactsSchemaPool<'a> {
    fn new(type_ctx: &'a EffectFactsTypeContext) -> Self {
        Self {
            type_ctx,
            step_schemas: BTreeMap::new(),
            continuation_schemas: BTreeMap::new(),
            continuation_schema_ids: BTreeMap::new(),
            synthetic_step_schema_ids: HashMap::new(),
            next_step_schema_id: 0,
            next_continuation_schema_id: 0,
        }
    }

    fn finish(
        self,
    ) -> (
        BTreeMap<StepSchemaId, StepSchema>,
        BTreeMap<ContinuationSchemaId, ContinuationSchema>,
    ) {
        (self.step_schemas, self.continuation_schemas)
    }

    fn step_schema(&self, id: StepSchemaId) -> &StepSchema {
        self.step_schemas
            .get(&id)
            .expect("referenced step schema should exist")
    }

    fn full_case_set(&self, step_schema: StepSchemaId) -> CaseSet {
        CaseSet::new(
            step_schema,
            self.step_schema(step_schema)
                .cases()
                .iter()
                .map(|case| case.case_tag())
                .collect(),
        )
    }

    fn intern_callable_step_schema(
        &mut self,
        types: &mut TypeStore,
        seed: &CallableSeed,
    ) -> Result<StepSchemaId, EffectFactsError> {
        let invoke_args_tuple_ty = canonical_tuple_carrier_ty(types, &seed.invoke_arg_components);
        let continuation_obj_ty = continuation_object_ty(types, &seed.key);
        self.intern_step_schema(
            types,
            invoke_args_tuple_ty,
            seed.complete_ty,
            continuation_obj_ty,
            &seed.step_effect_row,
            &seed.surface_effect_row,
        )
    }

    fn intern_synthetic_step_schema(
        &mut self,
        types: &mut TypeStore,
        invoke_args_tuple_ty: TypeId,
        complete_ty: TypeId,
        effect_row: &EffectRow,
        continuation_surface_row: &EffectRow,
        kind: SyntheticStepSchemaKind,
    ) -> Result<StepSchemaId, EffectFactsError> {
        let key = SyntheticStepSchemaKey {
            invoke_args_tuple_ty,
            complete_ty,
            effect_row: effect_row.clone(),
            kind,
        };
        if let Some(id) = self.synthetic_step_schema_ids.get(&key) {
            return Ok(*id);
        }
        let continuation_obj_ty = synthetic_continuation_object_ty(
            types,
            kind,
            invoke_args_tuple_ty,
            complete_ty,
            effect_row,
        );
        let id = self.intern_step_schema(
            types,
            invoke_args_tuple_ty,
            complete_ty,
            continuation_obj_ty,
            effect_row,
            continuation_surface_row,
        )?;
        self.synthetic_step_schema_ids.insert(key, id);
        Ok(id)
    }

    fn project_case_set(
        &self,
        source: &CaseSet,
        current_case_index: &HashMap<ConcreteOpKey, CurrentBodyCaseInfo>,
    ) -> BTreeSet<CaseTag> {
        let mut projected = BTreeSet::new();
        for tag in source.tags() {
            let Some(source_case) = self
                .step_schema(source.schema())
                .cases()
                .iter()
                .find(|case| case.case_tag() == *tag)
            else {
                continue;
            };
            if let Some(target) = current_case_index.get(source_case.concrete_op_key()) {
                projected.insert(target.tag);
            }
        }
        projected
    }

    fn intern_step_schema(
        &mut self,
        types: &mut TypeStore,
        invoke_args_tuple_ty: TypeId,
        complete_ty: TypeId,
        continuation_obj_ty: TypeId,
        effect_row: &EffectRow,
        continuation_surface_row: &EffectRow,
    ) -> Result<StepSchemaId, EffectFactsError> {
        let step_schema_id = StepSchemaId::new(self.next_step_schema_id);
        self.next_step_schema_id += 1;

        let case_seeds = self.type_ctx.step_case_seeds(types, effect_row)?;
        let mut cases = Vec::with_capacity(case_seeds.len());
        for (case_index, case_seed) in case_seeds.into_iter().enumerate() {
            let case_tag = CaseTag::new(case_index as u32);
            let surface_ty = continuation_surface_ty(
                types,
                case_seed.resume_tuple_ty,
                complete_ty,
                continuation_surface_row,
            );
            let continuation_schema = self.intern_continuation_schema(
                case_seed.resume_tuple_ty,
                complete_ty,
                step_schema_id,
                surface_ty,
            );
            cases.push(StepCaseFact::new(
                case_tag,
                case_seed.concrete_op_key,
                case_seed.payload_tuple_ty,
                continuation_schema,
            ));
        }

        self.step_schemas.insert(
            step_schema_id,
            StepSchema::new(
                invoke_args_tuple_ty,
                complete_ty,
                continuation_obj_ty,
                cases,
            ),
        );
        Ok(step_schema_id)
    }

    fn intern_continuation_schema(
        &mut self,
        resume_tuple_ty: TypeId,
        answer_ty: TypeId,
        out_step_schema: StepSchemaId,
        surface_ty: TypeId,
    ) -> ContinuationSchemaId {
        let key = ContinuationSchemaKey {
            resume_tuple_ty,
            answer_ty,
            out_step_schema,
            surface_ty,
        };
        if let Some(id) = self.continuation_schema_ids.get(&key) {
            return *id;
        }
        let id = ContinuationSchemaId::new(self.next_continuation_schema_id);
        self.next_continuation_schema_id += 1;
        self.continuation_schemas.insert(
            id,
            ContinuationSchema::new(
                key.resume_tuple_ty,
                key.answer_ty,
                key.out_step_schema,
                key.surface_ty,
            ),
        );
        self.continuation_schema_ids.insert(key, id);
        id
    }
}

impl RegionCaseContribution {
    fn add_tags(&mut self, is_cleanup: bool, tags: impl IntoIterator<Item = CaseTag>) {
        let bucket = if is_cleanup {
            &mut self.cleanup
        } else {
            &mut self.non_cleanup
        };
        bucket.extend(tags);
    }

    fn extend(&mut self, other: Self) {
        self.non_cleanup.extend(other.non_cleanup);
        self.cleanup.extend(other.cleanup);
    }

    fn total_tags(&self) -> BTreeSet<CaseTag> {
        let mut tags = self.non_cleanup.clone();
        tags.extend(self.cleanup.iter().copied());
        tags
    }
}

impl<'a, 'b> BodyFactsBuilder<'a, 'b> {
    fn new(
        type_ctx: &'a EffectFactsTypeContext,
        schema_pool: &'b mut EffectFactsSchemaPool<'a>,
        owner_by_callable_fqn: &'a HashMap<String, InstanceKey>,
        callable_facts: &'a HashMap<InstanceKey, CallableEffectFacts>,
        raw_fun_by_fqn: &'a HashMap<String, MirFunDecl>,
        callable_fun: &'a MirFunDecl,
        callable_step_schema: StepSchemaId,
    ) -> Result<Self, EffectFactsError> {
        let current_case_index = schema_pool
            .step_schema(callable_step_schema)
            .cases()
            .iter()
            .map(|case| {
                (
                    case.concrete_op_key().clone(),
                    CurrentBodyCaseInfo {
                        tag: case.case_tag(),
                        continuation_schema: case.continuation_schema(),
                    },
                )
            })
            .collect();
        Ok(Self {
            type_ctx,
            schema_pool,
            owner_by_callable_fqn,
            callable_facts,
            raw_fun_by_fqn,
            callable_fun,
            callable_step_schema,
            current_case_index,
            sites: BTreeMap::new(),
            block_drafts: BTreeMap::new(),
            block_scan_cache: BTreeMap::new(),
            block_site_ids: BTreeMap::new(),
            block_handled_tags: BTreeMap::new(),
            handle_site_solver_facts: BTreeMap::new(),
        })
    }

    fn build(mut self, types: &mut TypeStore) -> Result<BodyEffectFacts, EffectFactsError> {
        let Some((body_len, block_successors, cleanup_blocks)) =
            self.callable_fun.body.as_ref().map(|body| {
                (
                    body.blocks.len(),
                    collect_block_successors(body),
                    body.blocks
                        .iter()
                        .enumerate()
                        .filter_map(|(index, block)| {
                            block
                                .is_cleanup
                                .then_some(BasicBlockId::from_raw(index as u32))
                        })
                        .collect::<BTreeSet<_>>(),
                )
            })
        else {
            return Ok(BodyEffectFacts::default());
        };

        for block_index in 0..body_len {
            let block_id = BasicBlockId::from_raw(block_index as u32);
            let _ = self.scan_block_sites(types, block_id)?;
        }

        let mut blocks = BTreeMap::new();
        for block_index in 0..body_len {
            let block_id = BasicBlockId::from_raw(block_index as u32);
            let draft = self.block_drafts.remove(&block_id).unwrap_or_default();
            blocks.insert(
                block_id,
                BlockEffectFacts::new(
                    self.empty_case_set(),
                    self.case_set_from_tags(draft.outward_tags),
                    draft.has_suspend_boundary,
                    draft.has_handle_boundary,
                ),
            );
        }

        let block_handled_cases = std::mem::take(&mut self.block_handled_tags)
            .into_iter()
            .map(|(block_id, tags)| (block_id, self.case_set_from_tags(tags)))
            .collect();
        let solver_facts = BodyEffectSolverFacts::new(
            block_successors,
            std::mem::take(&mut self.block_site_ids),
            block_handled_cases,
            cleanup_blocks,
            std::mem::take(&mut self.handle_site_solver_facts),
        );

        Ok(BodyEffectFacts::with_solver_facts(
            blocks,
            self.sites,
            None,
            solver_facts,
        ))
    }

    fn record_block_site(&mut self, block_id: BasicBlockId, site_id: SiteId) {
        let sites = self.block_site_ids.entry(block_id).or_default();
        if !sites.contains(&site_id) {
            sites.push(site_id);
        }
    }

    fn mark_region_handled_cases(
        &mut self,
        entry: BasicBlockId,
        stops: &BTreeSet<BasicBlockId>,
        tags: &BTreeSet<CaseTag>,
        visited: &mut BTreeSet<BasicBlockId>,
    ) {
        if stops.contains(&entry) || !visited.insert(entry) {
            return;
        }

        let block = self.body().blocks[entry.as_u32() as usize].clone();
        if !block.is_cleanup {
            self.block_handled_tags
                .entry(entry)
                .or_default()
                .extend(tags.iter().copied());
        }

        block.terminator.for_each_successor(|target| {
            self.mark_region_handled_cases(target, stops, tags, visited);
        });
    }

    fn scan_block_sites(
        &mut self,
        types: &mut TypeStore,
        block_id: BasicBlockId,
    ) -> Result<RegionCaseContribution, EffectFactsError> {
        if let Some(cached) = self.block_scan_cache.get(&block_id) {
            return Ok(cached.clone());
        }

        let block = self.body().blocks[block_id.as_u32() as usize].clone();
        let mut direct = RegionCaseContribution::default();
        let mut draft = BlockDraft::default();

        for stmt in &block.stmts {
            let StatementKind::Assign { target, value } = &stmt.kind else {
                continue;
            };
            let Rvalue::Call {
                site_id,
                kind,
                args,
            } = value
            else {
                continue;
            };
            self.record_block_site(block_id, *site_id);
            match kind {
                CallKind::Resume { resume, .. } => {
                    let projected = self.ensure_resume_site_facts(types, *site_id, resume)?;
                    if !projected.is_empty() {
                        draft.has_suspend_boundary = true;
                    }
                    direct.add_tags(block.is_cleanup, projected);
                }
                _ => {
                    self.ensure_call_site_facts(types, *target, *site_id, kind, args)?;
                }
            }
        }

        match &block.terminator.kind {
            TerminatorKind::Perform {
                site_id,
                op_fqn,
                metadata,
                ..
            } => {
                self.record_block_site(block_id, *site_id);
                let emitted = self.ensure_perform_site_facts(types, *site_id, op_fqn, metadata)?;
                direct.add_tags(block.is_cleanup, [emitted]);
                draft.has_suspend_boundary = true;
            }
            TerminatorKind::Handle {
                site_id,
                metadata,
                arms,
                body_target,
                arm_targets,
                finally_target,
                exit_target,
                ..
            } => {
                self.record_block_site(block_id, *site_id);
                let outward = self.ensure_handle_site_facts(
                    types,
                    *site_id,
                    metadata,
                    arms,
                    *body_target,
                    arm_targets,
                    *finally_target,
                    *exit_target,
                )?;
                direct.add_tags(block.is_cleanup, outward);
                draft.has_handle_boundary = true;
                if matches!(
                    self.sites.get(site_id),
                    Some(SiteEffectFacts::Handle(facts))
                        if facts.nested_handle_classification()
                            == NestedHandleClassification::MaySuspendOutward
                ) {
                    draft.has_suspend_boundary = true;
                }
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Goto { .. }
            | TerminatorKind::CondBr { .. }
            | TerminatorKind::Unreachable
            | TerminatorKind::Todo(_) => {}
        }

        draft.outward_tags.extend(direct.total_tags());
        self.block_drafts.insert(block_id, draft);
        self.block_scan_cache.insert(block_id, direct.clone());
        Ok(direct)
    }

    fn collect_region_cases(
        &mut self,
        types: &mut TypeStore,
        entry: BasicBlockId,
        stops: &BTreeSet<BasicBlockId>,
        visited: &mut BTreeSet<BasicBlockId>,
    ) -> Result<RegionCaseContribution, EffectFactsError> {
        if stops.contains(&entry) || !visited.insert(entry) {
            return Ok(RegionCaseContribution::default());
        }

        let block = self.body().blocks[entry.as_u32() as usize].clone();
        let mut acc = self.scan_block_sites(types, entry)?;

        match &block.terminator.kind {
            TerminatorKind::Goto { target } => {
                acc.extend(self.collect_region_cases(types, *target, stops, visited)?);
            }
            TerminatorKind::CondBr {
                then_target,
                else_target,
                ..
            } => {
                acc.extend(self.collect_region_cases(types, *then_target, stops, visited)?);
                acc.extend(self.collect_region_cases(types, *else_target, stops, visited)?);
            }
            TerminatorKind::Perform { resume_target, .. } => {
                acc.extend(self.collect_region_cases(types, *resume_target, stops, visited)?);
                if let UnwindAction::Cleanup { target } = block.terminator.unwind {
                    acc.extend(self.collect_region_cases(types, target, stops, visited)?);
                }
            }
            TerminatorKind::Handle { exit_target, .. } => {
                acc.extend(self.collect_region_cases(types, *exit_target, stops, visited)?);
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Unreachable
            | TerminatorKind::Todo(_) => {}
        }
        Ok(acc)
    }

    fn ensure_call_site_facts(
        &mut self,
        types: &mut TypeStore,
        result_local: crate::mir::LocalId,
        site_id: SiteId,
        kind: &CallKind,
        args: &[CallArg],
    ) -> Result<(), EffectFactsError> {
        if self.sites.contains_key(&site_id) {
            return Ok(());
        }
        let facts = self.build_call_site_effect_facts(types, result_local, kind, args)?;
        self.sites.insert(site_id, SiteEffectFacts::Call(facts));
        Ok(())
    }

    fn ensure_resume_site_facts(
        &mut self,
        types: &mut TypeStore,
        site_id: SiteId,
        resume: &ResumeMetadata,
    ) -> Result<BTreeSet<CaseTag>, EffectFactsError> {
        if let Some(SiteEffectFacts::Resume(facts)) = self.sites.get(&site_id) {
            return Ok(self
                .schema_pool
                .project_case_set(facts.resolved_cases(), &self.current_case_index));
        }

        let mut out_terms = resume.out_effects.terms.clone();
        if let Some(runtime_error_effect_ty) = resume.runtime_error_effect_ty {
            out_terms.push(runtime_error_effect_ty);
        }
        let out_row = EffectRow::new(out_terms);
        let out_step_schema = self.schema_pool.intern_synthetic_step_schema(
            types,
            resume.resume_ty,
            resume.answer_ty,
            &out_row,
            &resume.out_effects,
            SyntheticStepSchemaKind::ResumeSurface,
        )?;
        let continuation_schema = self.schema_pool.intern_continuation_schema(
            resume.resume_ty,
            resume.answer_ty,
            out_step_schema,
            resume.continuation_ty,
        );
        let resolved_cases = self.schema_pool.full_case_set(out_step_schema);
        let projected = self
            .schema_pool
            .project_case_set(&resolved_cases, &self.current_case_index);
        self.sites.insert(
            site_id,
            SiteEffectFacts::Resume(ResumeSiteEffectFacts::new(
                continuation_schema,
                resume.resume_ty,
                resume.answer_ty,
                out_step_schema,
                resolved_cases,
            )),
        );
        Ok(projected)
    }

    fn ensure_perform_site_facts(
        &mut self,
        types: &mut TypeStore,
        site_id: SiteId,
        op_fqn: &str,
        metadata: &PerformMetadata,
    ) -> Result<CaseTag, EffectFactsError> {
        if let Some(SiteEffectFacts::Perform(facts)) = self.sites.get(&site_id) {
            return Ok(facts.emitted_case());
        }

        let case_info = self.current_case_for_effect_op(types, metadata.effect_ty, op_fqn)?;
        let payload_tuple_ty = metadata
            .payload_tuple_ty
            .unwrap_or_else(|| canonical_tuple_carrier_ty(types, &metadata.payload_component_tys));
        self.sites.insert(
            site_id,
            SiteEffectFacts::Perform(PerformSiteEffectFacts::new(
                case_info.tag,
                payload_tuple_ty,
                case_info.continuation_schema,
            )),
        );
        Ok(case_info.tag)
    }

    #[allow(clippy::too_many_arguments)]
    fn ensure_handle_site_facts(
        &mut self,
        types: &mut TypeStore,
        site_id: SiteId,
        metadata: &HandleMetadata,
        arms: &[HandlerArm],
        body_target: BasicBlockId,
        arm_targets: &[BasicBlockId],
        finally_target: Option<BasicBlockId>,
        exit_target: BasicBlockId,
    ) -> Result<BTreeSet<CaseTag>, EffectFactsError> {
        self.handle_site_solver_facts.insert(
            site_id,
            HandleSiteSolverFacts::new(
                body_target,
                arm_targets.to_vec(),
                finally_target,
                exit_target,
            ),
        );
        if let Some(SiteEffectFacts::Handle(facts)) = self.sites.get(&site_id) {
            return Ok(handle_total_outward_tags(facts));
        }

        let mut body_stops = BTreeSet::from([exit_target]);
        if let Some(finally_target) = finally_target {
            body_stops.insert(finally_target);
        }
        let body_cases =
            self.collect_region_cases(types, body_target, &body_stops, &mut BTreeSet::new())?;

        let mut handled_tags = BTreeSet::new();
        let mut arm_facts = Vec::with_capacity(arms.len());
        let mut arm_non_cleanup = BTreeSet::new();
        let mut cleanup_outward = body_cases.cleanup.clone();

        for (arm, arm_target) in arms.iter().zip(arm_targets.iter().copied()) {
            let case_info =
                self.current_case_for_effect_op(types, arm.handled_effect_ty, &arm.op_fqn)?;
            handled_tags.insert(case_info.tag);

            let arm_cases =
                self.collect_region_cases(types, arm_target, &body_stops, &mut BTreeSet::new())?;
            cleanup_outward.extend(arm_cases.cleanup.iter().copied());
            arm_non_cleanup.extend(arm_cases.non_cleanup.iter().copied());

            let payload_tuple_ty = arm
                .payload_tuple_ty
                .unwrap_or_else(|| canonical_tuple_carrier_ty(types, &arm.payload_component_tys));
            arm_facts.push(HandleArmEffectFacts::new(
                case_info.tag,
                payload_tuple_ty,
                case_info.continuation_schema,
                self.case_set_from_tags(arm_cases.non_cleanup),
            ));
        }

        self.mark_region_handled_cases(
            body_target,
            &body_stops,
            &handled_tags,
            &mut BTreeSet::new(),
        );

        let finally_cases = if let Some(finally_target) = finally_target {
            self.collect_region_cases(
                types,
                finally_target,
                &BTreeSet::from([exit_target]),
                &mut BTreeSet::new(),
            )?
        } else {
            RegionCaseContribution::default()
        };

        let body_outward = body_cases
            .non_cleanup
            .difference(&handled_tags)
            .copied()
            .collect::<BTreeSet<_>>();
        cleanup_outward.extend(finally_cases.total_tags());

        let classification = if body_outward.is_empty()
            && arm_non_cleanup.is_empty()
            && cleanup_outward.is_empty()
        {
            NestedHandleClassification::SelfContained
        } else {
            NestedHandleClassification::MaySuspendOutward
        };

        let facts = HandleSiteEffectFacts::new(
            metadata.result_ty,
            self.case_set_from_tags(handled_tags.clone()),
            self.case_set_from_tags(body_outward.clone()),
            arm_facts,
            self.case_set_from_tags(cleanup_outward.clone()),
            classification,
        );
        let mut total_outward = body_outward;
        total_outward.extend(arm_non_cleanup);
        total_outward.extend(cleanup_outward.iter().copied());
        self.sites.insert(site_id, SiteEffectFacts::Handle(facts));
        Ok(total_outward)
    }

    fn build_call_site_effect_facts(
        &mut self,
        types: &mut TypeStore,
        result_local: crate::mir::LocalId,
        kind: &CallKind,
        args: &[CallArg],
    ) -> Result<CallSiteEffectFacts, EffectFactsError> {
        let arg_tys = args
            .iter()
            .map(|arg| operand_ty(self.body(), types, &arg.value))
            .collect::<Vec<_>>();
        let invoke_args_tuple_ty = canonical_tuple_carrier_ty(types, &arg_tys);
        let result_ty = self.body().locals[result_local.as_u32() as usize].ty;
        match kind {
            CallKind::Direct { callee_fqn } => self.build_direct_like_call_site(
                types,
                CallSiteKind::Direct,
                callee_fqn,
                args.len(),
                invoke_args_tuple_ty,
                result_ty,
                None,
            ),
            CallKind::Closure { fn_ptr, .. } => self.build_direct_like_call_site(
                types,
                CallSiteKind::Closure,
                fn_ptr,
                args.len(),
                invoke_args_tuple_ty,
                result_ty,
                None,
            ),
            CallKind::FunValue { callee } => {
                if let Some(facts) = self.build_builtin_fun_value_call_site(
                    types,
                    callee,
                    invoke_args_tuple_ty,
                    result_ty,
                )? {
                    return Ok(facts);
                }
                let callee_ty = operand_ty(self.body(), types, callee);
                if let Some(contract) = function_surface_contract_from_ty(types, callee_ty)
                    && contract.declared_row.is_pure()
                {
                    return Ok(CallSiteEffectFacts::new_plain(
                        CallSiteKind::FunValue,
                        CallSiteTarget::DynamicFallback,
                        invoke_args_tuple_ty,
                        CaseSet::new(self.callable_step_schema, Vec::new()),
                        EffectPrecision::Precise,
                    ));
                }
                let (step_schema, resolved_cases) =
                    if let Some(contract) = function_surface_contract_from_ty(types, callee_ty) {
                        let schema = self.schema_pool.intern_synthetic_step_schema(
                            types,
                            invoke_args_tuple_ty,
                            result_ty,
                            &contract.declared_row,
                            &contract.declared_row,
                            SyntheticStepSchemaKind::CallSurface,
                        )?;
                        (schema, self.schema_pool.full_case_set(schema))
                    } else {
                        (
                            self.callable_step_schema,
                            self.schema_pool.full_case_set(self.callable_step_schema),
                        )
                    };
                Ok(CallSiteEffectFacts::new(
                    CallSiteKind::FunValue,
                    CallSiteTarget::DynamicFallback,
                    invoke_args_tuple_ty,
                    step_schema,
                    resolved_cases,
                    EffectPrecision::SignatureFallback,
                ))
            }
            CallKind::Virtual { dispatch, .. } => self.build_dispatch_call_site(
                types,
                CallSiteKind::Virtual,
                &self.type_ctx.virtual_dispatch_candidate_fqns(
                    types,
                    dispatch.receiver_ty,
                    &dispatch.member_name,
                    args.len(),
                ),
                &format!("{}.{}", dispatch.owner_fqn, dispatch.member_name),
                args.len(),
                invoke_args_tuple_ty,
                result_ty,
                Some(dispatch.receiver_ty),
            ),
            CallKind::Interface { dispatch, .. } => self.build_dispatch_call_site(
                types,
                CallSiteKind::Interface,
                &self.type_ctx.interface_dispatch_candidate_fqns(
                    types,
                    dispatch.receiver_ty,
                    &dispatch.owner_fqn,
                    &dispatch.member_name,
                    args.len(),
                ),
                &format!("{}.{}", dispatch.owner_fqn, dispatch.member_name),
                args.len(),
                invoke_args_tuple_ty,
                result_ty,
                Some(dispatch.receiver_ty),
            ),
            CallKind::Resume { .. } => unreachable!("resume call sites are handled separately"),
        }
    }

    fn build_builtin_fun_value_call_site(
        &mut self,
        types: &mut TypeStore,
        callee: &Operand,
        invoke_args_tuple_ty: TypeId,
        _result_ty: TypeId,
    ) -> Result<Option<CallSiteEffectFacts>, EffectFactsError> {
        if !self.fun_value_callee_is_builtin_string_concat(types, callee) {
            return Ok(None);
        }
        Ok(Some(CallSiteEffectFacts::new_plain(
            CallSiteKind::FunValue,
            CallSiteTarget::DynamicFallback,
            invoke_args_tuple_ty,
            CaseSet::new(self.callable_step_schema, Vec::new()),
            EffectPrecision::Precise,
        )))
    }

    fn fun_value_callee_is_builtin_string_concat(
        &self,
        types: &TypeStore,
        callee: &Operand,
    ) -> bool {
        let Operand::Local(callee_local) = callee else {
            return false;
        };
        self.body().blocks.iter().any(|block| {
            block.stmts.iter().any(|stmt| {
                let StatementKind::Assign { target, value } = &stmt.kind else {
                    return false;
                };
                if target != callee_local {
                    return false;
                }
                let Rvalue::MemberAccess { member, .. } = value else {
                    return false;
                };
                member.name == "concat"
                    && matches!(
                        types.kind(member.receiver_ty),
                        TypeKind::Ref(RefTypeKind::String)
                    )
            })
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn build_direct_like_call_site(
        &mut self,
        types: &mut TypeStore,
        kind: CallSiteKind,
        callable_fqn: &str,
        explicit_arg_count: usize,
        invoke_args_tuple_ty: TypeId,
        result_ty: TypeId,
        receiver_ty: Option<TypeId>,
    ) -> Result<CallSiteEffectFacts, EffectFactsError> {
        if is_plain_compiler_intrinsic(callable_fqn) {
            return Ok(CallSiteEffectFacts::new_plain(
                kind,
                CallSiteTarget::DynamicFallback,
                invoke_args_tuple_ty,
                CaseSet::new(self.callable_step_schema, Vec::new()),
                EffectPrecision::Precise,
            ));
        }

        if let Some(target_key) =
            self.known_callable_key(callable_fqn, explicit_arg_count, receiver_ty.is_some())
        {
            if let Some(facts) = self.callable_facts.get(&target_key) {
                if matches!(facts.call_abi_kind(), CallableAbiKind::Plain) {
                    return Ok(CallSiteEffectFacts::new_plain(
                        kind,
                        CallSiteTarget::KnownInstance(target_key),
                        invoke_args_tuple_ty,
                        CaseSet::new(self.callable_step_schema, Vec::new()),
                        EffectPrecision::Precise,
                    ));
                }
                // P4-T03 只发布 callable shell 的保守上界；直到 P4-T04 回填求解结果前，
                // 只有空 case-set 才能提前视为精确，其余 known-instance call site 必须保守标宽。
                let precision = if facts.resolved_outward_cases().is_empty() {
                    EffectPrecision::Precise
                } else {
                    EffectPrecision::Widened
                };
                return Ok(CallSiteEffectFacts::new(
                    kind,
                    CallSiteTarget::KnownInstance(target_key),
                    invoke_args_tuple_ty,
                    facts.step_schema(),
                    facts.resolved_outward_cases().clone(),
                    precision,
                ));
            }

            let declared_row = if let Some(raw_fun) = self.raw_fun_by_fqn.get(callable_fqn) {
                declared_effect_row(raw_fun, types)
            } else if let Some(contract) = self.callable_value_surface_contract(types, callable_fqn)
            {
                contract.declared_row
            } else if self.callable_value_reference_exists(callable_fqn) {
                return Ok(self.dynamic_callable_value_fallback(kind, invoke_args_tuple_ty));
            } else {
                self.type_ctx
                    .surface_callable_contract(
                        types,
                        callable_fqn,
                        explicit_arg_count,
                        receiver_ty.is_some(),
                    )
                    .ok_or_else(|| EffectFactsError::MissingCallableSurfaceContract {
                        callable: callable_fqn.to_string(),
                    })?
                    .declared_row
            };
            if declared_row.is_pure() {
                return Ok(CallSiteEffectFacts::new_plain(
                    kind,
                    CallSiteTarget::KnownInstance(target_key),
                    invoke_args_tuple_ty,
                    CaseSet::new(self.callable_step_schema, Vec::new()),
                    EffectPrecision::Precise,
                ));
            }
            let step_schema = self.schema_pool.intern_synthetic_step_schema(
                types,
                invoke_args_tuple_ty,
                result_ty,
                &declared_row,
                &declared_row,
                SyntheticStepSchemaKind::CallSurface,
            )?;
            return Ok(CallSiteEffectFacts::new(
                kind,
                CallSiteTarget::KnownInstance(target_key),
                invoke_args_tuple_ty,
                step_schema,
                self.schema_pool.full_case_set(step_schema),
                EffectPrecision::SignatureFallback,
            ));
        }

        let declared_row = if let Some(raw_fun) = self.raw_fun_by_fqn.get(callable_fqn) {
            declared_effect_row(raw_fun, types)
        } else if let Some(contract) = self.callable_value_surface_contract(types, callable_fqn) {
            contract.declared_row
        } else if self.callable_value_reference_exists(callable_fqn) {
            return Ok(self.dynamic_callable_value_fallback(kind, invoke_args_tuple_ty));
        } else {
            self.type_ctx
                .surface_callable_contract(
                    types,
                    callable_fqn,
                    explicit_arg_count,
                    receiver_ty.is_some(),
                )
                .ok_or_else(|| EffectFactsError::MissingCallableSurfaceContract {
                    callable: callable_fqn.to_string(),
                })?
                .declared_row
        };
        if declared_row.is_pure() {
            return Ok(CallSiteEffectFacts::new_plain(
                kind,
                CallSiteTarget::DynamicFallback,
                invoke_args_tuple_ty,
                CaseSet::new(self.callable_step_schema, Vec::new()),
                EffectPrecision::SignatureFallback,
            ));
        }
        let step_schema = self.schema_pool.intern_synthetic_step_schema(
            types,
            invoke_args_tuple_ty,
            result_ty,
            &declared_row,
            &declared_row,
            SyntheticStepSchemaKind::CallSurface,
        )?;
        Ok(CallSiteEffectFacts::new(
            kind,
            CallSiteTarget::DynamicFallback,
            invoke_args_tuple_ty,
            step_schema,
            self.schema_pool.full_case_set(step_schema),
            EffectPrecision::SignatureFallback,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn build_dispatch_call_site(
        &mut self,
        types: &mut TypeStore,
        kind: CallSiteKind,
        candidate_fqns: &[String],
        fallback_fqn: &str,
        explicit_arg_count: usize,
        invoke_args_tuple_ty: TypeId,
        result_ty: TypeId,
        receiver_ty: Option<TypeId>,
    ) -> Result<CallSiteEffectFacts, EffectFactsError> {
        let candidate_keys =
            self.resolve_candidate_keys(candidate_fqns, explicit_arg_count, receiver_ty.is_some());
        if let [single] = candidate_keys.as_slice() {
            return self
                .build_direct_like_call_site(
                    types,
                    kind,
                    candidate_fqns
                        .first()
                        .map(String::as_str)
                        .unwrap_or(fallback_fqn),
                    explicit_arg_count,
                    invoke_args_tuple_ty,
                    result_ty,
                    receiver_ty,
                )
                .map(|facts| match facts.target() {
                    CallSiteTarget::KnownInstance(_) => facts,
                    _ => CallSiteEffectFacts::new_with_abi(
                        kind,
                        CallSiteTarget::KnownInstance(single.clone()),
                        facts.callee_abi_kind(),
                        facts.invoke_args_tuple_ty(),
                        facts.callee_step_schema(),
                        facts.resolved_cases().clone(),
                        facts.precision(),
                    ),
                });
        }

        if !candidate_keys.is_empty() {
            let declared_row =
                self.union_candidate_rows(types, candidate_fqns, explicit_arg_count, receiver_ty)?;
            if declared_row.is_pure() {
                return Ok(CallSiteEffectFacts::new_plain(
                    kind,
                    CallSiteTarget::CandidateSet(candidate_keys),
                    invoke_args_tuple_ty,
                    CaseSet::new(self.callable_step_schema, Vec::new()),
                    EffectPrecision::Precise,
                ));
            }
            let step_schema = self.schema_pool.intern_synthetic_step_schema(
                types,
                invoke_args_tuple_ty,
                result_ty,
                &declared_row,
                &declared_row,
                SyntheticStepSchemaKind::CallSurface,
            )?;
            return Ok(CallSiteEffectFacts::new(
                kind,
                CallSiteTarget::CandidateSet(candidate_keys),
                invoke_args_tuple_ty,
                step_schema,
                self.schema_pool.full_case_set(step_schema),
                EffectPrecision::Widened,
            ));
        }

        self.build_direct_like_call_site(
            types,
            kind,
            fallback_fqn,
            explicit_arg_count,
            invoke_args_tuple_ty,
            result_ty,
            receiver_ty,
        )
    }

    fn union_candidate_rows(
        &self,
        types: &mut TypeStore,
        candidate_fqns: &[String],
        explicit_arg_count: usize,
        receiver_ty: Option<TypeId>,
    ) -> Result<EffectRow, EffectFactsError> {
        let mut terms = Vec::new();
        let mut saw_any = false;
        for fqn in candidate_fqns {
            if let Some(key) =
                self.known_callable_key(fqn, explicit_arg_count, receiver_ty.is_some())
                && let Some(facts) = self.callable_facts.get(&key)
            {
                terms.extend(facts.declared_row().terms.iter().copied());
                saw_any = true;
                continue;
            }
            if let Some(raw_fun) = self.raw_fun_by_fqn.get(fqn) {
                terms.extend(declared_effect_row(raw_fun, types).terms);
                saw_any = true;
                continue;
            }
            if let Some(contract) = self.type_ctx.surface_callable_contract(
                types,
                fqn,
                explicit_arg_count,
                receiver_ty.is_some(),
            ) {
                terms.extend(contract.declared_row.terms);
                saw_any = true;
            }
        }
        if !saw_any {
            return Err(EffectFactsError::MissingCallableSurfaceContract {
                callable: candidate_fqns.join(", "),
            });
        }
        Ok(EffectRow::new(terms))
    }

    fn callable_value_surface_contract(
        &self,
        types: &TypeStore,
        callable_fqn: &str,
    ) -> Option<SurfaceCallableContract> {
        self.body().blocks.iter().find_map(|block| {
            block.stmts.iter().find_map(|stmt| {
                let StatementKind::Assign { target, value } = &stmt.kind else {
                    return None;
                };
                let Rvalue::TopLevelRef(top_level) = value else {
                    return None;
                };
                if top_level.fqn != callable_fqn {
                    return None;
                }
                let local_ty = self.body().locals.get(target.as_u32() as usize)?.ty;
                function_surface_contract_from_ty(types, local_ty)
            })
        })
    }

    fn callable_value_reference_exists(&self, callable_fqn: &str) -> bool {
        self.body().blocks.iter().any(|block| {
            block.stmts.iter().any(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign {
                        value: Rvalue::TopLevelRef(top_level),
                        ..
                    } if top_level.fqn == callable_fqn
                )
            })
        })
    }

    fn dynamic_callable_value_fallback(
        &self,
        kind: CallSiteKind,
        invoke_args_tuple_ty: TypeId,
    ) -> CallSiteEffectFacts {
        CallSiteEffectFacts::new(
            kind,
            CallSiteTarget::DynamicFallback,
            invoke_args_tuple_ty,
            self.callable_step_schema,
            self.schema_pool.full_case_set(self.callable_step_schema),
            EffectPrecision::SignatureFallback,
        )
    }

    fn known_callable_key(
        &self,
        callable_fqn: &str,
        explicit_arg_count: usize,
        has_receiver: bool,
    ) -> Option<InstanceKey> {
        self.owner_by_callable_fqn
            .get(callable_fqn)
            .cloned()
            .or_else(|| {
                self.type_ctx.known_callable_instance_key(
                    callable_fqn,
                    explicit_arg_count,
                    has_receiver,
                )
            })
    }

    fn resolve_candidate_keys(
        &self,
        candidate_fqns: &[String],
        explicit_arg_count: usize,
        has_receiver: bool,
    ) -> Vec<InstanceKey> {
        let mut keys = candidate_fqns
            .iter()
            .filter_map(|fqn| self.known_callable_key(fqn, explicit_arg_count, has_receiver))
            .collect::<Vec<_>>();
        keys.sort_by_key(|key| format!("{key:?}"));
        keys.dedup();
        keys
    }

    fn current_case_for_effect_op(
        &self,
        types: &mut TypeStore,
        effect_ty: TypeId,
        op_fqn: &str,
    ) -> Result<CurrentBodyCaseInfo, EffectFactsError> {
        let contract = self
            .type_ctx
            .concrete_effect_op_contract_for_site(types, effect_ty, op_fqn)?;
        self.current_case_index
            .get(&contract.concrete_op_key)
            .cloned()
            .ok_or_else(|| EffectFactsError::MissingCallableCase {
                callable: self.callable_fun.fqn.clone(),
                op_fqn: op_fqn.to_string(),
            })
    }

    fn body(&self) -> &MirBody {
        self.callable_fun
            .body
            .as_ref()
            .expect("body facts builder requires a callable body")
    }

    fn empty_case_set(&self) -> CaseSet {
        CaseSet::new(self.callable_step_schema, Vec::new())
    }

    fn case_set_from_tags(&self, tags: BTreeSet<CaseTag>) -> CaseSet {
        CaseSet::new(self.callable_step_schema, tags.into_iter().collect())
    }
}

#[derive(Debug, Clone)]
struct CallableSeed {
    key: InstanceKey,
    root_fun: MirFunDecl,
    declared_row: EffectRow,
    // `surface_effect_row` 只表达源码层 residual row；`step_effect_row` 允许额外带上
    // compiler-generated one-shot runtime-error upper bound，供 out-step contract 使用。
    surface_effect_row: EffectRow,
    step_effect_row: EffectRow,
    invoke_arg_components: Vec<TypeId>,
    complete_ty: TypeId,
}

#[derive(Debug, Clone)]
struct StepCaseSeed {
    sort_key: String,
    concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: TypeId,
    resume_tuple_ty: TypeId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ContinuationSchemaKey {
    resume_tuple_ty: TypeId,
    answer_ty: TypeId,
    out_step_schema: StepSchemaId,
    surface_ty: TypeId,
}

#[derive(Debug)]
struct ConcreteEffectOpContract {
    concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: TypeId,
    resume_tuple_ty: TypeId,
}

#[derive(Debug)]
struct EffectFactsTypeContext {
    index: Index,
    env: TypeEnv,
    class_vtables: crate::vtable::ClassVtableIndex,
    interfaces: crate::itable::InterfaceIndex,
    class_itables: crate::itable::ClassItableIndex,
    direct_subclasses: HashMap<String, BTreeSet<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum SyntheticStepSchemaKind {
    CallSurface,
    ResumeSurface,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SyntheticStepSchemaKey {
    invoke_args_tuple_ty: TypeId,
    complete_ty: TypeId,
    effect_row: EffectRow,
    kind: SyntheticStepSchemaKind,
}

#[derive(Debug)]
struct EffectFactsSchemaPool<'a> {
    type_ctx: &'a EffectFactsTypeContext,
    step_schemas: BTreeMap<StepSchemaId, StepSchema>,
    continuation_schemas: BTreeMap<ContinuationSchemaId, ContinuationSchema>,
    continuation_schema_ids: BTreeMap<ContinuationSchemaKey, ContinuationSchemaId>,
    synthetic_step_schema_ids: HashMap<SyntheticStepSchemaKey, StepSchemaId>,
    next_step_schema_id: u32,
    next_continuation_schema_id: u32,
}

#[derive(Debug, Clone)]
struct CurrentBodyCaseInfo {
    tag: CaseTag,
    continuation_schema: ContinuationSchemaId,
}

#[derive(Debug, Clone, Default)]
struct BlockDraft {
    outward_tags: BTreeSet<CaseTag>,
    has_suspend_boundary: bool,
    has_handle_boundary: bool,
}

#[derive(Debug, Clone, Default)]
struct RegionCaseContribution {
    non_cleanup: BTreeSet<CaseTag>,
    cleanup: BTreeSet<CaseTag>,
}

#[derive(Debug)]
struct BodyFactsBuilder<'a, 'b> {
    type_ctx: &'a EffectFactsTypeContext,
    schema_pool: &'b mut EffectFactsSchemaPool<'a>,
    owner_by_callable_fqn: &'a HashMap<String, InstanceKey>,
    callable_facts: &'a HashMap<InstanceKey, CallableEffectFacts>,
    raw_fun_by_fqn: &'a HashMap<String, MirFunDecl>,
    callable_fun: &'a MirFunDecl,
    callable_step_schema: StepSchemaId,
    current_case_index: HashMap<ConcreteOpKey, CurrentBodyCaseInfo>,
    sites: BTreeMap<SiteId, SiteEffectFacts>,
    block_drafts: BTreeMap<BasicBlockId, BlockDraft>,
    block_scan_cache: BTreeMap<BasicBlockId, RegionCaseContribution>,
    block_site_ids: BTreeMap<BasicBlockId, Vec<SiteId>>,
    block_handled_tags: BTreeMap<BasicBlockId, BTreeSet<CaseTag>>,
    handle_site_solver_facts: BTreeMap<SiteId, HandleSiteSolverFacts>,
}

impl<'a> MaterializedEffectFactsBuilder<'a> {
    pub fn from_materialized_snapshot(
        session: &'a Session,
        source: &'a SourceFile,
        materialized: &'a mut MaterializedMir,
    ) -> Self {
        Self {
            session,
            source,
            materialized,
            compiler_continuation_runtime_error_callables: HashSet::new(),
        }
    }

    pub fn with_compiler_continuation_runtime_error_callables(
        mut self,
        callables: impl IntoIterator<Item = InstanceKey>,
    ) -> Self {
        self.compiler_continuation_runtime_error_callables = callables.into_iter().collect();
        self
    }

    pub fn build(self) -> Result<MaterializedEffectFacts, EffectFactsError> {
        let snapshot_binding = {
            let pass_view = self.materialized.pass_view();
            MirSnapshotBinding::from_pass_view(&pass_view)
        };
        let type_ctx = EffectFactsTypeContext::build(self.session, self.source)?;
        let compiler_generated_runtime_error_effect_ty =
            find_or_intern_raise_runtime_error_effect(&mut self.materialized.types);
        let callable_seeds = collect_callable_seeds(
            self.materialized,
            &type_ctx.index,
            &self.compiler_continuation_runtime_error_callables,
            compiler_generated_runtime_error_effect_ty,
        )?;
        let owner_by_callable_fqn = collect_callable_owner_map(self.materialized);
        let raw_fun_by_fqn = collect_raw_fun_by_fqn(&self.materialized.file);

        let mut callable_facts = HashMap::with_capacity(callable_seeds.len());
        let mut bodies = HashMap::with_capacity(callable_seeds.len());
        let mut callable_step_schemas = HashMap::with_capacity(callable_seeds.len());
        let mut schema_pool = EffectFactsSchemaPool::new(&type_ctx);
        let types = &mut self.materialized.types;

        for seed in &callable_seeds {
            let step_schema_id = schema_pool.intern_callable_step_schema(types, seed)?;
            let invoke_args_tuple_ty =
                canonical_tuple_carrier_ty(types, &seed.invoke_arg_components);
            let resolved_outward_cases = schema_pool.full_case_set(step_schema_id);
            let needs_reentry = !resolved_outward_cases.is_empty();
            let impl_plan = match resolved_outward_cases.tags() {
                [] => ImplPlan::NoOutward,
                [single] => ImplPlan::SingleCase(*single),
                _ => ImplPlan::CanonicalFull,
            };

            callable_facts.insert(
                seed.key.clone(),
                CallableEffectFacts::new(
                    seed.declared_row.clone(),
                    CallableAbiKind::EffectStep,
                    Some(invoke_args_tuple_ty),
                    Some(step_schema_id),
                    resolved_outward_cases,
                    needs_reentry,
                    impl_plan,
                ),
            );
            callable_step_schemas.insert(seed.key.clone(), step_schema_id);
        }

        for seed in &callable_seeds {
            let body_facts = if seed.root_fun.body.is_some() {
                BodyFactsBuilder::new(
                    &type_ctx,
                    &mut schema_pool,
                    &owner_by_callable_fqn,
                    &callable_facts,
                    &raw_fun_by_fqn,
                    &seed.root_fun,
                    *callable_step_schemas
                        .get(&seed.key)
                        .expect("every callable seed should have a root step schema"),
                )?
                .build(types)?
            } else {
                BodyEffectFacts::default()
            };
            bodies.insert(seed.key.clone(), body_facts);
        }

        let (step_schemas, continuation_schemas) = schema_pool.finish();

        Ok(MaterializedEffectFacts::new(
            snapshot_binding,
            step_schemas,
            continuation_schemas,
            callable_facts,
            bodies,
        ))
    }
}

impl EffectFactsTypeContext {
    fn build(session: &Session, source: &SourceFile) -> Result<Self, EffectFactsError> {
        let sources = vec![source.clone()];
        let index = session.build_top_level_index(&sources)?;

        let mut parsed = session.parse(source)?;
        let source_refs = vec![source];
        let mut ast_refs = vec![&mut parsed];
        crate::comptime::trim_package_level_comptime_ifs_in_compilation_unit(
            session.sysroot(),
            &source_refs,
            &mut ast_refs,
        )?;

        let mut env = TypeEnv::from_sysroot(session.sysroot(), &index)
            .map_err(|error| EffectFactsError::TypeEnv(Box::new(error)))?;
        env.extend_from_file(source, &parsed, &index)
            .map_err(|error| EffectFactsError::TypeEnv(Box::new(error)))?;

        let mut pairs = session
            .sysroot()
            .files
            .iter()
            .map(|file| (&file.source, &file.ast))
            .collect::<Vec<_>>();
        pairs.push((source, &parsed));

        let class_vtables = crate::vtable::collect_class_vtables(&pairs, &index)?;
        let (interfaces, class_itables) =
            crate::itable::collect_interfaces_and_class_itables(&pairs, &index, &class_vtables)?;
        let direct_subclasses = collect_direct_subclasses(&pairs, &index);

        Ok(Self {
            index,
            env,
            class_vtables,
            interfaces,
            class_itables,
            direct_subclasses,
        })
    }

    fn step_case_seeds(
        &self,
        types: &mut TypeStore,
        declared_row: &EffectRow,
    ) -> Result<Vec<StepCaseSeed>, EffectFactsError> {
        let mut effect_terms = declared_row.terms.clone();
        effect_terms.sort_by(|lhs, rhs| {
            types
                .display(*lhs)
                .to_string()
                .cmp(&types.display(*rhs).to_string())
                .then_with(|| lhs.cmp(rhs))
        });

        let mut cases = Vec::new();
        for effect_ty in effect_terms {
            let effect_display = types.display(effect_ty).to_string();
            let (effect_fqn, effect_type_args) = lower_effect_nominal_identity(types, effect_ty)?;
            let effect_sym = self.env.type_symbol(&effect_fqn).ok_or_else(|| {
                EffectFactsError::MissingEffectTypeSymbol {
                    effect_fqn: effect_fqn.clone(),
                }
            })?;
            let mut ops = effect_op_overloads(&self.index, &effect_fqn);
            ops.sort_by(|(lhs_fqn, _), (rhs_fqn, _)| lhs_fqn.cmp(rhs_fqn));

            for (op_fqn, op) in ops {
                let contract = self.lower_effect_op_contract(
                    types,
                    &effect_fqn,
                    &effect_type_args,
                    &op_fqn,
                    &op,
                    effect_sym,
                )?;
                let sort_key = format!(
                    "{effect_display}::{op_fqn}::{}::{}",
                    types.display(contract.payload_tuple_ty),
                    types.display(contract.resume_tuple_ty)
                );
                cases.push(StepCaseSeed {
                    sort_key,
                    concrete_op_key: contract.concrete_op_key,
                    payload_tuple_ty: contract.payload_tuple_ty,
                    resume_tuple_ty: contract.resume_tuple_ty,
                });
            }
        }

        cases.sort_by(|lhs, rhs| lhs.sort_key.cmp(&rhs.sort_key));
        Ok(cases)
    }

    fn lower_effect_op_contract(
        &self,
        types: &mut TypeStore,
        effect_fqn: &str,
        effect_type_args: &[TypeId],
        op_fqn: &str,
        op: &FunOverload,
        effect_sym: &TypeSymbol,
    ) -> Result<ConcreteEffectOpContract, EffectFactsError> {
        if effect_sym.type_param_names.len() != effect_type_args.len() {
            return Err(EffectFactsError::EffectTypeArgArityMismatch {
                effect_fqn: effect_fqn.to_string(),
                expected: effect_sym.type_param_names.len(),
                found: effect_type_args.len(),
            });
        }

        let mut type_bindings = Vec::new();
        let mut concrete_key_type_args = Vec::new();
        for type_param in &op.sig.type_params {
            let ty = types.ty_param(TypeParamType {
                name: type_param.name.clone(),
                decl_file: op.symbol.decl_file.clone(),
                decl_span: type_param.name_span,
            });
            type_bindings.push((type_param.name.clone(), ty));
            concrete_key_type_args.push(ty);
        }
        for (name, actual_ty) in effect_sym
            .type_param_names
            .iter()
            .zip(effect_type_args.iter().copied())
        {
            type_bindings.push((name.clone(), actual_ty));
            concrete_key_type_args.push(actual_ty);
        }

        let decl_source = self.env.source(&op.symbol.decl_file).ok_or_else(|| {
            EffectFactsError::MissingDeclFileContext {
                path: op.symbol.decl_file.display().to_string(),
            }
        })?;
        let file_ctx = self
            .env
            .file_type_context(&op.symbol.decl_file)
            .ok_or_else(|| EffectFactsError::MissingDeclFileContext {
                path: op.symbol.decl_file.display().to_string(),
            })?;

        let (payload_component_tys, resume_tuple_ty) = {
            let builtins = types.intern_builtins();
            let mut lower = TypeLowering::new_with_ctx(
                decl_source,
                &self.index,
                &self.env,
                types,
                builtins,
                file_ctx.pkg_prefix.clone(),
                file_ctx.imports.clone(),
            );
            let mut payload_component_tys =
                Vec::with_capacity(op.sig.params.len() + usize::from(op.sig.receiver.is_some()));

            if let Some(receiver_ref) = &op.sig.receiver {
                payload_component_tys.push(
                    lower
                        .lower_type_ref_in_decl_file_with_scopes(
                            &op.symbol.decl_file,
                            type_bindings.clone(),
                            std::iter::empty::<(String, EffectRow)>(),
                            receiver_ref,
                        )
                        .map_err(|error| EffectFactsError::TypeLower(Box::new(error)))?,
                );
            }

            for param in &op.sig.params {
                let Some(param_ty_ref) = &param.ty else {
                    return Err(EffectFactsError::MalformedEffectOpSignature {
                        op_fqn: op_fqn.to_string(),
                        detail: "missing parameter type",
                    });
                };
                payload_component_tys.push(
                    lower
                        .lower_type_ref_in_decl_file_with_scopes(
                            &op.symbol.decl_file,
                            type_bindings.clone(),
                            std::iter::empty::<(String, EffectRow)>(),
                            param_ty_ref,
                        )
                        .map_err(|error| EffectFactsError::TypeLower(Box::new(error)))?,
                );
            }

            let resume_tuple_ty = match &op.sig.return_ty {
                Some(return_ty_ref) => lower
                    .lower_type_ref_in_decl_file_with_scopes(
                        &op.symbol.decl_file,
                        type_bindings.clone(),
                        std::iter::empty::<(String, EffectRow)>(),
                        return_ty_ref,
                    )
                    .map_err(|error| EffectFactsError::TypeLower(Box::new(error)))?,
                None => builtins.unit,
            };
            (payload_component_tys, resume_tuple_ty)
        };

        Ok(ConcreteEffectOpContract {
            concrete_op_key: ConcreteOpKey::new(
                InstanceKey {
                    template: TemplateKey {
                        fqn: op_fqn.to_string(),
                        source_path: op.symbol.decl_file.clone(),
                        decl_span: op.symbol.span,
                    },
                    type_args: concrete_key_type_args,
                    eff_args: Vec::new(),
                },
                crate::effect_facts::EffectFamilyKey::new(
                    effect_fqn.to_string(),
                    effect_type_args.to_vec(),
                ),
            ),
            payload_tuple_ty: canonical_tuple_carrier_ty(types, &payload_component_tys),
            resume_tuple_ty,
        })
    }
}

fn collect_callable_seeds(
    materialized: &MaterializedMir,
    index: &Index,
    compiler_continuation_runtime_error_callables: &HashSet<InstanceKey>,
    compiler_generated_runtime_error_effect_ty: TypeId,
) -> Result<Vec<CallableSeed>, EffectFactsError> {
    let pass_view = materialized.pass_view();
    let mut seeds = Vec::with_capacity(pass_view.len());
    for family in pass_view.instances() {
        // effect-op 声明与 compiler-owned `Continuation.resume` surface contract 都会由更专门的
        // metadata/schema 路径承载；它们不应在 P4 被误当成“普通 callable body shell”参与求解。
        if template_decl_is_effect_op(index, &family.key().template)
            || template_decl_is_compiler_owned_resume(&family.key().template)
        {
            continue;
        }
        // canonical pass-view 允许保留“仍有实例身份，但当前没有 root body”的 family（例如
        // declaration-only instance，或某个 pass 已把 root body 从当前 snapshot 中移除）。
        // P4 facts 只能基于仍存在于当前 canonical snapshot 的 root body 建立 callable/body facts；
        // 对这类无 root body 的 family 直接跳过，而不是回 raw MIR 或报错要求补 fallback。
        let Some(root_fun) = family.root_body() else {
            continue;
        };
        let declared_row = declared_effect_row(root_fun, &materialized.types);
        let surface_effect_row = callable_step_effect_row(root_fun, &declared_row, None);
        let step_effect_row =
            if compiler_continuation_runtime_error_callables.contains(family.key()) {
                let mut terms = surface_effect_row.terms.clone();
                terms.push(compiler_generated_runtime_error_effect_ty);
                EffectRow::new(terms)
            } else {
                surface_effect_row.clone()
            };
        seeds.push(CallableSeed {
            key: family.key().clone(),
            root_fun: root_fun.clone(),
            surface_effect_row,
            step_effect_row,
            declared_row,
            invoke_arg_components: root_fun.params.iter().map(|param| param.ty).collect(),
            complete_ty: root_fun.return_ty,
        });
    }
    Ok(seeds)
}

fn template_decl_is_effect_op(index: &Index, template: &TemplateKey) -> bool {
    index.by_fqn.get(&template.fqn).is_some_and(|symbols| {
        symbols.fun.iter().any(|overload| {
            overload.symbol.decl_file == template.source_path
                && overload.symbol.span == template.decl_span
                && overload.sig.kind == ast::FunDeclKind::EffectOp
        })
    })
}

fn template_decl_is_compiler_owned_resume(template: &TemplateKey) -> bool {
    template.fqn == "scoop.core.Continuation.resume"
}

fn declared_effect_row(fun: &MirFunDecl, types: &TypeStore) -> EffectRow {
    match types.kind(fun.ty) {
        TypeKind::Ref(RefTypeKind::Function(function)) => function.effects.clone(),
        _ => EffectRow::pure(),
    }
}

fn is_plain_compiler_intrinsic(callable_fqn: &str) -> bool {
    let base = callable_fqn
        .split("::<")
        .next()
        .unwrap_or(callable_fqn)
        .split("$overload")
        .next()
        .unwrap_or(callable_fqn);
    matches!(
        base,
        "scoop.core.__scoop_array_builder_new"
            | "scoop.core.__scoop_array_builder_push"
            | "scoop.core.__scoop_array_builder_push_string"
            | "scoop.core.__scoop_array_builder_build_array"
            | "scoop.core.__scoop_array_builder_build_mutable_array"
            | "scoop.core.__scoop_array_builder_build_array_string"
            | "scoop.core.size"
            | "scoop.core.get"
            | "scoop.core.set"
            | "scoop.core.toInt"
            | "scoop.thread.yield"
    )
}

/// site-level facts 需要给本地 `perform` / `resume` / `handle` 产生稳定 case tag，即使 callable 的
/// surface `declared_row` 因本地 `handle` 吸收而是 `Pure`。
fn callable_step_effect_row(
    fun: &MirFunDecl,
    declared_row: &EffectRow,
    compiler_generated_runtime_error_effect_ty: Option<TypeId>,
) -> EffectRow {
    let Some(body) = &fun.body else {
        return declared_row.clone();
    };

    let mut terms = declared_row.terms.clone();
    for block in &body.blocks {
        for stmt in &block.stmts {
            let StatementKind::Assign { value, .. } = &stmt.kind else {
                continue;
            };
            let Rvalue::Call {
                kind: CallKind::Resume { resume, .. },
                ..
            } = value
            else {
                continue;
            };
            terms.extend(resume.out_effects.terms.iter().copied());
            if let Some(runtime_error_effect_ty) = resume.runtime_error_effect_ty {
                terms.push(runtime_error_effect_ty);
            }
        }

        match &block.terminator.kind {
            TerminatorKind::Perform { metadata, .. } => terms.push(metadata.effect_ty),
            TerminatorKind::Handle { arms, .. } => {
                terms.extend(arms.iter().map(|arm| arm.handled_effect_ty));
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Goto { .. }
            | TerminatorKind::CondBr { .. }
            | TerminatorKind::Unreachable
            | TerminatorKind::Todo(_) => {}
        }
    }

    if let Some(runtime_error_effect_ty) = compiler_generated_runtime_error_effect_ty {
        terms.push(runtime_error_effect_ty);
    }

    EffectRow::new(terms)
}

fn find_or_intern_raise_runtime_error_effect(types: &mut TypeStore) -> TypeId {
    let runtime_error_ty = types.iter_ids().find(|&id| {
        matches!(
            types.kind(id),
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.RuntimeError"
        ) || matches!(
            types.kind(id),
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.RuntimeError"
        )
    });
    let runtime_error_ty = runtime_error_ty.unwrap_or_else(|| {
        types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "scoop.core.RuntimeError".to_string(),
            args: Vec::new(),
            eff: None,
        })))
    });
    let raise_runtime_error_effect = types.iter_ids().find(|&id| {
        matches!(
            types.kind(id),
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.Raise"
                    && nominal.args.as_slice() == [runtime_error_ty]
                    && nominal.eff.is_none()
        )
    });
    raise_runtime_error_effect.unwrap_or_else(|| {
        types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "scoop.core.Raise".to_string(),
            args: vec![runtime_error_ty],
            eff: None,
        })))
    })
}

fn lower_effect_nominal_identity(
    types: &TypeStore,
    effect_ty: TypeId,
) -> Result<(String, Vec<TypeId>), EffectFactsError> {
    match types.kind(effect_ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
            Ok((nominal.fqn.clone(), nominal.args.clone()))
        }
        _ => Err(EffectFactsError::UnsupportedEffectTerm {
            ty: types.display(effect_ty).to_string(),
        }),
    }
}

fn effect_op_overloads(index: &Index, effect_fqn: &str) -> Vec<(String, FunOverload)> {
    let prefix = format!("{effect_fqn}.");
    index
        .by_fqn
        .iter()
        .flat_map(|(fqn, symbols)| {
            let matches_effect = fqn.starts_with(&prefix);
            symbols
                .fun
                .iter()
                .filter(move |overload| {
                    matches_effect && overload.sig.kind == ast::FunDeclKind::EffectOp
                })
                .map(move |overload| (fqn.clone(), overload.clone()))
        })
        .collect()
}

fn canonical_tuple_carrier_ty(types: &mut TypeStore, components: &[TypeId]) -> TypeId {
    let builtins = types.intern_builtins();
    match components {
        [] => builtins.unit,
        [single] => *single,
        _ => types.ty_tuple(components.to_vec()),
    }
}

fn continuation_surface_ty(
    types: &mut TypeStore,
    resume_tuple_ty: TypeId,
    answer_ty: TypeId,
    out_row: &EffectRow,
) -> TypeId {
    types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
        fqn: "scoop.core.Continuation".to_string(),
        args: vec![resume_tuple_ty, answer_ty],
        eff: Some(out_row.clone()),
    })))
}

fn continuation_object_ty(types: &mut TypeStore, key: &InstanceKey) -> TypeId {
    let type_args_suffix = if key.type_args.is_empty() {
        String::new()
    } else {
        format!(
            "::{}",
            key.type_args
                .iter()
                .map(|ty| types.display(*ty).to_string())
                .collect::<Vec<_>>()
                .join(",")
        )
    };
    let effect_args_suffix = if key.eff_args.is_empty() {
        String::new()
    } else {
        format!(
            "#{}",
            key.eff_args
                .iter()
                .map(|row| effect_row_identity_string(types, row))
                .collect::<Vec<_>>()
                .join("|")
        )
    };
    types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
        fqn: format!(
            "scoop.__compiler.ContinuationObject@{}:{}..{}::{}{}{}",
            key.template.source_path.display(),
            key.template.decl_span.start,
            key.template.decl_span.end,
            key.template.fqn,
            type_args_suffix,
            effect_args_suffix,
        ),
        args: Vec::new(),
        eff: None,
    })))
}

fn effect_row_identity_string(types: &TypeStore, row: &EffectRow) -> String {
    match row.terms.as_slice() {
        [] => "Pure".to_string(),
        [term] => types.display(*term).to_string(),
        terms => format!(
            "({})",
            terms
                .iter()
                .map(|term| types.display(*term).to_string())
                .collect::<Vec<_>>()
                .join(" + ")
        ),
    }
}

fn collect_callable_owner_map(materialized: &MaterializedMir) -> HashMap<String, InstanceKey> {
    let mut owners = HashMap::new();
    let pass_view = materialized.pass_view();
    for family in pass_view.instances() {
        for fqn in family.callable_fqns() {
            owners.insert(fqn.to_string(), family.key().clone());
        }
    }
    owners
}

fn collect_raw_fun_by_fqn(file: &MirFile) -> HashMap<String, MirFunDecl> {
    file.items
        .iter()
        .filter_map(|item| match item {
            MirItem::Fun(fun) => Some((fun.fqn.clone(), fun.clone())),
            MirItem::Todo { .. } => None,
        })
        .collect()
}

fn collect_direct_subclasses(
    pairs: &[(&SourceFile, &ast::File)],
    index: &Index,
) -> HashMap<String, BTreeSet<String>> {
    let mut out = HashMap::new();
    for (source, file) in pairs {
        let pkg_prefix = package_prefix(source, file.package.as_ref());
        for item in &file.items {
            match item {
                ast::Item::Type(ty) => collect_direct_subclasses_in_type_decl(
                    source,
                    file,
                    ty,
                    &pkg_prefix,
                    index,
                    &mut out,
                ),
                ast::Item::Object(obj) => collect_direct_subclasses_in_object_decl(
                    source,
                    file,
                    obj,
                    &pkg_prefix,
                    index,
                    &mut out,
                ),
                ast::Item::Fun(_)
                | ast::Item::Val(_)
                | ast::Item::ExtensionProperty(_)
                | ast::Item::TypeAlias(_)
                | ast::Item::ComptimeIf(_) => {}
            }
        }
    }
    out
}

fn collect_direct_subclasses_in_type_decl(
    source: &SourceFile,
    file: &ast::File,
    decl: &ast::TypeDecl,
    owner_prefix: &str,
    index: &Index,
    out: &mut HashMap<String, BTreeSet<String>>,
) {
    let type_fqn = join_prefix(owner_prefix, decl.name.text(source));
    if matches!(decl.kind, ast::TypeKind::Class)
        && let Some(super_fqn) = decl
            .supertypes
            .iter()
            .filter(|super_ty| super_ty.ctor_args_span.is_some())
            .find_map(|super_ty| index.type_ref_to_fqn_in_file(source, file, &super_ty.ty))
    {
        out.entry(super_fqn).or_default().insert(type_fqn.clone());
    }

    let Some(body) = &decl.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_direct_subclasses_in_type_decl(source, file, nested, &type_fqn, index, out);
            }
            ast::TypeMember::Object(obj) => {
                collect_direct_subclasses_in_object_decl(source, file, obj, &type_fqn, index, out);
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_direct_subclasses_in_object_decl(
    source: &SourceFile,
    file: &ast::File,
    obj: &ast::ObjectDecl,
    owner_prefix: &str,
    index: &Index,
    out: &mut HashMap<String, BTreeSet<String>>,
) {
    let Some(name) = obj.name.as_ref() else {
        return;
    };
    let obj_fqn = join_prefix(owner_prefix, name.text(source));

    if let Some(super_fqn) = obj
        .supertypes
        .iter()
        .filter(|super_ty| super_ty.ctor_args_span.is_some())
        .find_map(|super_ty| index.type_ref_to_fqn_in_file(source, file, &super_ty.ty))
    {
        out.entry(super_fqn).or_default().insert(obj_fqn.clone());
    }

    let Some(body) = &obj.body else {
        return;
    };
    for member in &body.members {
        match member {
            ast::TypeMember::Type(nested) => {
                collect_direct_subclasses_in_type_decl(source, file, nested, &obj_fqn, index, out);
            }
            ast::TypeMember::Object(nested_obj) => {
                collect_direct_subclasses_in_object_decl(
                    source, file, nested_obj, &obj_fqn, index, out,
                );
            }
            ast::TypeMember::EnumVariant(_)
            | ast::TypeMember::Property(_)
            | ast::TypeMember::InitBlock(_)
            | ast::TypeMember::SecondaryCtor(_)
            | ast::TypeMember::Fun(_) => {}
        }
    }
}

fn collect_interface_slot_targets(
    entries: &[crate::itable::ClassItableEntry],
    owner_fqn: &str,
    slot_index: usize,
    out: &mut BTreeSet<String>,
) {
    for entry in entries {
        if entry.interface_fqn != owner_fqn {
            continue;
        }
        if let Some(target) = entry.method_impl_fqns.get(slot_index) {
            out.insert(target.clone());
        }
    }
}

fn nominal_type_fqn(types: &TypeStore, ty: TypeId) -> Option<&str> {
    match types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => Some(nominal.fqn.as_str()),
        _ => None,
    }
}

fn function_surface_contract_from_ty(
    types: &TypeStore,
    ty: TypeId,
) -> Option<SurfaceCallableContract> {
    match types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Function(function)) => Some(SurfaceCallableContract {
            declared_row: function.effects.clone(),
        }),
        TypeKind::Ref(RefTypeKind::Nominal(nominal))
        | TypeKind::Value(ValueTypeKind::Nominal(nominal))
            if nominal.fqn == "scoop.unsafe.FunPtr" && nominal.args.len() == 1 =>
        {
            function_surface_contract_from_ty(types, nominal.args[0])
        }
        _ => None,
    }
}

fn synthetic_continuation_object_ty(
    types: &mut TypeStore,
    kind: SyntheticStepSchemaKind,
    invoke_args_tuple_ty: TypeId,
    complete_ty: TypeId,
    effect_row: &EffectRow,
) -> TypeId {
    let kind_label = match kind {
        SyntheticStepSchemaKind::CallSurface => "CallSurface",
        SyntheticStepSchemaKind::ResumeSurface => "ResumeSurface",
    };
    types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
        fqn: format!(
            "scoop.__compiler.ContinuationObject@{kind_label}::{}::{}::{}",
            types.display(invoke_args_tuple_ty),
            types.display(complete_ty),
            effect_row_identity_string(types, effect_row),
        ),
        args: Vec::new(),
        eff: None,
    })))
}

fn handle_total_outward_tags(facts: &HandleSiteEffectFacts) -> BTreeSet<CaseTag> {
    let mut tags = facts
        .body_outward_cases()
        .tags()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for arm in facts.arm_facts() {
        tags.extend(arm.arm_outward_cases().tags().iter().copied());
    }
    tags.extend(facts.finally_outward_cases().tags().iter().copied());
    tags
}

fn collect_block_successors(body: &MirBody) -> BTreeMap<BasicBlockId, Vec<BasicBlockId>> {
    body.blocks
        .iter()
        .enumerate()
        .map(|(index, block)| {
            let block_id = BasicBlockId::from_raw(index as u32);
            let mut successors = Vec::new();
            block.terminator.for_each_successor(|target| {
                if !successors.contains(&target) {
                    successors.push(target);
                }
            });
            (block_id, successors)
        })
        .collect()
}

fn operand_ty(body: &MirBody, types: &mut TypeStore, operand: &Operand) -> TypeId {
    match operand {
        Operand::Local(local) => body.locals[local.as_u32() as usize].ty,
        Operand::Const(value) => match value {
            ConstValue::Bool(_) => types.intern_builtins().bool_,
            ConstValue::Char => types.intern_builtins().char_,
            ConstValue::Unit => types.intern_builtins().unit,
            ConstValue::Int | ConstValue::SynthInt(_) => types.intern_builtins().int,
            ConstValue::Float64 => types.intern_builtins().float64,
            ConstValue::Float32 => types.intern_builtins().float32,
            ConstValue::String => types.intern_builtins().string,
        },
    }
}

fn package_prefix(source: &SourceFile, package: Option<&ast::PackageDecl>) -> String {
    let Some(package) = package else {
        return String::new();
    };
    let mut out = String::new();
    for (index, segment) in package.path.iter().enumerate() {
        if index != 0 {
            out.push('.');
        }
        out.push_str(segment.text(source));
    }
    out
}

fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_string()
    } else {
        format!("{prefix}.{name}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};
    use std::path::PathBuf;

    use super::{MaterializedEffectFactsBuilder, continuation_object_ty};
    use crate::effect_facts::{
        CallSiteKind, CallSiteTarget, CallTargetMode, CanonicalMirQuerySurface, EffectPrecision,
        ImplPlan, NestedHandleClassification, SiteEffectFacts,
    };
    use crate::mir::{
        BasicBlockId, CallKind, InstanceKey, Rvalue, StatementKind, TemplateKey, TerminatorKind,
        materialize_for_dump,
    };
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::span::Span;
    use crate::ty::{EffectRow, NominalType, RefTypeKind, TypeKind, TypeStore};

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap()
    }

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_builder_fixture.scoop",
            r#"
package sample

effect Flag {
    fun ping(): Unit
}

fun <T> pureUnit(_witness: T): Unit {}

fun <T> raiseString(_witness: T): Unit / scoop.core.Raise<String> {
    scoop.core.Raise.raise("boom")
}

fun <T> raiseInt(_witness: T): Unit / scoop.core.Raise<Int> {
    scoop.core.Raise.raise(1)
}

fun <T> pingFlag(_witness: T): Unit / Flag {
    Flag.ping()
}

fun <T> resumeZero(_witness: T, k: scoop.core.Continuation<Unit, Unit, eff Pure>): Unit / scoop.core.Raise<scoop.core.RuntimeError> {
    k.resume()
}

fun exercise(k: scoop.core.Continuation<Unit, Unit, eff Pure>): Unit / (Flag + scoop.core.Raise<String> + scoop.core.Raise<Int> + scoop.core.Raise<scoop.core.RuntimeError>) {
    pureUnit(())
    raiseString(())
    raiseInt(())
    pingFlag(())
    resumeZero((), k)
}
"#,
        )
    }

    fn build_sample_facts() -> (
        crate::mir::MaterializedMir,
        crate::effect_facts::MaterializedEffectFacts,
    ) {
        let session = refactor_session();
        let source = sample_source();
        let mut materialized = materialize_for_dump(&session, &source).unwrap();
        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &session,
            &source,
            &mut materialized,
        )
        .build()
        .unwrap();
        (materialized, facts)
    }

    fn callable_facts_for<'a>(
        facts: &'a crate::effect_facts::MaterializedEffectFacts,
        fqn: &str,
    ) -> (
        &'a InstanceKey,
        &'a crate::effect_facts::CallableEffectFacts,
    ) {
        let available = facts
            .callable_facts()
            .keys()
            .map(|key| key.template.fqn.clone())
            .collect::<Vec<_>>();
        facts
            .callable_facts()
            .iter()
            .find(|(key, _)| key.template.fqn == fqn || key.template.fqn.ends_with(fqn))
            .unwrap_or_else(|| {
                panic!("fixture callable 应在 facts 中可见: {fqn}; available={available:?}")
            })
    }

    fn build_facts_for_source(
        source: SourceFile,
    ) -> (
        crate::mir::MaterializedMir,
        crate::effect_facts::MaterializedEffectFacts,
    ) {
        let session = refactor_session();
        let mut materialized = materialize_for_dump(&session, &source).unwrap();
        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &session,
            &source,
            &mut materialized,
        )
        .build()
        .unwrap();
        (materialized, facts)
    }

    fn call_and_resume_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_call_sites.scoop",
            r#"
package sample

import scoop.core.*

effect Boom {
    fun next(): Int
}

open class Base() {
    open fun ping(): Int {
        return 1
    }
}

class DerivedA() : Base() {
    override fun ping(): Int {
        return 2
    }
}

class DerivedB() : Base() {
    override fun ping(): Int {
        return 3
    }
}

interface IFace {
    fun foo(): Int
}

class ImplA() : IFace {
    fun foo(): Int {
        return 4
    }
}

class ImplB() : IFace {
    fun foo(): Int {
        return 5
    }
}

fun direct(x: Int): Int {
    return x
}

fun apply(f: (Int) -> Int / (Boom), x: Int): Int / (Boom) {
    return f(x)
}

fun exercise(
    base: Base,
    face: IFace,
    f: (Int) -> Int / (Boom),
    k: Continuation<Int, Int, eff Boom>
): Int / (Boom + Raise<RuntimeError>) {
    val a: Int = direct(1)
    val b: Int = apply(f, 2)
    val c: Int = base.ping()
    val d: Int = face.foo()
    val e: Int = k.resume(3)
    return a + b + c + d + e
}
"#,
        )
    }

    fn handle_site_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_handle_sites.scoop",
            r#"
package sample

import scoop.core.Raise

fun handled_raise(): Int {
    return handle {
        Raise.raise(1)
        0
    } with {
        Raise.raise(e) -> e + 1
    }
}
"#,
        )
    }

    fn nested_handle_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_nested_handle.scoop",
            r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun nested_self_contained(): Int {
    return handle {
        val inner: Int = handle {
            Inner.go()
            0
        } with {
            Inner.go() -> 1
        }
        inner + 10
    } with {
        Outer.again() -> 99
    }
}

fun nested_may_suspend_outward(): Int {
    return handle {
        val inner: Int = handle {
            Inner.go()
            0
        } with {
            Inner.go() -> 1
        } finally {
            Outer.again()
        }
        inner + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
        )
    }

    fn compiler_continuation_runtime_error_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_compiler_cont_runtime_error.scoop",
            r#"
package sample

effect Ping {
    fun hit(): Unit
}

fun leaf(): Unit / Ping {
    Ping.hit()
}

fun pureHelper(): Unit {}
"#,
        )
    }

    fn schema_case_fqns(
        facts: &crate::effect_facts::MaterializedEffectFacts,
        step_schema: crate::effect_facts::StepSchemaId,
    ) -> BTreeSet<String> {
        facts
            .step_schemas()
            .get(&step_schema)
            .expect("step schema 应存在")
            .cases()
            .iter()
            .map(|case| case.concrete_op_key().instance_key().template.fqn.clone())
            .collect()
    }

    fn case_fqns(
        facts: &crate::effect_facts::MaterializedEffectFacts,
        case_set: &crate::effect_facts::CaseSet,
    ) -> BTreeSet<String> {
        if case_set.is_empty() {
            return BTreeSet::new();
        }
        let schema = facts
            .step_schemas()
            .get(&case_set.schema())
            .expect("case set 应引用已存在的 step schema");
        case_set
            .tags()
            .iter()
            .map(|tag| {
                schema
                    .cases()
                    .iter()
                    .find(|case| case.case_tag() == *tag)
                    .expect("case tag 应落在对应 schema 中")
                    .concrete_op_key()
                    .instance_key()
                    .template
                    .fqn
                    .clone()
            })
            .collect()
    }

    fn continuation_surface_ty_string(
        materialized: &crate::mir::MaterializedMir,
        facts: &crate::effect_facts::MaterializedEffectFacts,
        schema_id: crate::effect_facts::ContinuationSchemaId,
    ) -> String {
        let schema = facts
            .continuation_schemas()
            .get(&schema_id)
            .expect("continuation schema 应存在");
        materialized.types.display(schema.surface_ty()).to_string()
    }

    fn continuation_surface_tys_for_step_schema(
        materialized: &crate::mir::MaterializedMir,
        facts: &crate::effect_facts::MaterializedEffectFacts,
        step_schema: crate::effect_facts::StepSchemaId,
    ) -> BTreeSet<String> {
        facts
            .step_schemas()
            .get(&step_schema)
            .expect("step schema 应存在")
            .cases()
            .iter()
            .map(|case| {
                continuation_surface_ty_string(materialized, facts, case.continuation_schema())
            })
            .collect()
    }

    #[test]
    fn refactor_site_effect_facts_capture_call_target_modes_and_resume_contracts() {
        let (materialized, facts) = build_facts_for_source(call_and_resume_source());
        let (apply_key, _) = callable_facts_for(&facts, "sample.apply");
        let (exercise_key, _) = callable_facts_for(&facts, "sample.exercise");
        let pass_view = materialized.pass_view();
        let apply_body = pass_view
            .instance(apply_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("apply 应有 canonical body");
        let exercise_body = pass_view
            .instance(exercise_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("exercise 应有 canonical body");
        let apply_body_facts = facts.body(apply_key).expect("apply 应有 body facts");
        let exercise_body_facts = facts.body(exercise_key).expect("exercise 应有 body facts");

        let mut direct_site_ids = Vec::new();
        let mut virtual_site_id = None;
        let mut interface_site_id = None;
        let mut resume_site_id = None;
        let mut resume_block_id = None;
        for (block_index, block) in exercise_body.blocks.iter().enumerate() {
            for stmt in &block.stmts {
                let StatementKind::Assign { value, .. } = &stmt.kind else {
                    continue;
                };
                let Rvalue::Call { site_id, kind, .. } = value else {
                    continue;
                };
                match kind {
                    CallKind::Direct { .. } => {
                        direct_site_ids.push(*site_id);
                    }
                    CallKind::Virtual { .. } => {
                        virtual_site_id = Some(*site_id);
                    }
                    CallKind::Interface { .. } => {
                        interface_site_id = Some(*site_id);
                    }
                    CallKind::Resume { .. } => {
                        resume_site_id = Some(*site_id);
                        resume_block_id = Some(BasicBlockId::from_raw(block_index as u32));
                    }
                    CallKind::Closure { .. } | CallKind::FunValue { .. } => {}
                }
            }
        }
        let fun_value_site_id = apply_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| {
                let StatementKind::Assign { value, .. } = &stmt.kind else {
                    return None;
                };
                let Rvalue::Call {
                    site_id,
                    kind: CallKind::FunValue { .. },
                    ..
                } = value
                else {
                    return None;
                };
                Some(*site_id)
            })
            .expect("apply 应包含 callable-value site");

        let effectful_direct_facts = direct_site_ids
            .into_iter()
            .filter_map(|site_id| match exercise_body_facts.site(site_id) {
                Some(SiteEffectFacts::Call(call_facts))
                    if call_facts.kind() == CallSiteKind::Direct =>
                {
                    Some(call_facts)
                }
                _ => None,
            })
            .find(|call_facts| {
                case_fqns(&facts, call_facts.resolved_cases()).contains("sample.Boom.next")
            })
            .expect("exercise 应包含带 outward case 的 known direct call");
        assert_eq!(effectful_direct_facts.kind(), CallSiteKind::Direct);
        assert_eq!(
            effectful_direct_facts.target_mode(),
            CallTargetMode::KnownInstance
        );
        assert_eq!(effectful_direct_facts.precision(), EffectPrecision::Widened);
        assert!(matches!(
            effectful_direct_facts.target(),
            CallSiteTarget::KnownInstance(key)
                if key.template.fqn.starts_with("sample.") && key.template.fqn != "sample.exercise"
        ));
        assert_eq!(
            case_fqns(&facts, effectful_direct_facts.resolved_cases()),
            ["sample.Boom.next".to_string()].into_iter().collect()
        );

        let SiteEffectFacts::Call(fun_value_facts) = apply_body_facts
            .site(fun_value_site_id)
            .expect("fun-value site 应可通过 SiteId 查询")
        else {
            panic!("fun-value site 应产生 CallSiteEffectFacts");
        };
        assert_eq!(fun_value_facts.kind(), CallSiteKind::FunValue);
        assert_eq!(
            fun_value_facts.target_mode(),
            CallTargetMode::DynamicFallback
        );
        assert!(matches!(
            fun_value_facts.target(),
            CallSiteTarget::DynamicFallback
        ));
        assert_eq!(
            fun_value_facts.precision(),
            EffectPrecision::SignatureFallback
        );
        assert_eq!(
            facts
                .step_schemas()
                .get(&fun_value_facts.callee_schema())
                .expect("fun-value fallback schema 应存在")
                .cases()
                .iter()
                .map(|case| case.concrete_op_key().instance_key().template.fqn.clone())
                .collect::<BTreeSet<_>>(),
            ["sample.Boom.next".to_string()].into_iter().collect()
        );

        let SiteEffectFacts::Call(virtual_facts) = exercise_body_facts
            .site(virtual_site_id.expect("exercise 应包含 virtual dispatch site"))
            .expect("virtual site 应可通过 SiteId 查询")
        else {
            panic!("virtual site 应产生 CallSiteEffectFacts");
        };
        assert_eq!(virtual_facts.kind(), CallSiteKind::Virtual);
        assert_eq!(virtual_facts.target_mode(), CallTargetMode::CandidateSet);
        assert_eq!(virtual_facts.precision(), EffectPrecision::Precise);
        let CallSiteTarget::CandidateSet(virtual_targets) = virtual_facts.target() else {
            panic!("virtual dispatch 应保留 candidate set");
        };
        assert_eq!(
            virtual_targets
                .iter()
                .map(|key| key.template.fqn.clone())
                .collect::<BTreeSet<_>>(),
            [
                "sample.Base.ping".to_string(),
                "sample.DerivedA.ping".to_string(),
                "sample.DerivedB.ping".to_string(),
            ]
            .into_iter()
            .collect()
        );

        let SiteEffectFacts::Call(interface_facts) = exercise_body_facts
            .site(interface_site_id.expect("exercise 应包含 interface dispatch site"))
            .expect("interface site 应可通过 SiteId 查询")
        else {
            panic!("interface site 应产生 CallSiteEffectFacts");
        };
        assert_eq!(interface_facts.kind(), CallSiteKind::Interface);
        assert_eq!(interface_facts.target_mode(), CallTargetMode::CandidateSet);
        assert_eq!(interface_facts.precision(), EffectPrecision::Precise);
        let CallSiteTarget::CandidateSet(interface_targets) = interface_facts.target() else {
            panic!("interface dispatch 应保留 candidate set");
        };
        assert_eq!(
            interface_targets
                .iter()
                .map(|key| key.template.fqn.clone())
                .collect::<BTreeSet<_>>(),
            [
                "sample.ImplA.foo".to_string(),
                "sample.ImplB.foo".to_string()
            ]
            .into_iter()
            .collect()
        );

        let SiteEffectFacts::Resume(resume_facts) = exercise_body_facts
            .site(resume_site_id.expect("exercise 应包含 resume site"))
            .expect("resume site 应可通过 SiteId 查询")
        else {
            panic!("resume site 应产生 ResumeSiteEffectFacts");
        };
        assert_eq!(
            materialized
                .types
                .display(resume_facts.resume_tuple_ty())
                .to_string(),
            "Int"
        );
        assert_eq!(
            materialized
                .types
                .display(resume_facts.answer_ty())
                .to_string(),
            "Int"
        );
        assert_eq!(
            facts
                .step_schemas()
                .get(&resume_facts.out_step_schema())
                .expect("resume outward step schema 应存在")
                .cases()
                .iter()
                .map(|case| case.concrete_op_key().instance_key().template.fqn.clone())
                .collect::<BTreeSet<_>>(),
            [
                "sample.Boom.next".to_string(),
                "scoop.core.Raise.raise".to_string(),
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            continuation_surface_ty_string(
                &materialized,
                &facts,
                resume_facts.continuation_schema(),
            ),
            "scoop.core.Continuation<Int, Int, eff sample.Boom>"
        );
        assert_eq!(
            continuation_surface_tys_for_step_schema(
                &materialized,
                &facts,
                resume_facts.out_step_schema(),
            ),
            [
                "scoop.core.Continuation<Int, Int, eff sample.Boom>".to_string(),
                "scoop.core.Continuation<Nothing, Int, eff sample.Boom>".to_string(),
            ]
            .into_iter()
            .collect(),
            "resume synthetic step upper bound 可以保留 runtime-error case，但 continuation surface_ty 仍应保持源码 residual row"
        );
        assert_eq!(resume_facts.resolved_cases().tags().len(), 2);
        assert!(
            exercise_body_facts
                .block(resume_block_id.expect("resume site 应落在某个 basic block 中"))
                .expect("resume block facts 应存在")
                .has_suspend_boundary()
        );
    }

    #[test]
    fn refactor_site_effect_facts_capture_perform_and_handle_contracts() {
        let (materialized, facts) = build_facts_for_source(handle_site_source());
        let (key, _) = callable_facts_for(&facts, "sample.handled_raise");
        let pass_view = materialized.pass_view();
        let body = pass_view
            .instance(key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("handled_raise 应有 canonical body");
        let body_facts = facts.body(key).expect("handled_raise 应有 body facts");

        let handle_site_id = body
            .blocks
            .iter()
            .find_map(|block| match &block.terminator.kind {
                TerminatorKind::Handle { site_id, .. } => Some(*site_id),
                _ => None,
            })
            .expect("handled_raise 应包含 handle site");
        let perform_site_id = body
            .blocks
            .iter()
            .find_map(|block| match &block.terminator.kind {
                TerminatorKind::Perform { site_id, .. } => Some(*site_id),
                _ => None,
            })
            .expect("handled_raise 应包含 perform site");

        let SiteEffectFacts::Handle(handle_facts) = body_facts
            .site(handle_site_id)
            .expect("handle site 应可通过 SiteId 查询")
        else {
            panic!("handle site 应产生 HandleSiteEffectFacts");
        };
        assert_eq!(
            handle_facts.nested_handle_classification(),
            NestedHandleClassification::SelfContained
        );
        assert_eq!(
            case_fqns(&facts, handle_facts.handled_cases()),
            ["scoop.core.Raise.raise".to_string()].into_iter().collect()
        );
        assert!(handle_facts.body_outward_cases().is_empty());
        assert!(handle_facts.finally_outward_cases().is_empty());
        assert_eq!(handle_facts.arm_facts().len(), 1);
        let arm_facts = &handle_facts.arm_facts()[0];
        assert!(arm_facts.arm_outward_cases().is_empty());

        let SiteEffectFacts::Perform(perform_facts) = body_facts
            .site(perform_site_id)
            .expect("perform site 应可通过 SiteId 查询")
        else {
            panic!("perform site 应产生 PerformSiteEffectFacts");
        };
        assert_eq!(perform_facts.emitted_case(), arm_facts.handled_case());
        assert_eq!(
            materialized
                .types
                .display(perform_facts.payload_tuple_ty())
                .to_string(),
            "Int"
        );
        assert_eq!(
            perform_facts.captured_cont_schema(),
            arm_facts.continuation_schema()
        );
    }

    #[test]
    fn refactor_body_effect_facts_index_blocks_and_sites_by_stable_ids() {
        let (materialized, facts) = build_facts_for_source(handle_site_source());
        let (key, _) = callable_facts_for(&facts, "sample.handled_raise");
        let pass_view = materialized.pass_view();
        let body = pass_view
            .instance(key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("handled_raise 应有 canonical body");
        let body_facts = facts.body(key).expect("handled_raise 应有 body facts");

        assert_eq!(body_facts.blocks().len(), body.blocks.len());

        let mut handle_block_id = None;
        let mut perform_block_id = None;
        let mut handle_body_target = None;
        for (block_index, block) in body.blocks.iter().enumerate() {
            let block_id = BasicBlockId::from_raw(block_index as u32);
            match &block.terminator.kind {
                TerminatorKind::Handle {
                    site_id,
                    body_target,
                    ..
                } => {
                    handle_block_id = Some(block_id);
                    handle_body_target = Some(*body_target);
                    assert!(body_facts.site(*site_id).is_some());
                }
                TerminatorKind::Perform { site_id, .. } => {
                    perform_block_id = Some(block_id);
                    assert!(body_facts.site(*site_id).is_some());
                }
                TerminatorKind::Return { .. }
                | TerminatorKind::ResumeUnwind
                | TerminatorKind::Goto { .. }
                | TerminatorKind::CondBr { .. }
                | TerminatorKind::Unreachable
                | TerminatorKind::Todo(_) => {}
            }
        }

        let handle_block_id = handle_block_id.expect("应存在 handle block");
        let perform_block_id = perform_block_id.expect("应存在 perform block");
        assert_eq!(Some(perform_block_id), handle_body_target);

        let handle_block_facts = body_facts
            .block(handle_block_id)
            .expect("handle block 应有 BlockEffectFacts");
        assert!(handle_block_facts.has_handle_boundary());
        assert!(handle_block_facts.outward_cases().is_empty());

        let perform_block_facts = body_facts
            .block(perform_block_id)
            .expect("perform block 应有 BlockEffectFacts");
        assert!(perform_block_facts.has_suspend_boundary());
        assert_eq!(perform_block_facts.outward_cases().tags().len(), 1);
    }

    #[test]
    fn refactor_nested_handle_classification_distinguishes_self_contained_and_finally_outward() {
        let (materialized, facts) = build_facts_for_source(nested_handle_source());
        let pass_view = materialized.pass_view();

        let (self_key, _) = callable_facts_for(&facts, "sample.nested_self_contained");
        let self_body = pass_view
            .instance(self_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("nested_self_contained 应有 canonical body");
        let self_body_facts = facts
            .body(self_key)
            .expect("nested_self_contained 应有 body facts");
        let self_inner_handle = self_body
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator.kind {
                TerminatorKind::Handle { site_id, .. } => self_body_facts.site(*site_id),
                _ => None,
            })
            .find_map(|site| match site {
                SiteEffectFacts::Handle(handle_facts)
                    if case_fqns(&facts, handle_facts.handled_cases())
                        == ["sample.Inner.go".to_string()].into_iter().collect() =>
                {
                    Some(handle_facts)
                }
                SiteEffectFacts::Call(_)
                | SiteEffectFacts::Perform(_)
                | SiteEffectFacts::Resume(_)
                | SiteEffectFacts::Handle(_) => None,
            })
            .expect("nested_self_contained 应包含 inner handle site");
        assert_eq!(
            self_inner_handle.nested_handle_classification(),
            NestedHandleClassification::SelfContained
        );
        assert!(self_inner_handle.finally_outward_cases().is_empty());

        let (may_key, _) = callable_facts_for(&facts, "sample.nested_may_suspend_outward");
        let may_body = pass_view
            .instance(may_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("nested_may_suspend_outward 应有 canonical body");
        let may_body_facts = facts
            .body(may_key)
            .expect("nested_may_suspend_outward 应有 body facts");
        let may_inner_handle = may_body
            .blocks
            .iter()
            .filter_map(|block| match &block.terminator.kind {
                TerminatorKind::Handle { site_id, .. } => may_body_facts.site(*site_id),
                _ => None,
            })
            .find_map(|site| match site {
                SiteEffectFacts::Handle(handle_facts)
                    if case_fqns(&facts, handle_facts.handled_cases())
                        == ["sample.Inner.go".to_string()].into_iter().collect() =>
                {
                    Some(handle_facts)
                }
                SiteEffectFacts::Call(_)
                | SiteEffectFacts::Perform(_)
                | SiteEffectFacts::Resume(_)
                | SiteEffectFacts::Handle(_) => None,
            })
            .expect("nested_may_suspend_outward 应包含 inner handle site");
        assert_eq!(
            may_inner_handle.nested_handle_classification(),
            NestedHandleClassification::MaySuspendOutward
        );
        assert_eq!(
            case_fqns(&facts, may_inner_handle.finally_outward_cases()),
            ["sample.Outer.again".to_string()].into_iter().collect()
        );
    }

    #[test]
    fn materialized_effect_facts_builder_uses_canonical_pass_view_snapshot() {
        let session = refactor_session();
        let source = sample_source();
        let mut materialized = materialize_for_dump(&session, &source).unwrap();
        let removed_fqn = materialized
            .pass_view()
            .instances()
            .next()
            .expect("fixture 应该产生至少一个 instance")
            .root_fqn()
            .to_string();

        materialized
            .pass_artifacts_mut()
            .remove_callable_body(&removed_fqn);

        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &session,
            &source,
            &mut materialized,
        )
        .build()
        .unwrap();

        assert_eq!(
            facts.snapshot_binding().query_surface(),
            CanonicalMirQuerySurface::PassView
        );
        assert_eq!(
            facts.snapshot_binding().instance_count(),
            materialized.pass_view().len()
        );
        assert_eq!(facts.callable_facts().len(), facts.bodies().len());
        assert!(
            !facts
                .snapshot_binding()
                .canonical_body_fqns()
                .iter()
                .any(|fqn| fqn == &removed_fqn)
        );
    }

    #[test]
    fn refactor_callable_effect_facts_shell_skips_effect_op_roots() {
        let (materialized, facts) = build_sample_facts();
        let pass_view = materialized.pass_view();
        let pass_roots = pass_view
            .instances()
            .map(|family| family.root_fqn().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            facts.callable_facts().len(),
            pass_view.len(),
            "pass-view roots: {pass_roots:?}"
        );
        assert!(
            pass_view
                .instances()
                .all(|family| facts.callable_facts().contains_key(family.key()))
        );
        assert!(
            facts
                .callable_facts()
                .keys()
                .all(|key| key.template.fqn != "sample.Flag.ping")
        );
        assert!(
            facts
                .callable_facts()
                .keys()
                .all(|key| key.template.fqn != "scoop.core.Raise.raise")
        );
    }

    #[test]
    fn refactor_effect_schema_case_tags_are_stable_and_distinguish_generic_specialized_raise_cases()
    {
        let (materialized, facts) = build_sample_facts();

        let (_, raise_string_facts) = callable_facts_for(&facts, "sample.raiseString");
        let (_, raise_int_facts) = callable_facts_for(&facts, "sample.raiseInt");
        let raise_string_schema = facts
            .step_schemas()
            .get(&raise_string_facts.step_schema())
            .expect("raiseString 应有 step schema");
        let raise_int_schema = facts
            .step_schemas()
            .get(&raise_int_facts.step_schema())
            .expect("raiseInt 应有 step schema");

        let raise_string_case = &raise_string_schema.cases()[0];
        let raise_int_case = &raise_int_schema.cases()[0];
        assert_eq!(raise_string_case.case_tag().as_u32(), 0);
        assert_eq!(raise_int_case.case_tag().as_u32(), 0);
        assert_eq!(
            raise_string_case
                .concrete_op_key()
                .instance_key()
                .template
                .fqn,
            "scoop.core.Raise.raise"
        );
        assert_ne!(
            raise_string_case.concrete_op_key(),
            raise_int_case.concrete_op_key(),
            "Raise<String>.raise 与 Raise<Int>.raise 应是不同 concrete op"
        );
        assert_eq!(
            materialized
                .types
                .display(raise_string_case.concrete_op_key().instance_key().type_args[0])
                .to_string(),
            "String"
        );
        assert_eq!(
            materialized
                .types
                .display(raise_int_case.concrete_op_key().instance_key().type_args[0])
                .to_string(),
            "Int"
        );
    }

    #[test]
    fn refactor_continuation_schema_explicitly_records_unit_payload_resume_and_surface_type() {
        let (materialized, facts) = build_sample_facts();

        let (_, ping_flag_facts) = callable_facts_for(&facts, "sample.pingFlag");
        let schema = facts
            .step_schemas()
            .get(&ping_flag_facts.step_schema())
            .expect("pingFlag 应有 step schema");
        let case = &schema.cases()[0];
        let continuation_schema = facts
            .continuation_schemas()
            .get(&case.continuation_schema())
            .expect("pingFlag case 应有 continuation schema");

        assert_eq!(
            materialized
                .types
                .display(case.payload_tuple_ty())
                .to_string(),
            "Unit"
        );
        assert_eq!(
            materialized
                .types
                .display(continuation_schema.resume_tuple_ty())
                .to_string(),
            "Unit"
        );
        assert_eq!(
            materialized
                .types
                .display(continuation_schema.answer_ty())
                .to_string(),
            "Unit"
        );
        assert_eq!(
            materialized
                .types
                .display(continuation_schema.surface_ty())
                .to_string(),
            "scoop.core.Continuation<Unit, Unit, eff sample.Flag>"
        );
        assert!(
            materialized
                .types
                .display(schema.continuation_obj_ty())
                .to_string()
                .contains("sample.pingFlag")
        );
    }

    #[test]
    fn refactor_callable_effect_facts_shell_uses_final_shape_and_runtime_error_case() {
        let (materialized, facts) = build_sample_facts();

        let (_, pure_facts) = callable_facts_for(&facts, "sample.pureUnit");
        assert!(matches!(pure_facts.impl_plan(), ImplPlan::NoOutward));
        assert!(!pure_facts.needs_reentry());
        assert!(pure_facts.resolved_outward_cases().is_empty());

        let (_, resume_zero_facts) = callable_facts_for(&facts, "sample.resumeZero");
        assert!(resume_zero_facts.needs_reentry());
        assert!(matches!(
            resume_zero_facts.impl_plan(),
            ImplPlan::SingleCase(tag) if tag.as_u32() == 0
        ));

        let schema = facts
            .step_schemas()
            .get(&resume_zero_facts.step_schema())
            .expect("resumeZero 应有 step schema");
        let runtime_case = &schema.cases()[0];
        assert_eq!(
            runtime_case.concrete_op_key().instance_key().template.fqn,
            "scoop.core.Raise.raise"
        );
        assert_eq!(
            materialized
                .types
                .display(runtime_case.payload_tuple_ty())
                .to_string(),
            "scoop.core.RuntimeError"
        );
        assert_eq!(
            continuation_surface_tys_for_step_schema(
                &materialized,
                &facts,
                resume_zero_facts.step_schema(),
            ),
            ["scoop.core.Continuation<Nothing, Unit, eff scoop.core.Raise<scoop.core.RuntimeError>>".to_string()]
                .into_iter()
                .collect(),
            "若源码 residual row 本来就包含 Raise<RuntimeError>，surface_ty 必须继续如实保留它"
        );
    }

    #[test]
    fn refactor_effect_schema_compiler_continuation_runtime_error_adds_runtime_error_case_to_step_schema()
     {
        let session = refactor_session();
        let source = compiler_continuation_runtime_error_source();
        let mut materialized = materialize_for_dump(&session, &source).unwrap();
        let leaf_key = materialized
            .pass_view()
            .owner_of_callable("sample.leaf")
            .expect("leaf 应有 canonical owner")
            .clone();

        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &session,
            &source,
            &mut materialized,
        )
        .with_compiler_continuation_runtime_error_callables([leaf_key.clone()])
        .build()
        .unwrap();

        let leaf_facts = facts
            .callable_facts()
            .get(&leaf_key)
            .expect("leaf 应存在于 callable facts");
        assert_eq!(
            schema_case_fqns(&facts, leaf_facts.step_schema()),
            [
                "sample.Ping.hit".to_string(),
                "scoop.core.Raise.raise".to_string(),
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn refactor_continuation_schema_surface_ty_preserves_residual_out_row_for_compiler_runtime_error_upper_bound()
     {
        let session = refactor_session();
        let source = compiler_continuation_runtime_error_source();
        let mut materialized = materialize_for_dump(&session, &source).unwrap();
        let leaf_key = materialized
            .pass_view()
            .owner_of_callable("sample.leaf")
            .expect("leaf 应有 canonical owner")
            .clone();

        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &session,
            &source,
            &mut materialized,
        )
        .with_compiler_continuation_runtime_error_callables([leaf_key.clone()])
        .build()
        .unwrap();

        let leaf_facts = facts
            .callable_facts()
            .get(&leaf_key)
            .expect("leaf 应存在于 callable facts");
        assert_eq!(
            continuation_surface_tys_for_step_schema(
                &materialized,
                &facts,
                leaf_facts.step_schema()
            ),
            [
                "scoop.core.Continuation<Nothing, Unit, eff sample.Ping>".to_string(),
                "scoop.core.Continuation<Unit, Unit, eff sample.Ping>".to_string(),
            ]
            .into_iter()
            .collect(),
            "compiler-generated one-shot runtime-error upper bound 只能进入 step/out-step schema，不能反写进 continuation surface_ty"
        );
    }

    #[test]
    fn refactor_callable_effect_facts_shell_compiler_continuation_runtime_error_only_expands_selected_callables()
     {
        let session = refactor_session();
        let source = compiler_continuation_runtime_error_source();
        let mut materialized = materialize_for_dump(&session, &source).unwrap();
        let pass_view = materialized.pass_view();
        let leaf_key = pass_view
            .owner_of_callable("sample.leaf")
            .expect("leaf 应有 canonical owner")
            .clone();
        let pure_key = pass_view
            .owner_of_callable("sample.pureHelper")
            .expect("pureHelper 应有 canonical owner")
            .clone();

        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &session,
            &source,
            &mut materialized,
        )
        .with_compiler_continuation_runtime_error_callables([leaf_key.clone()])
        .build()
        .unwrap();

        let leaf_facts = facts
            .callable_facts()
            .get(&leaf_key)
            .expect("leaf 应存在于 callable facts");
        let pure_facts = facts
            .callable_facts()
            .get(&pure_key)
            .expect("pureHelper 应存在于 callable facts");

        assert!(
            schema_case_fqns(&facts, leaf_facts.step_schema()).contains("scoop.core.Raise.raise"),
            "被标记为 compiler continuation runtime-error callable 的 step schema 应包含 Raise<RuntimeError> case"
        );
        assert!(
            schema_case_fqns(&facts, pure_facts.step_schema()).is_empty(),
            "未被标记且 truly no-outward 的 callable 不应无端长出 runtime-error case"
        );
    }

    #[test]
    fn refactor_continuation_schema_surface_ty_preserves_pure_resume_surface_row() {
        let (materialized, facts) = build_sample_facts();
        let (resume_zero_key, _) = callable_facts_for(&facts, "sample.resumeZero");
        let pass_view = materialized.pass_view();
        let resume_zero_body = pass_view
            .instance(resume_zero_key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("resumeZero 应有 canonical body");
        let resume_zero_body_facts = facts
            .body(resume_zero_key)
            .expect("resumeZero 应有 body facts");
        let resume_site_id = resume_zero_body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| {
                let StatementKind::Assign { value, .. } = &stmt.kind else {
                    return None;
                };
                let Rvalue::Call {
                    site_id,
                    kind: CallKind::Resume { .. },
                    ..
                } = value
                else {
                    return None;
                };
                Some(*site_id)
            })
            .expect("resumeZero 应包含 resume site");

        let SiteEffectFacts::Resume(resume_facts) = resume_zero_body_facts
            .site(resume_site_id)
            .expect("resume site 应可通过 SiteId 查询")
        else {
            panic!("resume site 应产生 ResumeSiteEffectFacts");
        };

        assert_eq!(
            continuation_surface_ty_string(
                &materialized,
                &facts,
                resume_facts.continuation_schema()
            ),
            "scoop.core.Continuation<Unit, Unit, eff Pure>"
        );
        assert_eq!(
            continuation_surface_tys_for_step_schema(
                &materialized,
                &facts,
                resume_facts.out_step_schema()
            ),
            ["scoop.core.Continuation<Nothing, Unit, eff Pure>".to_string()]
                .into_iter()
                .collect(),
            "resume synthetic step upper bound 即使额外带 runtime-error case，也不能把 Pure residual row 扩大回 surface_ty"
        );
    }

    #[test]
    fn refactor_callable_effect_facts_shell_instance_keys_distinguish_allowed_rows() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let raise_string = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "scoop.core.Raise".to_string(),
            args: vec![builtins.string],
            eff: None,
        })));
        let raise_int = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "scoop.core.Raise".to_string(),
            args: vec![builtins.int],
            eff: None,
        })));
        let template = TemplateKey {
            fqn: "sample.forward".to_string(),
            source_path: PathBuf::from("<mem>/forward.scoop"),
            decl_span: Span::new(0, 1),
        };
        let string_key = InstanceKey {
            template: template.clone(),
            type_args: Vec::new(),
            eff_args: vec![EffectRow::new(vec![raise_string])],
        };
        let int_key = InstanceKey {
            template,
            type_args: Vec::new(),
            eff_args: vec![EffectRow::new(vec![raise_int])],
        };

        let mut seen = HashMap::new();
        seen.insert(string_key, "string");
        seen.insert(int_key, "int");

        assert_eq!(seen.len(), 2);
    }

    #[test]
    fn refactor_continuation_schema_identity_distinguishes_callable_instances() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let raise_string = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "scoop.core.Raise".to_string(),
            args: vec![builtins.string],
            eff: None,
        })));
        let raise_int = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "scoop.core.Raise".to_string(),
            args: vec![builtins.int],
            eff: None,
        })));
        let template = TemplateKey {
            fqn: "sample.forward".to_string(),
            source_path: PathBuf::from("<mem>/forward.scoop"),
            decl_span: Span::new(0, 1),
        };
        let string_key = InstanceKey {
            template: template.clone(),
            type_args: vec![builtins.string],
            eff_args: vec![EffectRow::new(vec![raise_string])],
        };
        let int_key = InstanceKey {
            template,
            type_args: vec![builtins.int],
            eff_args: vec![EffectRow::new(vec![raise_int])],
        };

        let string_cont_ty = continuation_object_ty(&mut types, &string_key);
        let int_cont_ty = continuation_object_ty(&mut types, &int_key);

        assert_ne!(string_cont_ty, int_cont_ty);
        assert_ne!(
            types.display(string_cont_ty).to_string(),
            types.display(int_cont_ty).to_string()
        );
    }
}
