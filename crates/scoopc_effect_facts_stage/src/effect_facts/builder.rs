use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::ast;
use crate::mir::{
    BasicBlockId, FunDecl as MirFunDecl, InstanceKey, MaterializedMir, SiteId, TemplateKey,
};
use crate::resolve::{FunOverload, Index};
use crate::stable_id::{
    NoTypeParamResolver, StableCanonicalKey, StableConeKey, StableDefKey, StableDefNamespace,
    StableInstanceKey, StableTemplateKey, StableTypeParamKey, canonical_callable_signature_key,
    canonical_type_text,
};
use crate::ty::{
    EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore, ValueTypeKind,
};
use crate::typecheck::{TypeEnv, TypeLowering, TypeSymbol};
use scoopc_hir::stage::HirSemanticArtifact;
use scoopc_mir_facts::{
    MirFacts, backend as mir_backend, boundary as mir_boundary, effects as mir_effects,
};

use super::{
    BlockEffectFacts, BodyEffectFacts, BodyEffectSolverFacts, CallSiteEffectFacts, CallSiteKind,
    CallSiteTarget, CallableAbiKind, CallableEffectFacts, CaseSet, CaseTag,
    ClassCtorSiteEffectFacts, ConcreteOpKey, ContinuationSchema, ContinuationSchemaId,
    EffectFactsError, EffectOwnedTypeContext, EffectPrecision, HandleArmEffectFacts,
    HandleSiteEffectFacts, HandleSiteSolverFacts, ImplPlan, MaterializedEffectFacts,
    MirSnapshotBinding, NestedHandleClassification, PerformSiteEffectFacts, ResumeSiteEffectFacts,
    SiteEffectFacts, StepCaseFact, StepSchema, StepSchemaId,
};

/// 从 canonical materialized MIR snapshot 生成 P4 facts 容器。
#[derive(Debug)]
pub struct MaterializedEffectFactsBuilder<'a> {
    frontend_artifact: &'a HirSemanticArtifact,
    materialized: &'a MaterializedMir,
    mir_facts: &'a MirFacts,
    type_context: &'a mut EffectOwnedTypeContext,
    compiler_continuation_runtime_error_callables: HashSet<InstanceKey>,
}

#[derive(Debug)]
struct MirFactIndex<'a> {
    instance_by_stable_text: HashMap<String, InstanceKey>,
    instance_by_callable_fqn: HashMap<String, InstanceKey>,
    callable_instances: HashMap<InstanceKey, &'a mir_effects::CallableInstanceEffectFacts>,
    bodies: HashMap<(InstanceKey, String), MirBodyFactBundle<'a>>,
    source_signatures: HashMap<String, &'a mir_backend::SourceCallableSignatureFact>,
}

#[derive(Debug, Clone, Default)]
struct MirBodyFactBundle<'a> {
    regions: BTreeMap<BasicBlockId, &'a mir_effects::MirBlockEffectRegionFact>,
    sites: BTreeMap<SiteId, &'a mir_effects::MirSiteInventoryFact>,
    events: BTreeMap<SiteId, &'a mir_effects::MirEffectEventFact>,
    call_targets: BTreeMap<SiteId, &'a mir_effects::CallSiteTargetFact>,
    call_surfaces: BTreeMap<SiteId, &'a mir_effects::CallSiteSurfaceEffectFact>,
    boundaries: BTreeMap<SiteId, &'a mir_boundary::BoundarySourceContract>,
}

impl<'a> MirFactIndex<'a> {
    fn new(
        materialized: &MaterializedMir,
        mir_facts: &'a MirFacts,
    ) -> Result<Self, EffectFactsError> {
        let mut instance_by_stable_text = HashMap::new();
        let mut instance_by_callable_fqn = HashMap::new();
        for family in materialized.pass_view().instances() {
            let stable_key = materialized
                .authoritative_stable_instance_key(family.key())
                .ok_or_else(|| EffectFactsError::Frontend {
                    message: format!(
                        "callable family `{}` 缺少 authoritative stable instance key，无法消费 MIR facts",
                        family.key().template.fqn,
                    ),
                })?;
            instance_by_stable_text.insert(stable_key.canonical_text(), family.key().clone());
            for fqn in family.callable_fqns() {
                instance_by_callable_fqn.insert(fqn.to_string(), family.key().clone());
            }
        }

        let mut index = Self {
            instance_by_stable_text,
            instance_by_callable_fqn,
            callable_instances: HashMap::new(),
            bodies: HashMap::new(),
            source_signatures: mir_facts
                .backend
                .source_signatures
                .iter()
                .map(|signature| (signature.fqn.clone(), signature))
                .collect(),
        };

        for fact in &mir_facts.effects.callable_instances {
            let key = index.instance_for_artifact(&fact.instance)?.clone();
            index.callable_instances.insert(key, fact);
        }
        for fact in &mir_facts.effects.block_regions {
            let key = index.instance_for_artifact(&fact.instance)?.clone();
            index
                .body_mut(key, &fact.body.fqn)
                .regions
                .insert(BasicBlockId::from_raw(fact.block.as_u32()), fact);
        }
        for fact in &mir_facts.effects.site_inventory {
            let key = index.instance_for_artifact(&fact.instance)?.clone();
            index
                .body_mut(key, &fact.body.fqn)
                .sites
                .insert(fact.site_id, fact);
        }
        for fact in &mir_facts.effects.effect_events {
            let key = index.instance_for_artifact(&fact.instance)?.clone();
            index
                .body_mut(key, &fact.body.fqn)
                .events
                .insert(fact.site_id, fact);
        }
        for fact in &mir_facts.effects.call_site_targets {
            let key = index.instance_for_artifact(&fact.instance)?.clone();
            index
                .body_mut(key, &fact.body.fqn)
                .call_targets
                .insert(fact.site_id, fact);
        }
        for fact in &mir_facts.effects.call_site_surface_effects {
            let key = index.instance_for_artifact(&fact.instance)?.clone();
            index
                .body_mut(key, &fact.body.fqn)
                .call_surfaces
                .insert(fact.site_id, fact);
        }
        for fact in &mir_facts.boundary.source_contracts {
            let key = index.instance_for_artifact(&fact.instance)?.clone();
            index
                .body_mut(key, &fact.body.fqn)
                .boundaries
                .insert(fact.site_id, fact);
        }
        Ok(index)
    }

    fn callable_instance(
        &self,
        key: &InstanceKey,
    ) -> Result<&'a mir_effects::CallableInstanceEffectFacts, EffectFactsError> {
        self.callable_instances
            .get(key)
            .copied()
            .ok_or_else(|| EffectFactsError::MissingMirFact {
                kind: "CallableInstanceEffectFacts",
                detail: key.template.fqn.to_string(),
            })
    }

    fn body(
        &self,
        key: &InstanceKey,
        fqn: &str,
    ) -> Result<&MirBodyFactBundle<'a>, EffectFactsError> {
        self.bodies
            .get(&(key.clone(), fqn.to_string()))
            .ok_or_else(|| EffectFactsError::MissingMirFact {
                kind: "Mir body effect facts",
                detail: fqn.to_string(),
            })
    }

    fn source_signature(&self, fqn: &str) -> Option<&'a mir_backend::SourceCallableSignatureFact> {
        self.source_signatures.get(fqn).copied()
    }

    fn bodyless_direct_signature(&self, fqn: &str) -> Result<(), EffectFactsError> {
        let Some(signature) = self.source_signature(fqn) else {
            return Err(EffectFactsError::MissingMirFact {
                kind: "SourceCallableSignatureFact",
                detail: format!(
                    "bodyless direct target `{fqn}` lacks an upstream source signature fact"
                ),
            });
        };
        if signature.target_callable_key.is_none()
            || signature
                .abi_symbol
                .as_deref()
                .unwrap_or_default()
                .is_empty()
            || signature.abi_role.as_deref().unwrap_or_default().is_empty()
        {
            return Err(EffectFactsError::MissingMirFact {
                kind: "SourceCallableSignatureFact",
                detail: format!(
                    "bodyless direct target `{fqn}` lacks target-bound callable/ABI publication"
                ),
            });
        }
        Ok(())
    }

    fn instance_for_stable_text(&self, text: &str) -> Result<InstanceKey, EffectFactsError> {
        self.instance_by_stable_text
            .get(text)
            .cloned()
            .ok_or_else(|| EffectFactsError::UnknownMirFactInstance {
                key: text.to_string(),
            })
    }

    fn instance_for_callable_fqn(&self, fqn: &str) -> Option<InstanceKey> {
        self.instance_by_callable_fqn.get(fqn).cloned()
    }

    fn instance_for_artifact(
        &self,
        artifact: &scoopc_ids::StageArtifactKey,
    ) -> Result<&InstanceKey, EffectFactsError> {
        self.instance_by_stable_text
            .get(artifact.owner_canonical_text())
            .ok_or_else(|| EffectFactsError::UnknownMirFactInstance {
                key: artifact.owner_canonical_text().to_string(),
            })
    }

    fn body_mut(&mut self, key: InstanceKey, fqn: &str) -> &mut MirBodyFactBundle<'a> {
        self.bodies.entry((key, fqn.to_string())).or_default()
    }
}

impl EffectFactsTypeContext {
    fn concrete_effect_op_contract_for_site(
        &self,
        types: &mut TypeStore,
        effect_ty: TypeId,
        op_fqn: &str,
        op_type_args: &[TypeId],
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
            op_type_args,
        )
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
        let continuation_obj_ty = continuation_object_ty(types, &seed.stable_instance_key_text);
        let case_seeds = self.type_ctx.step_case_seeds(
            types,
            &seed.step_effect_row,
            &seed.body_concrete_effect_ops,
        )?;
        self.intern_step_schema_from_case_seeds(
            types,
            invoke_args_tuple_ty,
            seed.complete_ty,
            continuation_obj_ty,
            &seed.surface_effect_row,
            case_seeds,
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
        let case_seeds = self.type_ctx.step_case_seeds(types, effect_row, &[])?;
        self.intern_step_schema_from_case_seeds(
            types,
            invoke_args_tuple_ty,
            complete_ty,
            continuation_obj_ty,
            continuation_surface_row,
            case_seeds,
        )
    }

    fn intern_step_schema_from_case_seeds(
        &mut self,
        types: &mut TypeStore,
        invoke_args_tuple_ty: TypeId,
        complete_ty: TypeId,
        continuation_obj_ty: TypeId,
        continuation_surface_row: &EffectRow,
        case_seeds: Vec<StepCaseSeed>,
    ) -> Result<StepSchemaId, EffectFactsError> {
        let step_schema_id = StepSchemaId::new(self.next_step_schema_id);
        self.next_step_schema_id += 1;
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

impl<'ctx, 'facts, 'pool> BodyFactsBuilder<'ctx, 'facts, 'pool> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        type_ctx: &'ctx EffectFactsTypeContext,
        schema_pool: &'pool mut EffectFactsSchemaPool<'ctx>,
        mir_fact_index: &'ctx MirFactIndex<'facts>,
        mir_body_facts: &'ctx MirBodyFactBundle<'facts>,
        callable_facts: &'ctx HashMap<InstanceKey, CallableEffectFacts>,
        callable_fun: &'ctx MirFunDecl,
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
            mir_fact_index,
            mir_body_facts,
            callable_facts,
            callable_fun,
            callable_step_schema,
            current_case_index,
            sites: BTreeMap::new(),
            block_drafts: BTreeMap::new(),
            block_site_ids: BTreeMap::new(),
            block_handled_tags: BTreeMap::new(),
            handle_site_solver_facts: BTreeMap::new(),
        })
    }

    fn build(mut self, types: &mut TypeStore) -> Result<BodyEffectFacts, EffectFactsError> {
        if self.callable_fun.body.is_none() {
            return Ok(BodyEffectFacts::default());
        }

        let mut block_successors = BTreeMap::new();
        let mut cleanup_blocks = BTreeSet::new();
        for (block_id, region) in &self.mir_body_facts.regions {
            block_successors.insert(
                *block_id,
                region
                    .successors
                    .iter()
                    .map(|successor| BasicBlockId::from_raw(successor.as_u32()))
                    .collect::<Vec<_>>(),
            );
            self.block_site_ids
                .insert(*block_id, region.site_ids.clone());
            if region.cleanup {
                cleanup_blocks.insert(*block_id);
            }
        }

        let site_ids = self
            .mir_body_facts
            .sites
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for site_id in site_ids {
            let _ = self.ensure_site_facts_from_mir(types, site_id)?;
        }

        let mut blocks = BTreeMap::new();
        for block_id in self.mir_body_facts.regions.keys().copied() {
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

    fn ensure_site_facts_from_mir(
        &mut self,
        types: &mut TypeStore,
        site_id: SiteId,
    ) -> Result<RegionCaseContribution, EffectFactsError> {
        let event = self.mir_body_facts.events.get(&site_id).ok_or_else(|| {
            EffectFactsError::MissingMirFact {
                kind: "MirEffectEventFact",
                detail: format!("{} site{}", self.callable_fun.fqn, site_id.as_u32()),
            }
        })?;
        let block_id = BasicBlockId::from_raw(event.block.as_u32());
        if self.sites.contains_key(&site_id) {
            return Ok(self.region_contribution_for_site(site_id, event.cleanup));
        }

        let mut direct = RegionCaseContribution::default();
        let mut draft = self.block_drafts.remove(&block_id).unwrap_or_default();
        match &event.kind {
            mir_effects::MirEffectEventKind::Call { call_kind } => {
                self.ensure_call_site_facts_from_mir(types, site_id, *call_kind)?;
                if !event.effect_row.terms.is_empty() {
                    draft.has_suspend_boundary = true;
                }
            }
            mir_effects::MirEffectEventKind::ClassCtor { .. }
            | mir_effects::MirEffectEventKind::HiddenInitializer { .. } => {
                let row = effect_row_from_fact_template(types, &event.effect_row)?;
                let projected = self.ensure_class_ctor_site_facts_from_row(types, site_id, &row)?;
                if !projected.is_empty() {
                    draft.has_suspend_boundary = true;
                }
                direct.add_tags(event.cleanup, projected);
            }
            mir_effects::MirEffectEventKind::Perform { op } => {
                let emitted = self.ensure_perform_site_facts_from_contract(types, site_id, op)?;
                direct.add_tags(event.cleanup, [emitted]);
                draft.has_suspend_boundary = true;
            }
            mir_effects::MirEffectEventKind::Resume {
                resume_tuple_ty,
                answer_ty,
                continuation_ty,
                surface_row,
            } => {
                let out_row = effect_row_from_fact_template(types, &event.effect_row)?;
                let surface_row = effect_row_from_fact_template(types, surface_row)?;
                let projected = self.ensure_resume_site_facts_from_mir(
                    types,
                    site_id,
                    *resume_tuple_ty,
                    *answer_ty,
                    *continuation_ty,
                    &out_row,
                    &surface_row,
                )?;
                if !projected.is_empty() {
                    draft.has_suspend_boundary = true;
                }
                direct.add_tags(event.cleanup, projected);
            }
            mir_effects::MirEffectEventKind::Handle {
                result_ty,
                body_target,
                arm_targets,
                finally_target,
                exit_target,
                arms,
            } => {
                let outward = self.ensure_handle_site_facts_from_mir(
                    types,
                    site_id,
                    *result_ty,
                    BasicBlockId::from_raw(body_target.as_u32()),
                    arm_targets
                        .iter()
                        .map(|target| BasicBlockId::from_raw(target.as_u32()))
                        .collect(),
                    finally_target.map(|target| BasicBlockId::from_raw(target.as_u32())),
                    BasicBlockId::from_raw(exit_target.as_u32()),
                    arms,
                )?;
                direct.add_tags(event.cleanup, outward);
                draft.has_handle_boundary = true;
                if matches!(
                    self.sites.get(&site_id),
                    Some(SiteEffectFacts::Handle(facts))
                        if facts.nested_handle_classification()
                            == NestedHandleClassification::MaySuspendOutward
                ) {
                    draft.has_suspend_boundary = true;
                }
            }
        }
        draft.outward_tags.extend(direct.total_tags());
        self.block_drafts.insert(block_id, draft);
        Ok(direct)
    }

    fn region_contribution_for_site(
        &self,
        site_id: SiteId,
        is_cleanup: bool,
    ) -> RegionCaseContribution {
        let mut contribution = RegionCaseContribution::default();
        match self.sites.get(&site_id) {
            Some(SiteEffectFacts::ClassCtor(facts)) => {
                contribution.add_tags(is_cleanup, facts.emitted_cases().tags().iter().copied());
            }
            Some(SiteEffectFacts::Perform(facts)) => {
                contribution.add_tags(is_cleanup, [facts.emitted_case()]);
            }
            Some(SiteEffectFacts::Resume(facts)) => {
                let tags = self
                    .schema_pool
                    .project_case_set(facts.resolved_cases(), &self.current_case_index);
                contribution.add_tags(is_cleanup, tags);
            }
            Some(SiteEffectFacts::Handle(facts)) => {
                contribution.add_tags(is_cleanup, handle_total_outward_tags(facts));
            }
            Some(SiteEffectFacts::Call(_)) | None => {}
        }
        contribution
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

        let Some(region) = self.mir_body_facts.regions.get(&entry) else {
            return;
        };
        if !region.cleanup {
            self.block_handled_tags
                .entry(entry)
                .or_default()
                .extend(tags.iter().copied());
        }

        let successors = region
            .successors
            .iter()
            .map(|target| BasicBlockId::from_raw(target.as_u32()))
            .collect::<Vec<_>>();
        for target in successors {
            self.mark_region_handled_cases(target, stops, tags, visited);
        }
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

        let Some(region) = self.mir_body_facts.regions.get(&entry) else {
            return Ok(RegionCaseContribution::default());
        };
        let site_ids = region.site_ids.clone();
        let successors = region
            .successors
            .iter()
            .map(|successor| BasicBlockId::from_raw(successor.as_u32()))
            .collect::<Vec<_>>();
        let mut acc = RegionCaseContribution::default();
        for site_id in site_ids {
            acc.extend(self.ensure_site_facts_from_mir(types, site_id)?);
        }
        for successor in successors {
            acc.extend(self.collect_region_cases(types, successor, stops, visited)?);
        }
        Ok(acc)
    }

    fn ensure_call_site_facts_from_mir(
        &mut self,
        types: &mut TypeStore,
        site_id: SiteId,
        call_kind: mir_effects::MirCallKind,
    ) -> Result<(), EffectFactsError> {
        if self.sites.contains_key(&site_id) {
            return Ok(());
        }
        let site = self.required_site_inventory(site_id)?;
        let boundary = self.required_boundary(site_id)?;
        let surface = self.required_call_surface(site_id)?;
        let target = self.required_call_target(site_id)?;
        let kind = call_site_kind_from_mir(call_kind);
        let invoke_args_tuple_ty =
            self.invoke_args_tuple_ty_from_boundary(types, site_id, boundary)?;
        let result_ty = site
            .result_ty
            .ok_or_else(|| EffectFactsError::MissingMirFact {
                kind: "MirSiteInventoryFact.result_ty",
                detail: format!("{} site{}", self.callable_fun.fqn, site_id.as_u32()),
            })?;
        let surface_row = effect_row_from_fact_template(types, &surface.surface_row)?;
        let facts = self.call_site_facts_from_published_target(
            types,
            kind,
            target,
            invoke_args_tuple_ty,
            result_ty,
            &surface_row,
        )?;
        self.sites.insert(site_id, SiteEffectFacts::Call(facts));
        Ok(())
    }

    fn ensure_class_ctor_site_facts_from_row(
        &mut self,
        types: &TypeStore,
        site_id: SiteId,
        hidden_effects: &EffectRow,
    ) -> Result<BTreeSet<CaseTag>, EffectFactsError> {
        if let Some(SiteEffectFacts::ClassCtor(facts)) = self.sites.get(&site_id) {
            return Ok(facts.emitted_cases().tags().iter().copied().collect());
        }
        let projected = self.current_cases_for_effect_row(types, hidden_effects)?;
        let cases = self.case_set_from_tags(projected.clone());
        self.sites.insert(
            site_id,
            SiteEffectFacts::ClassCtor(ClassCtorSiteEffectFacts::new(cases)),
        );
        Ok(projected)
    }

    fn ensure_perform_site_facts_from_contract(
        &mut self,
        types: &mut TypeStore,
        site_id: SiteId,
        op: &mir_effects::MirEffectOpSiteContract,
    ) -> Result<CaseTag, EffectFactsError> {
        if let Some(SiteEffectFacts::Perform(facts)) = self.sites.get(&site_id) {
            return Ok(facts.emitted_case());
        }
        let case_info =
            self.current_case_for_effect_op(types, op.effect_ty, &op.op_fqn, &op.op_type_args)?;
        self.sites.insert(
            site_id,
            SiteEffectFacts::Perform(PerformSiteEffectFacts::new(
                case_info.tag,
                op.payload_tuple_ty,
                case_info.continuation_schema,
            )),
        );
        Ok(case_info.tag)
    }

    #[allow(clippy::too_many_arguments)]
    fn ensure_resume_site_facts_from_mir(
        &mut self,
        types: &mut TypeStore,
        site_id: SiteId,
        resume_tuple_ty: TypeId,
        answer_ty: TypeId,
        continuation_ty: TypeId,
        out_row: &EffectRow,
        surface_row: &EffectRow,
    ) -> Result<BTreeSet<CaseTag>, EffectFactsError> {
        if let Some(SiteEffectFacts::Resume(facts)) = self.sites.get(&site_id) {
            return Ok(self
                .schema_pool
                .project_case_set(facts.resolved_cases(), &self.current_case_index));
        }
        let out_step_schema = self.schema_pool.intern_synthetic_step_schema(
            types,
            resume_tuple_ty,
            answer_ty,
            out_row,
            surface_row,
            SyntheticStepSchemaKind::ResumeSurface,
        )?;
        let continuation_schema = self.schema_pool.intern_continuation_schema(
            resume_tuple_ty,
            answer_ty,
            out_step_schema,
            continuation_ty,
        );
        let resolved_cases = self.schema_pool.full_case_set(out_step_schema);
        let projected = self
            .schema_pool
            .project_case_set(&resolved_cases, &self.current_case_index);
        self.sites.insert(
            site_id,
            SiteEffectFacts::Resume(ResumeSiteEffectFacts::new(
                continuation_schema,
                resume_tuple_ty,
                answer_ty,
                out_step_schema,
                resolved_cases,
            )),
        );
        Ok(projected)
    }

    #[allow(clippy::too_many_arguments)]
    fn ensure_handle_site_facts_from_mir(
        &mut self,
        types: &mut TypeStore,
        site_id: SiteId,
        result_ty: TypeId,
        body_target: BasicBlockId,
        arm_targets: Vec<BasicBlockId>,
        finally_target: Option<BasicBlockId>,
        exit_target: BasicBlockId,
        arms: &[mir_effects::MirEffectOpSiteContract],
    ) -> Result<BTreeSet<CaseTag>, EffectFactsError> {
        self.handle_site_solver_facts.insert(
            site_id,
            HandleSiteSolverFacts::new(
                body_target,
                arm_targets.clone(),
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
            let case_info = self.current_case_for_effect_op(
                types,
                arm.effect_ty,
                &arm.op_fqn,
                &arm.op_type_args,
            )?;
            handled_tags.insert(case_info.tag);

            let arm_cases =
                self.collect_region_cases(types, arm_target, &body_stops, &mut BTreeSet::new())?;
            cleanup_outward.extend(arm_cases.cleanup.iter().copied());
            arm_non_cleanup.extend(arm_cases.non_cleanup.iter().copied());
            arm_facts.push(HandleArmEffectFacts::new(
                case_info.tag,
                arm.payload_tuple_ty,
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
            result_ty,
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

    #[allow(clippy::too_many_arguments)]
    fn call_site_facts_from_published_target(
        &mut self,
        types: &mut TypeStore,
        kind: CallSiteKind,
        target: &mir_effects::CallSiteTargetFact,
        invoke_args_tuple_ty: TypeId,
        result_ty: TypeId,
        surface_row: &EffectRow,
    ) -> Result<CallSiteEffectFacts, EffectFactsError> {
        match &target.target {
            mir_effects::CallSiteTarget::KnownInstance { key } => {
                let target_key = self.mir_fact_index.instance_for_stable_text(key.as_str())?;
                self.call_site_for_known_instance(
                    types,
                    kind,
                    target_key,
                    invoke_args_tuple_ty,
                    result_ty,
                    surface_row,
                )
            }
            mir_effects::CallSiteTarget::CandidateSet { keys } => {
                if keys.is_empty() {
                    return Err(EffectFactsError::MissingMirFact {
                        kind: "CallSiteTargetFact.CandidateSet",
                        detail: format!(
                            "{} site{} published an empty candidate set",
                            self.callable_fun.fqn,
                            target.site_id.as_u32()
                        ),
                    });
                }
                let target_keys = keys
                    .iter()
                    .map(|key| self.mir_fact_index.instance_for_stable_text(key.as_str()))
                    .collect::<Result<Vec<_>, _>>()?;
                self.call_site_for_candidate_set(
                    types,
                    kind,
                    target_keys,
                    invoke_args_tuple_ty,
                    result_ty,
                    surface_row,
                )
            }
            mir_effects::CallSiteTarget::DirectFunction { fqn } => {
                if let Some(target_key) = self.mir_fact_index.instance_for_callable_fqn(fqn) {
                    self.call_site_for_known_instance(
                        types,
                        kind,
                        target_key,
                        invoke_args_tuple_ty,
                        result_ty,
                        surface_row,
                    )
                } else {
                    if !surface_row.is_pure() {
                        let _ = (types, kind, invoke_args_tuple_ty, result_ty);
                        return Err(EffectFactsError::MissingMirFact {
                            kind: "CallSiteTargetFact.DirectFunction",
                            detail: format!(
                                "{} direct call target `{fqn}` has no published stable callable target",
                                self.callable_fun.fqn
                            ),
                        });
                    }
                    self.mir_fact_index.bodyless_direct_signature(fqn)?;
                    self.call_site_for_surface_row(
                        types,
                        kind,
                        CallSiteTarget::BodylessDirect {
                            fqn: fqn.to_string(),
                        },
                        invoke_args_tuple_ty,
                        result_ty,
                        surface_row,
                        EffectPrecision::Precise,
                    )
                }
            }
            mir_effects::CallSiteTarget::KnownClosure { fn_ptr } => {
                if let Some(target_key) = self.mir_fact_index.instance_for_callable_fqn(fn_ptr) {
                    self.call_site_for_known_instance(
                        types,
                        kind,
                        target_key,
                        invoke_args_tuple_ty,
                        result_ty,
                        surface_row,
                    )
                } else {
                    Err(EffectFactsError::MissingMirFact {
                        kind: "CallSiteTargetFact.KnownClosure",
                        detail: format!(
                            "{} closure target `{fn_ptr}` has no stable callable instance",
                            self.callable_fun.fqn
                        ),
                    })
                }
            }
            mir_effects::CallSiteTarget::Dynamic if matches!(kind, CallSiteKind::FunPtr) => {
                Ok(CallSiteEffectFacts::new_plain(
                    kind,
                    CallSiteTarget::DynamicFallback,
                    invoke_args_tuple_ty,
                    CaseSet::new(self.callable_step_schema, Vec::new()),
                    EffectPrecision::Precise,
                ))
            }
            mir_effects::CallSiteTarget::Dynamic => self.call_site_for_surface_row(
                types,
                kind,
                CallSiteTarget::DynamicFallback,
                invoke_args_tuple_ty,
                result_ty,
                surface_row,
                Self::dynamic_surface_precision(surface_row),
            ),
            mir_effects::CallSiteTarget::Param { .. }
            | mir_effects::CallSiteTarget::Join { .. } => self.call_site_for_surface_row(
                types,
                kind,
                CallSiteTarget::DynamicFallback,
                invoke_args_tuple_ty,
                result_ty,
                surface_row,
                Self::dynamic_surface_precision(surface_row),
            ),
            mir_effects::CallSiteTarget::DynamicFallback { reason } => match reason {
                mir_effects::DynamicFallbackReason::OpenParam => self.call_site_for_surface_row(
                    types,
                    kind,
                    CallSiteTarget::DynamicFallback,
                    invoke_args_tuple_ty,
                    result_ty,
                    surface_row,
                    Self::dynamic_surface_precision(surface_row),
                ),
                mir_effects::DynamicFallbackReason::UnknownCallable
                    if matches!(kind, CallSiteKind::Closure | CallSiteKind::FunValue) =>
                {
                    self.call_site_for_surface_row(
                        types,
                        kind,
                        CallSiteTarget::DynamicFallback,
                        invoke_args_tuple_ty,
                        result_ty,
                        surface_row,
                        Self::dynamic_surface_precision(surface_row),
                    )
                }
                mir_effects::DynamicFallbackReason::NativeFunPtr
                    if matches!(kind, CallSiteKind::FunPtr) =>
                {
                    Ok(CallSiteEffectFacts::new_plain(
                        kind,
                        CallSiteTarget::DynamicFallback,
                        invoke_args_tuple_ty,
                        CaseSet::new(self.callable_step_schema, Vec::new()),
                        EffectPrecision::Precise,
                    ))
                }
                mir_effects::DynamicFallbackReason::UnknownCallable
                | mir_effects::DynamicFallbackReason::EmptyCandidateSet
                | mir_effects::DynamicFallbackReason::NativeFunPtr => {
                    Err(EffectFactsError::MissingMirFact {
                        kind: "CallSiteTargetFact.DynamicFallback",
                        detail: format!(
                            "{} site{} published unsupported fallback reason {reason:?}",
                            self.callable_fun.fqn,
                            target.site_id.as_u32()
                        ),
                    })
                }
            },
        }
    }

    fn dynamic_surface_precision(surface_row: &EffectRow) -> EffectPrecision {
        if surface_row.is_pure() {
            EffectPrecision::Precise
        } else {
            EffectPrecision::Widened
        }
    }

    fn call_site_for_known_instance(
        &mut self,
        _types: &mut TypeStore,
        kind: CallSiteKind,
        target_key: InstanceKey,
        invoke_args_tuple_ty: TypeId,
        _result_ty: TypeId,
        _surface_row: &EffectRow,
    ) -> Result<CallSiteEffectFacts, EffectFactsError> {
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
            let precision = if facts.resolved_outward_cases().is_empty() {
                EffectPrecision::Precise
            } else {
                EffectPrecision::Widened
            };
            return Ok(CallSiteEffectFacts::new(
                kind,
                CallSiteTarget::KnownInstance(target_key),
                facts.invoke_args_tuple_ty(),
                facts.step_schema(),
                facts.resolved_outward_cases().clone(),
                precision,
            ));
        }
        if _surface_row.is_pure() {
            self.mir_fact_index
                .bodyless_direct_signature(&target_key.template.fqn)?;
            return self.call_site_for_surface_row(
                _types,
                kind,
                CallSiteTarget::BodylessDirect {
                    fqn: target_key.template.fqn.clone(),
                },
                invoke_args_tuple_ty,
                _result_ty,
                _surface_row,
                EffectPrecision::Precise,
            );
        }
        Err(EffectFactsError::MissingMirFact {
            kind: "CallableInstanceEffectFacts",
            detail: format!(
                "{} known call target `{}` has no callable effect facts",
                self.callable_fun.fqn, target_key.template.fqn
            ),
        })
    }

    fn call_site_for_candidate_set(
        &mut self,
        types: &mut TypeStore,
        kind: CallSiteKind,
        target_keys: Vec<InstanceKey>,
        invoke_args_tuple_ty: TypeId,
        result_ty: TypeId,
        surface_row: &EffectRow,
    ) -> Result<CallSiteEffectFacts, EffectFactsError> {
        self.call_site_for_surface_row(
            types,
            kind,
            CallSiteTarget::CandidateSet(target_keys),
            invoke_args_tuple_ty,
            result_ty,
            surface_row,
            if surface_row.is_pure() {
                EffectPrecision::Precise
            } else {
                EffectPrecision::Widened
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn call_site_for_surface_row(
        &mut self,
        types: &mut TypeStore,
        kind: CallSiteKind,
        target: CallSiteTarget,
        invoke_args_tuple_ty: TypeId,
        result_ty: TypeId,
        surface_row: &EffectRow,
        precision: EffectPrecision,
    ) -> Result<CallSiteEffectFacts, EffectFactsError> {
        if surface_row.is_pure() {
            return Ok(CallSiteEffectFacts::new_plain(
                kind,
                target,
                invoke_args_tuple_ty,
                CaseSet::new(self.callable_step_schema, Vec::new()),
                precision,
            ));
        }
        let step_schema = self.schema_pool.intern_synthetic_step_schema(
            types,
            invoke_args_tuple_ty,
            result_ty,
            surface_row,
            surface_row,
            SyntheticStepSchemaKind::CallSurface,
        )?;
        Ok(CallSiteEffectFacts::new(
            kind,
            target,
            invoke_args_tuple_ty,
            step_schema,
            self.schema_pool.full_case_set(step_schema),
            precision,
        ))
    }

    fn invoke_args_tuple_ty_from_boundary(
        &self,
        types: &mut TypeStore,
        site_id: SiteId,
        boundary: &mir_boundary::BoundarySourceContract,
    ) -> Result<TypeId, EffectFactsError> {
        let arg_tys = boundary
            .args
            .iter()
            .map(|source| {
                boundary_operand_ty(source).ok_or_else(|| EffectFactsError::MissingMirFact {
                    kind: "BoundaryOperandSource.ty",
                    detail: format!("{} site{}", self.callable_fun.fqn, site_id.as_u32()),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(canonical_tuple_carrier_ty(types, &arg_tys))
    }

    fn required_site_inventory(
        &self,
        site_id: SiteId,
    ) -> Result<&'facts mir_effects::MirSiteInventoryFact, EffectFactsError> {
        self.mir_body_facts
            .sites
            .get(&site_id)
            .copied()
            .ok_or_else(|| EffectFactsError::MissingMirFact {
                kind: "MirSiteInventoryFact",
                detail: format!("{} site{}", self.callable_fun.fqn, site_id.as_u32()),
            })
    }

    fn required_boundary(
        &self,
        site_id: SiteId,
    ) -> Result<&'facts mir_boundary::BoundarySourceContract, EffectFactsError> {
        self.mir_body_facts
            .boundaries
            .get(&site_id)
            .copied()
            .ok_or_else(|| EffectFactsError::MissingMirFact {
                kind: "BoundarySourceContract",
                detail: format!("{} site{}", self.callable_fun.fqn, site_id.as_u32()),
            })
    }

    fn required_call_surface(
        &self,
        site_id: SiteId,
    ) -> Result<&'facts mir_effects::CallSiteSurfaceEffectFact, EffectFactsError> {
        self.mir_body_facts
            .call_surfaces
            .get(&site_id)
            .copied()
            .ok_or_else(|| EffectFactsError::MissingMirFact {
                kind: "CallSiteSurfaceEffectFact",
                detail: format!("{} site{}", self.callable_fun.fqn, site_id.as_u32()),
            })
    }

    fn required_call_target(
        &self,
        site_id: SiteId,
    ) -> Result<&'facts mir_effects::CallSiteTargetFact, EffectFactsError> {
        self.mir_body_facts
            .call_targets
            .get(&site_id)
            .copied()
            .ok_or_else(|| EffectFactsError::MissingMirFact {
                kind: "CallSiteTargetFact",
                detail: format!("{} site{}", self.callable_fun.fqn, site_id.as_u32()),
            })
    }

    fn current_cases_for_effect_row(
        &self,
        types: &TypeStore,
        row: &EffectRow,
    ) -> Result<BTreeSet<CaseTag>, EffectFactsError> {
        let mut tags = BTreeSet::new();
        for effect_ty in &row.terms {
            let (effect_fqn, effect_type_args) = lower_effect_nominal_identity(types, *effect_ty)?;
            for (op_key, case_info) in &self.current_case_index {
                let family = op_key.effect_family();
                if family.effect_fqn() == effect_fqn
                    && family.type_args() == effect_type_args.as_slice()
                {
                    tags.insert(case_info.tag);
                }
            }
        }
        Ok(tags)
    }
    fn current_case_for_effect_op(
        &self,
        types: &mut TypeStore,
        effect_ty: TypeId,
        op_fqn: &str,
        op_type_args: &[TypeId],
    ) -> Result<CurrentBodyCaseInfo, EffectFactsError> {
        let contract = self.type_ctx.concrete_effect_op_contract_for_site(
            types,
            effect_ty,
            op_fqn,
            op_type_args,
        )?;
        self.current_case_index
            .get(&contract.concrete_op_key)
            .cloned()
            .ok_or_else(|| EffectFactsError::MissingCallableCase {
                callable: self.callable_fun.fqn.clone(),
                op_fqn: op_fqn.to_string(),
            })
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
    /// Machine-independent canonical text for `key`, used as the disambiguator in
    /// `ContinuationObject@<...>` names so the resulting LIR body hash is portable.
    stable_instance_key_text: String,
    root_fun: MirFunDecl,
    declared_row: EffectRow,
    // `surface_effect_row` 只表达源码层 residual row；`step_effect_row` 允许额外带上
    // compiler-generated one-shot runtime-error upper bound，供 out-step contract 使用。
    surface_effect_row: EffectRow,
    step_effect_row: EffectRow,
    invoke_arg_components: Vec<TypeId>,
    complete_ty: TypeId,
    body_concrete_effect_ops: Vec<ConcreteEffectOpContract>,
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

#[derive(Debug, Clone)]
struct ConcreteEffectOpContract {
    concrete_op_key: ConcreteOpKey,
    payload_tuple_ty: TypeId,
    resume_tuple_ty: TypeId,
}

/// Analysis-only context borrowed from the HIR semantic artifact so P4 can interpret declarations.
///
/// This is not a replacement owner for HIR/MIR facts: it is private builder state copied from the
/// upstream artifact to lower effect signatures into the effect-owned type context published on
/// `MaterializedEffectFacts`.
#[derive(Debug)]
struct EffectFactsTypeContext {
    stable_cone_key: StableConeKey,
    index: Index,
    env: TypeEnv,
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
struct BodyFactsBuilder<'ctx, 'facts, 'pool> {
    type_ctx: &'ctx EffectFactsTypeContext,
    schema_pool: &'pool mut EffectFactsSchemaPool<'ctx>,
    mir_fact_index: &'ctx MirFactIndex<'facts>,
    mir_body_facts: &'ctx MirBodyFactBundle<'facts>,
    callable_facts: &'ctx HashMap<InstanceKey, CallableEffectFacts>,
    callable_fun: &'ctx MirFunDecl,
    callable_step_schema: StepSchemaId,
    current_case_index: HashMap<ConcreteOpKey, CurrentBodyCaseInfo>,
    sites: BTreeMap<SiteId, SiteEffectFacts>,
    block_drafts: BTreeMap<BasicBlockId, BlockDraft>,
    block_site_ids: BTreeMap<BasicBlockId, Vec<SiteId>>,
    block_handled_tags: BTreeMap<BasicBlockId, BTreeSet<CaseTag>>,
    handle_site_solver_facts: BTreeMap<SiteId, HandleSiteSolverFacts>,
}

impl<'a> MaterializedEffectFactsBuilder<'a> {
    pub fn from_materialized_snapshot(
        frontend_artifact: &'a HirSemanticArtifact,
        materialized: &'a MaterializedMir,
        mir_facts: &'a MirFacts,
        type_context: &'a mut EffectOwnedTypeContext,
    ) -> Self {
        Self {
            frontend_artifact,
            materialized,
            mir_facts,
            type_context,
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
        let type_ctx = EffectFactsTypeContext::from_frontend_artifact(self.frontend_artifact);
        let compiler_generated_runtime_error_effect_ty =
            find_or_intern_raise_runtime_error_effect(self.type_context.types_mut());
        let mir_fact_index = MirFactIndex::new(self.materialized, self.mir_facts)?;
        let callable_seeds = collect_callable_seeds(
            self.materialized,
            self.type_context.types_mut(),
            &type_ctx,
            &type_ctx.index,
            &mir_fact_index,
            &self.compiler_continuation_runtime_error_callables,
            compiler_generated_runtime_error_effect_ty,
        )?;

        let mut callable_facts = HashMap::with_capacity(callable_seeds.len());
        let mut bodies = HashMap::with_capacity(callable_seeds.len());
        let mut callable_step_schemas = HashMap::with_capacity(callable_seeds.len());
        let mut schema_pool = EffectFactsSchemaPool::new(&type_ctx);

        {
            let types = self.type_context.types_mut();
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
                    let mir_body_facts = mir_fact_index.body(&seed.key, &seed.root_fun.fqn)?;
                    BodyFactsBuilder::new(
                        &type_ctx,
                        &mut schema_pool,
                        &mir_fact_index,
                        mir_body_facts,
                        &callable_facts,
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
        }

        let (step_schemas, continuation_schemas) = schema_pool.finish();

        Ok(MaterializedEffectFacts::new(
            self.type_context.clone(),
            snapshot_binding,
            step_schemas,
            continuation_schemas,
            callable_facts,
            bodies,
        ))
    }
}

impl EffectFactsTypeContext {
    fn from_frontend_artifact(frontend_artifact: &HirSemanticArtifact) -> Self {
        Self {
            stable_cone_key: frontend_artifact.stable_cone_key().clone(),
            index: frontend_artifact.index().clone(),
            env: frontend_artifact.type_env().clone(),
        }
    }

    fn step_case_seeds(
        &self,
        types: &mut TypeStore,
        declared_row: &EffectRow,
        body_concrete_effect_ops: &[ConcreteEffectOpContract],
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
                let mut concrete_cases = body_concrete_effect_ops
                    .iter()
                    .filter(|contract| {
                        contract.concrete_op_key.effect_family().effect_fqn() == effect_fqn
                            && contract.concrete_op_key.effect_family().type_args()
                                == effect_type_args.as_slice()
                            && contract.concrete_op_key.instance_key().template.fqn == op_fqn
                    })
                    .map(|contract| StepCaseSeed {
                        sort_key: format!(
                            "{effect_display}::{op_fqn}::{}::{}",
                            types.display(contract.payload_tuple_ty),
                            types.display(contract.resume_tuple_ty)
                        ),
                        concrete_op_key: contract.concrete_op_key.clone(),
                        payload_tuple_ty: contract.payload_tuple_ty,
                        resume_tuple_ty: contract.resume_tuple_ty,
                    })
                    .collect::<Vec<_>>();
                concrete_cases.sort_by(|lhs, rhs| lhs.sort_key.cmp(&rhs.sort_key));
                let mut seen = HashSet::new();
                concrete_cases.retain(|case| seen.insert(case.concrete_op_key.clone()));
                if concrete_cases.is_empty() {
                    let contract = self.lower_effect_op_contract(
                        types,
                        &effect_fqn,
                        &effect_type_args,
                        &op_fqn,
                        &op,
                        effect_sym,
                        &[],
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
                } else {
                    cases.extend(concrete_cases);
                }
            }
        }

        cases.sort_by(|lhs, rhs| lhs.sort_key.cmp(&rhs.sort_key));
        Ok(cases)
    }

    #[allow(clippy::too_many_arguments)]
    fn lower_effect_op_contract(
        &self,
        types: &mut TypeStore,
        effect_fqn: &str,
        effect_type_args: &[TypeId],
        op_fqn: &str,
        op: &FunOverload,
        effect_sym: &TypeSymbol,
        op_type_args: &[TypeId],
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
        let use_concrete_op_type_args = op.sig.type_params.len() == op_type_args.len();
        for (index, type_param) in op.sig.type_params.iter().enumerate() {
            let ty = if use_concrete_op_type_args {
                op_type_args[index]
            } else {
                types.ty_param(TypeParamType {
                    name: type_param.name.clone(),
                    decl_file: op.symbol.decl_file.clone(),
                    decl_span: type_param.name_span,
                })
            };
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
        let declaration_kind =
            if op.sig.type_params.is_empty() && effect_sym.type_param_names.is_empty() {
                "effect_op"
            } else {
                "generic_effect_op"
            };
        let owner_def_key = StableDefKey::new(
            self.stable_cone_key.clone(),
            StableDefNamespace::Fun,
            op_fqn,
            declaration_kind,
            None,
        );
        let owner_def_key_text = owner_def_key.canonical_text();
        let mut signature_resolver = HashMap::new();
        let mut op_signature_type_bindings = Vec::new();
        let mut effect_signature_type_bindings = Vec::new();
        for (index, type_param) in effect_sym.type_param_names.iter().enumerate() {
            let placeholder = TypeParamType {
                name: type_param.clone(),
                decl_file: op.symbol.decl_file.clone(),
                decl_span: op.symbol.span,
            };
            let ty = types.ty_param(placeholder.clone());
            signature_resolver.insert(
                placeholder.clone(),
                StableTypeParamKey::new(owner_def_key_text.clone(), index),
            );
            effect_signature_type_bindings.push((type_param.clone(), ty));
        }
        for (index, type_param) in op.sig.type_params.iter().enumerate() {
            let placeholder = TypeParamType {
                name: type_param.name.clone(),
                decl_file: op.symbol.decl_file.clone(),
                decl_span: type_param.name_span,
            };
            let ty = types.ty_param(placeholder.clone());
            signature_resolver.insert(
                placeholder.clone(),
                StableTypeParamKey::new(
                    owner_def_key_text.clone(),
                    effect_sym.type_param_names.len() + index,
                ),
            );
            op_signature_type_bindings.push((type_param.name.clone(), ty));
        }
        let mut signature_type_bindings = op_signature_type_bindings;
        signature_type_bindings.extend(effect_signature_type_bindings);
        let signature_fun_ty = {
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
            let receiver_ty = op
                .sig
                .receiver
                .as_ref()
                .map(|receiver_ref| {
                    lower
                        .lower_type_ref_in_decl_file_with_scopes(
                            &op.symbol.decl_file,
                            signature_type_bindings.clone(),
                            std::iter::empty::<(String, EffectRow)>(),
                            receiver_ref,
                        )
                        .map_err(|error| EffectFactsError::TypeLower(Box::new(error)))
                })
                .transpose()?;
            let mut param_tys = Vec::with_capacity(op.sig.params.len());
            for param in &op.sig.params {
                let Some(param_ty_ref) = &param.ty else {
                    return Err(EffectFactsError::MalformedEffectOpSignature {
                        op_fqn: op_fqn.to_string(),
                        detail: "missing parameter type",
                    });
                };
                param_tys.push(
                    lower
                        .lower_type_ref_in_decl_file_with_scopes(
                            &op.symbol.decl_file,
                            signature_type_bindings.clone(),
                            std::iter::empty::<(String, EffectRow)>(),
                            param_ty_ref,
                        )
                        .map_err(|error| EffectFactsError::TypeLower(Box::new(error)))?,
                );
            }
            let return_ty = match &op.sig.return_ty {
                Some(return_ty_ref) => lower
                    .lower_type_ref_in_decl_file_with_scopes(
                        &op.symbol.decl_file,
                        signature_type_bindings,
                        std::iter::empty::<(String, EffectRow)>(),
                        return_ty_ref,
                    )
                    .map_err(|error| EffectFactsError::TypeLower(Box::new(error)))?,
                None => builtins.unit,
            };
            types.ty_function(receiver_ty, param_tys, return_ty, EffectRow::pure(), false)
        };
        let signature_key = canonical_callable_signature_key(
            types,
            signature_fun_ty,
            effect_sym.type_param_names.len(),
            op.sig.type_params.len(),
            0,
            &signature_resolver,
        )
        .map_err(|_| EffectFactsError::MalformedEffectOpSignature {
            op_fqn: op_fqn.to_string(),
            detail: "stable signature encoding failed",
        })?;
        let stable_instance_key = StableInstanceKey::from_type_arguments(
            StableTemplateKey::new(StableDefKey::new(
                self.stable_cone_key.clone(),
                StableDefNamespace::Fun,
                op_fqn,
                declaration_kind,
                Some(signature_key),
            )),
            types,
            &concrete_key_type_args,
            &[],
            &NoTypeParamResolver,
        )
        .map_err(|_| EffectFactsError::MalformedEffectOpSignature {
            op_fqn: op_fqn.to_string(),
            detail: "stable instance key encoding failed",
        })?;

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
                stable_instance_key,
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

fn collect_body_concrete_effect_ops_from_facts(
    type_ctx: &EffectFactsTypeContext,
    types: &mut TypeStore,
    body_facts: &MirBodyFactBundle<'_>,
) -> Result<Vec<ConcreteEffectOpContract>, EffectFactsError> {
    let mut contracts = Vec::new();
    for event in body_facts.events.values() {
        match &event.kind {
            mir_effects::MirEffectEventKind::Perform { op } => {
                contracts.push(type_ctx.concrete_effect_op_contract_for_site(
                    types,
                    op.effect_ty,
                    &op.op_fqn,
                    &op.op_type_args,
                )?);
            }
            mir_effects::MirEffectEventKind::Handle { arms, .. } => {
                for arm in arms {
                    contracts.push(type_ctx.concrete_effect_op_contract_for_site(
                        types,
                        arm.effect_ty,
                        &arm.op_fqn,
                        &arm.op_type_args,
                    )?);
                }
            }
            mir_effects::MirEffectEventKind::Call { .. }
            | mir_effects::MirEffectEventKind::ClassCtor { .. }
            | mir_effects::MirEffectEventKind::HiddenInitializer { .. }
            | mir_effects::MirEffectEventKind::Resume { .. } => {}
        }
    }

    contracts.sort_by_key(|contract| format!("{:?}", contract.concrete_op_key));
    let mut seen = HashSet::new();
    contracts.retain(|contract| seen.insert(contract.concrete_op_key.clone()));
    Ok(contracts)
}

fn collect_callable_seeds(
    materialized: &MaterializedMir,
    types: &mut TypeStore,
    type_ctx: &EffectFactsTypeContext,
    index: &Index,
    mir_fact_index: &MirFactIndex<'_>,
    compiler_continuation_runtime_error_callables: &HashSet<InstanceKey>,
    compiler_generated_runtime_error_effect_ty: TypeId,
) -> Result<Vec<CallableSeed>, EffectFactsError> {
    let pass_view = materialized.pass_view();
    let families = pass_view
        .instances()
        .map(|family| (family.key().clone(), family.root_body().cloned()))
        .collect::<Vec<_>>();
    let mut seeds = Vec::with_capacity(families.len());
    for (family_key, root_fun) in families {
        // effect-op 声明与 compiler-owned `Continuation.resume` surface contract 都会由更专门的
        // metadata/schema 路径承载；它们不应在 P4 被误当成“普通 callable body shell”参与求解。
        if template_decl_is_effect_op(index, &family_key.template)
            || template_decl_is_compiler_owned_resume(&family_key.template)
        {
            continue;
        }
        // canonical pass-view 允许保留“仍有实例身份，但当前没有 root body”的 family（例如
        // declaration-only instance，或某个 pass 已把 root body 从当前 snapshot 中移除）。
        // P4 facts 只能基于仍存在于当前 canonical snapshot 的 root body 建立 callable/body facts；
        // 对这类无 root body 的 family 直接跳过，而不是回 raw MIR 或报错要求补 fallback。
        let Some(root_fun) = root_fun else {
            continue;
        };
        let instance_effects = mir_fact_index.callable_instance(&family_key)?;
        let published_surface_row =
            effect_row_from_fact_template(types, &instance_effects.published_surface_row)?;
        let declared_row = instance_effects
            .declared_surface_row
            .as_ref()
            .map(|row| effect_row_from_fact_template(types, row))
            .transpose()?
            .unwrap_or_else(|| published_surface_row.clone());
        let surface_effect_row = published_surface_row;
        let step_effect_row = if compiler_continuation_runtime_error_callables.contains(&family_key)
        {
            let mut terms =
                effect_row_from_fact_template(types, &instance_effects.step_effect_row)?.terms;
            terms.push(compiler_generated_runtime_error_effect_ty);
            EffectRow::new(terms)
        } else {
            effect_row_from_fact_template(types, &instance_effects.step_effect_row)?
        };
        let body_concrete_effect_ops = collect_body_concrete_effect_ops_from_facts(
            type_ctx,
            types,
            mir_fact_index.body(&family_key, &root_fun.fqn)?,
        )?;
        let stable_instance_key_text = materialized
            .authoritative_stable_instance_key(&family_key)
            .ok_or_else(|| EffectFactsError::Frontend {
                message: format!(
                    "callable family `{}` 缺少 authoritative stable instance key，无法生成稳定 continuation 标识",
                    family_key.template.fqn,
                ),
            })?
            .canonical_text();
        let signature = mir_fact_index
            .source_signature(&root_fun.fqn)
            .ok_or_else(|| EffectFactsError::MissingMirFact {
                kind: "SourceCallableSignatureFact",
                detail: root_fun.fqn.clone(),
            })?;
        let (invoke_arg_components, complete_ty) =
            (signature.param_tys.clone(), signature.return_ty);
        seeds.push(CallableSeed {
            key: family_key,
            stable_instance_key_text,
            root_fun: root_fun.clone(),
            surface_effect_row,
            step_effect_row,
            declared_row,
            invoke_arg_components,
            complete_ty,
            body_concrete_effect_ops,
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

fn boundary_operand_ty(source: &mir_boundary::BoundaryOperandSource) -> Option<TypeId> {
    match source {
        mir_boundary::BoundaryOperandSource::Local { ty, .. }
        | mir_boundary::BoundaryOperandSource::Const { ty, .. } => *ty,
    }
}

fn call_site_kind_from_mir(kind: mir_effects::MirCallKind) -> CallSiteKind {
    match kind {
        mir_effects::MirCallKind::Direct => CallSiteKind::Direct,
        mir_effects::MirCallKind::Closure => CallSiteKind::Closure,
        mir_effects::MirCallKind::FunValue => CallSiteKind::FunValue,
        mir_effects::MirCallKind::FunPtr => CallSiteKind::FunPtr,
        mir_effects::MirCallKind::Virtual => CallSiteKind::Virtual,
        mir_effects::MirCallKind::Interface => CallSiteKind::Interface,
    }
}

fn effect_row_from_fact_template(
    types: &TypeStore,
    template: &mir_effects::EffectRowTemplate,
) -> Result<EffectRow, EffectFactsError> {
    let mut terms = Vec::with_capacity(template.terms.len());
    for term in &template.terms {
        match term {
            mir_effects::EffectRowTerm::Concrete { type_key } => {
                let ty = types
                    .iter_ids()
                    .find(|ty| {
                        canonical_type_text(types, *ty, &NoTypeParamResolver)
                            .is_ok_and(|text| text == type_key.as_str())
                    })
                    .ok_or_else(|| EffectFactsError::UnknownMirFactEffectTerm {
                        term: type_key.as_str().to_string(),
                    })?;
                terms.push(ty);
            }
            mir_effects::EffectRowTerm::Param {
                owner,
                ordinal,
                name,
            } => {
                return Err(EffectFactsError::UnsupportedMirFactEffectTerm {
                    term: format!("eff_param({},{},{})", owner.as_str(), ordinal, name),
                });
            }
        }
    }
    Ok(EffectRow::new(terms))
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

fn continuation_object_ty(types: &mut TypeStore, stable_instance_text: &str) -> TypeId {
    types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
        fqn: format!("scoop.__compiler.ContinuationObject@{stable_instance_text}"),
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeSet, HashMap};
    use std::path::PathBuf;

    use super::{EffectOwnedTypeContext, MaterializedEffectFactsBuilder, continuation_object_ty};
    use crate::effect_facts::{
        CallSiteKind, CallSiteTarget, CallTargetMode, CallableAbiKind, CanonicalMirQuerySurface,
        EffectPrecision, ImplPlan, NestedHandleClassification, SiteEffectFacts,
    };
    use crate::mir::{
        BasicBlockId, CallKind, InstanceKey, Rvalue, SiteId, StatementKind, TemplateKey,
        TerminatorKind, materialize_for_dump,
    };
    use crate::session::{Session, SessionOptions};
    use crate::source::SourceFile;
    use crate::span::Span;
    use crate::ty::{
        EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind,
    };

    fn session() -> Session {
        Session::with_options(SessionOptions::new()).unwrap()
    }

    fn frontend_artifact(
        session: &Session,
        source: &SourceFile,
    ) -> scoopc_hir::stage::HirSemanticArtifact {
        scoopc_hir::stage::run(session, source)
            .unwrap()
            .hir_semantic_artifact()
            .expect("HIR stage 应发布 effect-facts 可消费的 semantic artifact")
            .clone()
    }

    fn build_test_mir_facts(
        materialized: &crate::mir::MaterializedMir,
    ) -> scoopc_mir_facts::MirFacts {
        use scoopc_ids::{
            AbiMangler, BodyBlockId, BodyVersionKey, CanonicalTextKey, StableCanonicalKey as _,
            StableLirCallableKey, StageArtifactKey,
        };
        use scoopc_mir_facts::backend::SourceCallableSignatureFact;
        use scoopc_mir_facts::boundary::{
            BoundaryAnchor, BoundaryOperandSource, BoundarySourceContract, MirBoundaryFacts,
        };
        use scoopc_mir_facts::common::{FactIdentity, MirBodyReference};
        use scoopc_mir_facts::effects::{
            CallSiteSurfaceEffectFact, CallSiteTarget as FactCallSiteTarget, CallSiteTargetFact,
            CallableInstanceEffectFacts, EffectRowTemplate as FactEffectRowTemplate,
            EffectRowTerm as FactEffectRowTerm, MirBlockEffectRegionFact, MirCallKind,
            MirEffectEventFact, MirEffectEventKind, MirEffectFacts, MirEffectOpSiteContract,
            MirSiteInventoryFact, MirSiteKind,
        };

        let cone = materialized.stable_cone_key().clone();
        let mut facts = scoopc_mir_facts::MirFacts::new();
        facts.backend.source_signatures = materialized
            .source_callable_signatures()
            .iter()
            .enumerate()
            .map(|(index, signature)| {
                let target_key = test_lir_callable_key(&signature.fqn);
                SourceCallableSignatureFact {
                    identity: test_identity(
                        &cone,
                        format!("test:source_signature:{index}:{}", signature.fqn),
                    ),
                    fqn: signature.fqn.clone(),
                    target_callable_key: Some(target_key.clone()),
                    abi_symbol: Some(AbiMangler.fun_symbol(&target_key)),
                    abi_role: Some("callable_export".to_string()),
                    param_names: signature.param_names.clone(),
                    param_tys: signature.param_tys.clone(),
                    return_ty: signature.return_ty,
                }
            })
            .collect();
        let mut seen_source_signatures = facts
            .backend
            .source_signatures
            .iter()
            .map(|signature| signature.fqn.clone())
            .collect::<BTreeSet<_>>();

        let pass_view = materialized.pass_view();
        let mut effects = MirEffectFacts::default();
        let mut boundary = MirBoundaryFacts::default();

        for family in pass_view.instances() {
            let instance_artifact = test_instance_artifact(materialized, family.key());
            let root_body = family.root_body();
            if let Some(root) = root_body
                && seen_source_signatures.insert(root.fqn.clone())
            {
                facts.backend.source_signatures.push({
                    let target_key = test_lir_callable_key(&root.fqn);
                    SourceCallableSignatureFact {
                        identity: test_identity(
                            &cone,
                            format!("test:source_signature:root:{}", root.fqn),
                        ),
                        fqn: root.fqn.clone(),
                        target_callable_key: Some(target_key.clone()),
                        abi_symbol: Some(AbiMangler.fun_symbol(&target_key)),
                        abi_role: Some("callable_export".to_string()),
                        param_names: root.params.iter().map(|param| param.name.clone()).collect(),
                        param_tys: root.params.iter().map(|param| param.ty).collect(),
                        return_ty: root.return_ty,
                    }
                });
            }
            let published = root_body
                .and_then(|fun| test_function_effect_row(&materialized.types, fun.ty))
                .map(|(row, closed)| test_effect_row_template(&materialized.types, row, closed))
                .unwrap_or_else(FactEffectRowTemplate::pure);
            let mut step_rows = vec![published.clone()];
            for fun in family.callable_bodies() {
                if let Some(body) = &fun.body {
                    step_rows.extend(test_body_local_effect_rows(&materialized.types, body));
                }
            }
            effects
                .callable_instances
                .push(CallableInstanceEffectFacts::new(
                    test_identity(
                        &cone,
                        format!(
                            "test:callable_instance:{}",
                            instance_artifact.canonical_text()
                        ),
                    ),
                    instance_artifact.clone(),
                    CanonicalTextKey::new(family.root_fqn()),
                    Some(published.clone()),
                    published.clone(),
                    published.clone(),
                    test_merge_effect_rows(step_rows),
                ));

            for fun in family.callable_bodies() {
                let Some(body) = &fun.body else {
                    continue;
                };
                let body_ref = test_body_reference(&instance_artifact, fun);
                for (block_index, block) in body.blocks.iter().enumerate() {
                    let block_id = BodyBlockId::from_raw(block_index as u32);
                    let mut block_sites = Vec::new();
                    let mut has_suspend_boundary = false;
                    let mut has_handle_boundary = false;

                    for (statement_index, stmt) in block.stmts.iter().enumerate() {
                        let StatementKind::Assign { target, value } = &stmt.kind else {
                            continue;
                        };
                        match value {
                            Rvalue::Call {
                                site_id,
                                kind,
                                args,
                                ..
                            } => {
                                let site_kind = if matches!(kind, CallKind::Resume { .. }) {
                                    MirSiteKind::Resume
                                } else {
                                    MirSiteKind::Call
                                };
                                block_sites.push(*site_id);
                                effects.site_inventory.push(test_site_inventory(
                                    &cone,
                                    &instance_artifact,
                                    &body_ref,
                                    *site_id,
                                    site_kind,
                                    block_id,
                                    Some(statement_index as u32),
                                    Some(target.as_u32()),
                                    body.locals
                                        .get(target.as_u32() as usize)
                                        .map(|local| local.ty),
                                    block.is_cleanup,
                                ));
                                let surface = test_call_site_surface_row(
                                    &materialized.types,
                                    body,
                                    kind,
                                    &pass_view,
                                )
                                .unwrap_or_else(FactEffectRowTemplate::pure);
                                effects
                                    .call_site_surface_effects
                                    .push(CallSiteSurfaceEffectFact {
                                        identity: test_identity(
                                            &cone,
                                            format!(
                                                "test:call_surface:{}:{}",
                                                fun.fqn,
                                                site_id.as_u32()
                                            ),
                                        ),
                                        instance: instance_artifact.clone(),
                                        body: body_ref.clone(),
                                        site_id: *site_id,
                                        surface_row: surface.clone(),
                                    });
                                if let Some((call_kind, target)) =
                                    test_call_site_target(materialized, kind, args.len())
                                {
                                    effects.call_site_targets.push(CallSiteTargetFact {
                                        identity: test_identity(
                                            &cone,
                                            format!(
                                                "test:call_target:{}:{}",
                                                fun.fqn,
                                                site_id.as_u32()
                                            ),
                                        ),
                                        instance: instance_artifact.clone(),
                                        body: body_ref.clone(),
                                        site_id: *site_id,
                                        call_kind,
                                        target,
                                    });
                                }
                                let (event_kind, row) = match kind {
                                    CallKind::Resume { resume, .. } => {
                                        has_suspend_boundary |= !resume.out_effects.is_pure();
                                        (
                                            MirEffectEventKind::Resume {
                                                resume_tuple_ty: resume.resume_ty,
                                                answer_ty: resume.answer_ty,
                                                continuation_ty: resume.continuation_ty,
                                                surface_row: test_effect_row_template(
                                                    &materialized.types,
                                                    &resume.out_effects,
                                                    false,
                                                ),
                                            },
                                            test_resume_effect_row(&materialized.types, resume),
                                        )
                                    }
                                    _ => (test_call_event_kind(kind), surface),
                                };
                                effects.effect_events.push(test_event(
                                    &cone,
                                    &instance_artifact,
                                    &body_ref,
                                    *site_id,
                                    event_kind,
                                    block_id,
                                    Some(statement_index as u32),
                                    row,
                                    block.is_cleanup,
                                ));
                                boundary.source_contracts.push(test_boundary_contract(
                                    &cone,
                                    &instance_artifact,
                                    &body_ref,
                                    *site_id,
                                    site_kind,
                                    BoundaryAnchor::Statement {
                                        block: block_id,
                                        statement_index: statement_index as u32,
                                    },
                                    Some(target.as_u32()),
                                    test_call_carrier_source(&materialized.types, body, kind),
                                    args.iter()
                                        .map(|arg| {
                                            test_operand_source(
                                                &materialized.types,
                                                body,
                                                &arg.value,
                                            )
                                        })
                                        .collect(),
                                    None,
                                ));
                            }
                            Rvalue::ClassCtor {
                                site_id,
                                class_fqn,
                                hidden_effects,
                                args,
                                ..
                            } => {
                                block_sites.push(*site_id);
                                effects.site_inventory.push(test_site_inventory(
                                    &cone,
                                    &instance_artifact,
                                    &body_ref,
                                    *site_id,
                                    MirSiteKind::ClassCtor,
                                    block_id,
                                    Some(statement_index as u32),
                                    Some(target.as_u32()),
                                    body.locals
                                        .get(target.as_u32() as usize)
                                        .map(|local| local.ty),
                                    block.is_cleanup,
                                ));
                                let row = test_effect_row_template(
                                    &materialized.types,
                                    hidden_effects,
                                    false,
                                );
                                has_suspend_boundary |= !row.terms.is_empty();
                                effects.effect_events.push(test_event(
                                    &cone,
                                    &instance_artifact,
                                    &body_ref,
                                    *site_id,
                                    MirEffectEventKind::ClassCtor {
                                        source_fqn: class_fqn.clone(),
                                    },
                                    block_id,
                                    Some(statement_index as u32),
                                    row,
                                    block.is_cleanup,
                                ));
                                boundary.source_contracts.push(test_boundary_contract(
                                    &cone,
                                    &instance_artifact,
                                    &body_ref,
                                    *site_id,
                                    MirSiteKind::ClassCtor,
                                    BoundaryAnchor::Statement {
                                        block: block_id,
                                        statement_index: statement_index as u32,
                                    },
                                    Some(target.as_u32()),
                                    None,
                                    args.iter()
                                        .map(|arg| {
                                            test_operand_source(
                                                &materialized.types,
                                                body,
                                                &arg.value,
                                            )
                                        })
                                        .collect(),
                                    None,
                                ));
                            }
                            Rvalue::TopLevelRef(top_level)
                                if top_level.site_id.is_some()
                                    && !top_level.hidden_effects.is_pure() =>
                            {
                                let site_id = top_level.site_id.expect("checked above");
                                block_sites.push(site_id);
                                effects.site_inventory.push(test_site_inventory(
                                    &cone,
                                    &instance_artifact,
                                    &body_ref,
                                    site_id,
                                    MirSiteKind::HiddenInitializer,
                                    block_id,
                                    Some(statement_index as u32),
                                    Some(target.as_u32()),
                                    body.locals
                                        .get(target.as_u32() as usize)
                                        .map(|local| local.ty),
                                    block.is_cleanup,
                                ));
                                let row = test_effect_row_template(
                                    &materialized.types,
                                    &top_level.hidden_effects,
                                    false,
                                );
                                has_suspend_boundary |= !row.terms.is_empty();
                                effects.effect_events.push(test_event(
                                    &cone,
                                    &instance_artifact,
                                    &body_ref,
                                    site_id,
                                    MirEffectEventKind::HiddenInitializer {
                                        source_fqn: top_level.fqn.clone(),
                                    },
                                    block_id,
                                    Some(statement_index as u32),
                                    row,
                                    block.is_cleanup,
                                ));
                            }
                            Rvalue::MemberAccess {
                                site_id: Some(site_id),
                                member,
                                receiver,
                            } if !member.hidden_effects.is_pure() => {
                                block_sites.push(*site_id);
                                effects.site_inventory.push(test_site_inventory(
                                    &cone,
                                    &instance_artifact,
                                    &body_ref,
                                    *site_id,
                                    MirSiteKind::HiddenInitializer,
                                    block_id,
                                    Some(statement_index as u32),
                                    Some(target.as_u32()),
                                    body.locals
                                        .get(target.as_u32() as usize)
                                        .map(|local| local.ty),
                                    block.is_cleanup,
                                ));
                                let row = test_effect_row_template(
                                    &materialized.types,
                                    &member.hidden_effects,
                                    false,
                                );
                                has_suspend_boundary |= !row.terms.is_empty();
                                effects.effect_events.push(test_event(
                                    &cone,
                                    &instance_artifact,
                                    &body_ref,
                                    *site_id,
                                    MirEffectEventKind::HiddenInitializer {
                                        source_fqn: member.name.clone(),
                                    },
                                    block_id,
                                    Some(statement_index as u32),
                                    row,
                                    block.is_cleanup,
                                ));
                                boundary.source_contracts.push(test_boundary_contract(
                                    &cone,
                                    &instance_artifact,
                                    &body_ref,
                                    *site_id,
                                    MirSiteKind::HiddenInitializer,
                                    BoundaryAnchor::Statement {
                                        block: block_id,
                                        statement_index: statement_index as u32,
                                    },
                                    Some(target.as_u32()),
                                    Some(test_operand_source(&materialized.types, body, receiver)),
                                    Vec::new(),
                                    None,
                                ));
                            }
                            _ => {}
                        }
                    }

                    match &block.terminator.kind {
                        TerminatorKind::Perform {
                            site_id,
                            op_fqn,
                            metadata,
                            args,
                            ..
                        } => {
                            block_sites.push(*site_id);
                            effects.site_inventory.push(test_site_inventory(
                                &cone,
                                &instance_artifact,
                                &body_ref,
                                *site_id,
                                MirSiteKind::Perform,
                                block_id,
                                None,
                                None,
                                None,
                                block.is_cleanup,
                            ));
                            has_suspend_boundary = true;
                            effects.effect_events.push(test_event(
                                &cone,
                                &instance_artifact,
                                &body_ref,
                                *site_id,
                                MirEffectEventKind::Perform {
                                    op: test_perform_contract(
                                        &materialized.types,
                                        op_fqn,
                                        metadata,
                                    ),
                                },
                                block_id,
                                None,
                                test_effect_row_template(
                                    &materialized.types,
                                    &EffectRow::new(vec![metadata.effect_ty]),
                                    false,
                                ),
                                block.is_cleanup,
                            ));
                            boundary.source_contracts.push(test_boundary_contract(
                                &cone,
                                &instance_artifact,
                                &body_ref,
                                *site_id,
                                MirSiteKind::Perform,
                                BoundaryAnchor::Terminator { block: block_id },
                                None,
                                None,
                                args.iter()
                                    .map(|arg| {
                                        test_operand_source(&materialized.types, body, &arg.value)
                                    })
                                    .collect(),
                                None,
                            ));
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
                            block_sites.push(*site_id);
                            effects.site_inventory.push(test_site_inventory(
                                &cone,
                                &instance_artifact,
                                &body_ref,
                                *site_id,
                                MirSiteKind::Handle,
                                block_id,
                                None,
                                None,
                                None,
                                block.is_cleanup,
                            ));
                            has_handle_boundary = true;
                            effects.effect_events.push(test_event(
                                &cone,
                                &instance_artifact,
                                &body_ref,
                                *site_id,
                                MirEffectEventKind::Handle {
                                    result_ty: metadata.result_ty,
                                    body_target: BodyBlockId::from_raw(body_target.as_u32()),
                                    arm_targets: arm_targets
                                        .iter()
                                        .map(|target| BodyBlockId::from_raw(target.as_u32()))
                                        .collect(),
                                    finally_target: finally_target
                                        .map(|target| BodyBlockId::from_raw(target.as_u32())),
                                    exit_target: BodyBlockId::from_raw(exit_target.as_u32()),
                                    arms: arms
                                        .iter()
                                        .map(|arm| test_handler_contract(&materialized.types, arm))
                                        .collect(),
                                },
                                block_id,
                                None,
                                test_effect_row_template(
                                    &materialized.types,
                                    &EffectRow::new(
                                        arms.iter().map(|arm| arm.handled_effect_ty).collect(),
                                    ),
                                    false,
                                ),
                                block.is_cleanup,
                            ));
                        }
                        _ => {}
                    }

                    let mut successors = Vec::new();
                    block.terminator.for_each_successor(|successor| {
                        successors.push(BodyBlockId::from_raw(successor.as_u32()));
                    });
                    effects.block_regions.push(MirBlockEffectRegionFact {
                        identity: test_identity(
                            &cone,
                            format!("test:block:{}:bb{}", fun.fqn, block_index),
                        ),
                        instance: instance_artifact.clone(),
                        body: body_ref.clone(),
                        block: block_id,
                        site_ids: block_sites,
                        successors,
                        cleanup: block.is_cleanup,
                        cleanup_target: None,
                        has_suspend_boundary,
                        has_handle_boundary,
                    });
                }
            }
        }

        facts.effects = effects;
        facts.boundary = boundary;

        fn test_identity(cone: &crate::stable_id::StableConeKey, key: String) -> FactIdentity {
            FactIdentity::new(CanonicalTextKey::new(key.clone()), key, cone.clone(), None)
        }

        fn test_lir_callable_key(fqn: &str) -> StableLirCallableKey {
            StableLirCallableKey::new(format!("test_lir_callable({fqn})"), fqn.to_string())
        }

        fn test_instance_artifact(
            materialized: &crate::mir::MaterializedMir,
            instance: &InstanceKey,
        ) -> StageArtifactKey {
            let stable = materialized
                .authoritative_stable_instance_key(instance)
                .expect("test materialized instance should have stable key");
            StageArtifactKey::new("mir", &stable, "materialized_instance", 0)
        }

        fn test_body_reference(
            owner: &StageArtifactKey,
            fun: &crate::mir::FunDecl,
        ) -> MirBodyReference {
            let owner_key = CanonicalTextKey::new(owner.canonical_text());
            MirBodyReference::new(
                BodyVersionKey::new(&owner_key, "canonical_materialized_mir", 0),
                owner_key,
                fun.fqn.clone(),
                Some(fun.return_ty),
            )
        }

        fn test_effect_row_template(
            types: &TypeStore,
            row: &EffectRow,
            closed: bool,
        ) -> FactEffectRowTemplate {
            let stable = crate::stable_id::EffectRowTemplate::from_concrete_effect_row(
                types,
                row,
                &crate::stable_id::NoTypeParamResolver,
                closed,
            )
            .expect("test effect row should be stable");
            FactEffectRowTemplate::new(
                stable
                    .terms()
                    .iter()
                    .map(|term| match term {
                        crate::stable_id::EffectTerm::Concrete { type_key } => {
                            FactEffectRowTerm::Concrete {
                                type_key: type_key.clone(),
                            }
                        }
                        crate::stable_id::EffectTerm::Param {
                            owner,
                            ordinal,
                            name,
                        } => FactEffectRowTerm::Param {
                            owner: CanonicalTextKey::new(owner.canonical_text()),
                            ordinal: *ordinal,
                            name: name.clone(),
                        },
                    })
                    .collect(),
                stable.closed(),
            )
        }

        fn test_merge_effect_rows(rows: Vec<FactEffectRowTemplate>) -> FactEffectRowTemplate {
            let mut terms = Vec::new();
            let mut closed = true;
            for row in rows {
                closed &= row.closed;
                terms.extend(row.terms);
            }
            FactEffectRowTemplate::new(terms, closed)
        }

        fn test_function_effect_row(types: &TypeStore, ty: TypeId) -> Option<(&EffectRow, bool)> {
            let TypeKind::Ref(RefTypeKind::Function(fun)) = types.kind(ty) else {
                return None;
            };
            Some((&fun.effects, fun.effects_closed))
        }

        fn test_body_local_effect_rows(
            types: &TypeStore,
            body: &crate::mir::Body,
        ) -> Vec<FactEffectRowTemplate> {
            let mut rows = Vec::new();
            for block in &body.blocks {
                for stmt in &block.stmts {
                    let StatementKind::Assign { value, .. } = &stmt.kind else {
                        continue;
                    };
                    match value {
                        Rvalue::ClassCtor { hidden_effects, .. } => {
                            rows.push(test_effect_row_template(types, hidden_effects, false))
                        }
                        Rvalue::TopLevelRef(top_level) if !top_level.hidden_effects.is_pure() => {
                            rows.push(test_effect_row_template(
                                types,
                                &top_level.hidden_effects,
                                false,
                            ))
                        }
                        Rvalue::MemberAccess { member, .. } if !member.hidden_effects.is_pure() => {
                            rows.push(test_effect_row_template(
                                types,
                                &member.hidden_effects,
                                false,
                            ))
                        }
                        Rvalue::Call {
                            kind: CallKind::Resume { resume, .. },
                            ..
                        } => rows.push(test_resume_effect_row(types, resume)),
                        _ => {}
                    }
                }
                match &block.terminator.kind {
                    TerminatorKind::Perform { metadata, .. } => {
                        rows.push(test_effect_row_template(
                            types,
                            &EffectRow::new(vec![metadata.effect_ty]),
                            false,
                        ))
                    }
                    TerminatorKind::Handle { arms, .. } => rows.push(test_effect_row_template(
                        types,
                        &EffectRow::new(arms.iter().map(|arm| arm.handled_effect_ty).collect()),
                        false,
                    )),
                    _ => {}
                }
            }
            rows
        }

        fn test_resume_effect_row(
            types: &TypeStore,
            resume: &crate::mir::ResumeMetadata,
        ) -> FactEffectRowTemplate {
            let mut terms = resume.out_effects.terms.clone();
            if let Some(runtime_error) = resume.runtime_error_effect_ty {
                terms.push(runtime_error);
            }
            test_effect_row_template(types, &EffectRow::new(terms), false)
        }

        fn test_call_site_surface_row(
            types: &TypeStore,
            body: &crate::mir::Body,
            kind: &CallKind,
            pass_view: &crate::mir::MaterializedMirPassView<'_>,
        ) -> Option<FactEffectRowTemplate> {
            match kind {
                CallKind::Direct { callee_fqn, .. } => pass_view
                    .callable(callee_fqn)
                    .and_then(|fun| test_function_effect_row(types, fun.ty))
                    .map(|(row, closed)| test_effect_row_template(types, row, closed)),
                CallKind::Closure { callee, .. }
                | CallKind::FunValue { callee }
                | CallKind::FunPtr { callee } => {
                    test_operand_function_effect_row(types, body, callee)
                }
                CallKind::Virtual { dispatch, .. } | CallKind::Interface { dispatch, .. } => {
                    pass_view
                        .callable(&dispatch.member_fqn)
                        .and_then(|fun| test_function_effect_row(types, fun.ty))
                        .map(|(row, closed)| test_effect_row_template(types, row, closed))
                }
                CallKind::Resume { resume, .. } => Some(test_resume_effect_row(types, resume)),
            }
        }

        fn test_operand_function_effect_row(
            types: &TypeStore,
            body: &crate::mir::Body,
            operand: &crate::mir::Operand,
        ) -> Option<FactEffectRowTemplate> {
            let crate::mir::Operand::Local(local) = operand else {
                return None;
            };
            let ty = body.locals.get(local.as_u32() as usize)?.ty;
            test_function_effect_row(types, ty)
                .map(|(row, closed)| test_effect_row_template(types, row, closed))
        }

        fn test_call_event_kind(kind: &CallKind) -> MirEffectEventKind {
            let call_kind = match kind {
                CallKind::Direct { .. } => MirCallKind::Direct,
                CallKind::Closure { .. } => MirCallKind::Closure,
                CallKind::FunValue { .. } => MirCallKind::FunValue,
                CallKind::FunPtr { .. } => MirCallKind::FunPtr,
                CallKind::Virtual { .. } => MirCallKind::Virtual,
                CallKind::Interface { .. } => MirCallKind::Interface,
                CallKind::Resume { .. } => unreachable!("resume has dedicated event kind"),
            };
            MirEffectEventKind::Call { call_kind }
        }

        fn test_call_site_target(
            materialized: &crate::mir::MaterializedMir,
            kind: &CallKind,
            explicit_arg_count: usize,
        ) -> Option<(MirCallKind, FactCallSiteTarget)> {
            match kind {
                CallKind::Direct {
                    callee_fqn,
                    stable_instance_key,
                    ..
                } => Some((
                    MirCallKind::Direct,
                    stable_instance_key
                        .as_ref()
                        .map(|key| FactCallSiteTarget::KnownInstance {
                            key: CanonicalTextKey::new(key.canonical_text()),
                        })
                        .or_else(|| {
                            test_stable_key_for_callable(materialized, callee_fqn).map(|key| {
                                FactCallSiteTarget::KnownInstance {
                                    key: CanonicalTextKey::new(key.canonical_text()),
                                }
                            })
                        })
                        .unwrap_or_else(|| FactCallSiteTarget::DirectFunction {
                            fqn: callee_fqn.clone(),
                        }),
                )),
                CallKind::Closure { fn_ptr, .. } => Some((
                    MirCallKind::Closure,
                    FactCallSiteTarget::KnownClosure {
                        fn_ptr: fn_ptr.clone(),
                    },
                )),
                CallKind::FunValue { .. } => {
                    Some((MirCallKind::FunValue, FactCallSiteTarget::Dynamic))
                }
                CallKind::FunPtr { .. } => Some((MirCallKind::FunPtr, FactCallSiteTarget::Dynamic)),
                CallKind::Virtual { dispatch, .. } => Some((
                    MirCallKind::Virtual,
                    FactCallSiteTarget::CandidateSet {
                        keys: test_dispatch_candidate_keys(
                            materialized,
                            dispatch,
                            explicit_arg_count,
                            false,
                        ),
                    },
                )),
                CallKind::Interface { dispatch, .. } => Some((
                    MirCallKind::Interface,
                    FactCallSiteTarget::CandidateSet {
                        keys: test_dispatch_candidate_keys(
                            materialized,
                            dispatch,
                            explicit_arg_count,
                            true,
                        ),
                    },
                )),
                CallKind::Resume { .. } => None,
            }
        }

        fn test_stable_key_for_callable(
            materialized: &crate::mir::MaterializedMir,
            fqn: &str,
        ) -> Option<crate::stable_id::StableInstanceKey> {
            let owner = materialized.pass_view().owner_of_callable(fqn)?;
            materialized.authoritative_stable_instance_key(owner)
        }

        fn test_dispatch_candidate_keys(
            materialized: &crate::mir::MaterializedMir,
            dispatch: &crate::mir::DispatchMetadata,
            explicit_arg_count: usize,
            is_interface: bool,
        ) -> Vec<CanonicalTextKey> {
            let keys = dispatch
                .stable_candidate_keys
                .iter()
                .map(|key| CanonicalTextKey::new(key.canonical_text()))
                .collect::<Vec<_>>();
            if !keys.is_empty() {
                return keys;
            }
            let fqns = if is_interface {
                test_interface_candidate_fqns(materialized, dispatch, explicit_arg_count)
            } else {
                test_virtual_candidate_fqns(materialized, dispatch, explicit_arg_count)
            };
            let mut keys = fqns
                .iter()
                .map(|fqn| {
                    test_stable_key_for_callable(materialized, fqn).unwrap_or_else(|| {
                        panic!("test dispatch candidate `{fqn}` lacks a published stable key")
                    })
                })
                .map(|key| CanonicalTextKey::new(key.canonical_text()))
                .collect::<Vec<_>>();
            keys.sort_by(|left, right| left.as_str().cmp(right.as_str()));
            keys.dedup();
            keys
        }

        fn test_virtual_candidate_fqns(
            materialized: &crate::mir::MaterializedMir,
            dispatch: &crate::mir::DispatchMetadata,
            explicit_arg_count: usize,
        ) -> Vec<String> {
            let Some(receiver_fqn) =
                test_nominal_type_fqn(&materialized.types, dispatch.receiver_ty)
            else {
                return Vec::new();
            };
            let mut out = BTreeSet::new();
            for (class_fqn, slots) in &materialized.backend_contracts().class_vtables {
                if class_fqn != receiver_fqn
                    && !test_class_is_subclass(materialized, class_fqn, receiver_fqn)
                {
                    continue;
                }
                if let Some(slot) = slots.iter().find(|slot| {
                    slot.name == dispatch.member_name
                        && slot.params_len == explicit_arg_count as u32
                }) {
                    out.insert(slot.impl_member_fqn.clone());
                }
            }
            out.into_iter().collect()
        }

        fn test_interface_candidate_fqns(
            materialized: &crate::mir::MaterializedMir,
            dispatch: &crate::mir::DispatchMetadata,
            explicit_arg_count: usize,
        ) -> Vec<String> {
            let Some(interface) = materialized
                .backend_contracts()
                .interfaces
                .get(&dispatch.owner_fqn)
            else {
                return Vec::new();
            };
            let Some(slot) = interface.method_slots.iter().find(|slot| {
                slot.name == dispatch.member_name && slot.params_len == explicit_arg_count as u32
            }) else {
                return Vec::new();
            };
            let mut out = BTreeSet::new();
            for entries in materialized.backend_contracts().class_itables.values() {
                for entry in entries {
                    if entry.interface_fqn != dispatch.owner_fqn {
                        continue;
                    }
                    if let Some(target) = entry.method_impl_fqns.get(slot.slot as usize) {
                        out.insert(target.clone());
                    }
                }
            }
            out.into_iter().collect()
        }

        fn test_class_is_subclass(
            materialized: &crate::mir::MaterializedMir,
            class_fqn: &str,
            ancestor: &str,
        ) -> bool {
            let mut current = Some(class_fqn);
            while let Some(fqn) = current {
                if fqn == ancestor {
                    return true;
                }
                current = materialized
                    .backend_contracts()
                    .class_inits
                    .values()
                    .find(|init| init.fqn == fqn)
                    .and_then(|init| init.super_class_fqn.as_deref());
            }
            false
        }

        fn test_nominal_type_fqn(types: &TypeStore, ty: TypeId) -> Option<&str> {
            match types.kind(ty) {
                TypeKind::Ref(RefTypeKind::Nominal(nominal))
                | TypeKind::Value(ValueTypeKind::Nominal(nominal)) => Some(nominal.fqn.as_str()),
                _ => None,
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn test_site_inventory(
            cone: &crate::stable_id::StableConeKey,
            instance: &StageArtifactKey,
            body: &MirBodyReference,
            site_id: SiteId,
            kind: MirSiteKind,
            block: BodyBlockId,
            statement_index: Option<u32>,
            result_local: Option<u32>,
            result_ty: Option<TypeId>,
            cleanup: bool,
        ) -> MirSiteInventoryFact {
            MirSiteInventoryFact {
                identity: test_identity(
                    cone,
                    format!("test:site:{}:{}", body.fqn, site_id.as_u32()),
                ),
                instance: instance.clone(),
                body: body.clone(),
                site_id,
                kind,
                block,
                statement_index,
                result_local,
                result_ty,
                span: None,
                cleanup,
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn test_event(
            cone: &crate::stable_id::StableConeKey,
            instance: &StageArtifactKey,
            body: &MirBodyReference,
            site_id: SiteId,
            kind: MirEffectEventKind,
            block: BodyBlockId,
            statement_index: Option<u32>,
            effect_row: FactEffectRowTemplate,
            cleanup: bool,
        ) -> MirEffectEventFact {
            MirEffectEventFact {
                identity: test_identity(
                    cone,
                    format!("test:event:{}:{}", body.fqn, site_id.as_u32()),
                ),
                instance: instance.clone(),
                body: body.clone(),
                site_id,
                kind,
                block,
                statement_index,
                effect_row,
                cleanup,
            }
        }

        #[allow(clippy::too_many_arguments)]
        fn test_boundary_contract(
            cone: &crate::stable_id::StableConeKey,
            instance: &StageArtifactKey,
            body: &MirBodyReference,
            site_id: SiteId,
            kind: MirSiteKind,
            anchor: BoundaryAnchor,
            result_local: Option<u32>,
            carrier: Option<BoundaryOperandSource>,
            args: Vec<BoundaryOperandSource>,
            closure_env: Option<scoopc_mir_facts::boundary::ClosureEnvDecomposition>,
        ) -> BoundarySourceContract {
            BoundarySourceContract {
                identity: test_identity(
                    cone,
                    format!("test:boundary:{}:{}", body.fqn, site_id.as_u32()),
                ),
                instance: instance.clone(),
                body: body.clone(),
                site_id,
                kind,
                anchor,
                result_local,
                carrier,
                args,
                closure_env,
            }
        }

        fn test_perform_contract(
            types: &TypeStore,
            op_fqn: &str,
            metadata: &crate::mir::PerformMetadata,
        ) -> MirEffectOpSiteContract {
            MirEffectOpSiteContract {
                op_fqn: op_fqn.to_string(),
                effect_ty: metadata.effect_ty,
                op_type_args: metadata.op_type_args.clone(),
                payload_tuple_ty: test_tuple_carrier_ty(
                    types,
                    metadata.payload_tuple_ty,
                    &metadata.payload_component_tys,
                ),
            }
        }

        fn test_handler_contract(
            types: &TypeStore,
            arm: &crate::mir::HandlerArm,
        ) -> MirEffectOpSiteContract {
            MirEffectOpSiteContract {
                op_fqn: arm.op_fqn.clone(),
                effect_ty: arm.handled_effect_ty,
                op_type_args: arm.op_type_args.clone(),
                payload_tuple_ty: test_tuple_carrier_ty(
                    types,
                    arm.payload_tuple_ty,
                    &arm.payload_component_tys,
                ),
            }
        }

        fn test_tuple_carrier_ty(
            types: &TypeStore,
            explicit: Option<TypeId>,
            components: &[TypeId],
        ) -> TypeId {
            if let Some(ty) = explicit {
                return ty;
            }
            match components {
                [] => types.builtins().expect("builtins").unit,
                [single] => *single,
                many => types
                    .iter_ids()
                    .find(|id| matches!(types.kind(*id), TypeKind::Value(ValueTypeKind::Tuple(elements)) if elements == many))
                    .expect("tuple carrier should exist"),
            }
        }

        fn test_call_carrier_source(
            types: &TypeStore,
            body: &crate::mir::Body,
            kind: &CallKind,
        ) -> Option<BoundaryOperandSource> {
            match kind {
                CallKind::Closure { callee, .. }
                | CallKind::FunValue { callee }
                | CallKind::FunPtr { callee } => Some(test_operand_source(types, body, callee)),
                CallKind::Virtual { receiver, .. } | CallKind::Interface { receiver, .. } => {
                    Some(test_operand_source(types, body, receiver))
                }
                CallKind::Resume { continuation, .. } => {
                    Some(test_operand_source(types, body, continuation))
                }
                CallKind::Direct { .. } => None,
            }
        }

        fn test_operand_source(
            types: &TypeStore,
            body: &crate::mir::Body,
            operand: &crate::mir::Operand,
        ) -> BoundaryOperandSource {
            match operand {
                crate::mir::Operand::Local(local) => BoundaryOperandSource::Local {
                    local: local.as_u32(),
                    ty: body.locals.get(local.as_u32() as usize).map(|decl| decl.ty),
                },
                crate::mir::Operand::Const(value) => BoundaryOperandSource::Const {
                    kind: format!("{value:?}"),
                    ty: test_const_ty(types, value),
                },
            }
        }

        fn test_const_ty(types: &TypeStore, value: &crate::mir::ConstValue) -> Option<TypeId> {
            let builtins = types.builtins()?;
            Some(match value {
                crate::mir::ConstValue::Bool(_) => builtins.bool_,
                crate::mir::ConstValue::Char => builtins.char_,
                crate::mir::ConstValue::Unit => builtins.unit,
                crate::mir::ConstValue::Int | crate::mir::ConstValue::SynthInt(_) => builtins.int,
                crate::mir::ConstValue::Float64 => builtins.float64,
                crate::mir::ConstValue::Float32 => builtins.float32,
                crate::mir::ConstValue::String | crate::mir::ConstValue::SynthString(_) => {
                    builtins.string
                }
            })
        }

        facts
    }

    fn sample_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_builder_fixture.scoop",
            r#"
package sample

import scoop.core.*

effect Flag {
    fun ping(): Unit
}

fun <T> pureUnit(_witness: T): Unit {}

fun <T> raiseString(_witness: T): Unit / Raise<String> {
    Raise.raise("boom")
}

fun <T> raiseInt(_witness: T): Unit / Raise<Int> {
    Raise.raise(1)
}

fun <T> pingFlag(_witness: T): Unit / Flag {
    Flag.ping()
}

fun <T> resumeZero(_witness: T, k: Continuation<Unit, Unit, eff Pure>): Unit / Raise<RuntimeError> {
    k.resume()
}

fun exercise(k: Continuation<Unit, Unit, eff Pure>): Unit / (Flag + Raise<String> + Raise<Int> + Raise<RuntimeError>) {
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
        let session = session();
        let source = sample_source();
        let frontend_artifact = frontend_artifact(&session, &source);
        let materialized = materialize_for_dump(&session, &source).unwrap();
        let mir_facts = build_test_mir_facts(&materialized);
        let mut type_context = EffectOwnedTypeContext::from_mir_types(&materialized.types);
        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &frontend_artifact,
            &materialized,
            &mir_facts,
            &mut type_context,
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
        let session = session();
        let frontend_artifact = frontend_artifact(&session, &source);
        let materialized = materialize_for_dump(&session, &source).unwrap();
        let mir_facts = build_test_mir_facts(&materialized);
        let mut type_context = EffectOwnedTypeContext::from_mir_types(&materialized.types);
        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &frontend_artifact,
            &materialized,
            &mir_facts,
            &mut type_context,
        )
        .build()
        .unwrap();
        (materialized, facts)
    }

    #[test]
    fn gc_handle_intrinsic_call_sites_use_handle_token_carrier() {
        let source = SourceFile::new_virtual(
            "<mem>/effect_facts_gc_handle_carrier.scoop",
            r#"
package sample

import scoop.core.*

class Box(val value: Int)

fun main(): Unit {
    @Unsafe do {
        val box: Any = Box(value = 1)
        val h: GcHandle = GC.handleNew(box)
        val got: Any = GC.handleGet(h)
        GC.handleDrop(h)
    }
}
"#,
        );
        let (_, facts) = build_facts_for_source(source);
        let (main_key, _) = callable_facts_for(&facts, "sample.main");
        let body = facts
            .body(main_key)
            .expect("sample.main body facts should be published");

        let handle_carriers = body
            .sites()
            .values()
            .filter_map(|site| {
                let SiteEffectFacts::Call(call) = site else {
                    return None;
                };
                let carrier = facts
                    .types()
                    .display(call.invoke_args_tuple_ty())
                    .to_string();
                (carrier == "scoop.core.GcHandle").then_some(carrier)
            })
            .collect::<Vec<_>>();

        assert_eq!(
            handle_carriers,
            vec![
                "scoop.core.GcHandle".to_string(),
                "scoop.core.GcHandle".to_string(),
            ],
            "GC handle token calls must publish the stable handle carrier, not the GC singleton receiver",
        );
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

    fn funptr_source() -> SourceFile {
        SourceFile::new_virtual(
            "<mem>/effect_facts_funptr_sites.scoop",
            r#"
package sample

import scoop.core.*
import scoop.unsafe.*

@Extern("native_get_funptr")
fun getFunPtr(): FunPtr<(Int) -> Int>

fun use(): Int {
    val fp: FunPtr<(Int) -> Int> = @Unsafe do { getFunPtr() }
    return @Unsafe do { fp(41) }
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
    } on {
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
        } on {
            Inner.go() -> 1
        }
        inner + 10
    } on {
        Outer.again() -> 99
    }
}

fun nested_may_suspend_outward(): Int {
    return handle {
        val inner: Int = handle {
            Inner.go()
            0
        } on {
            Inner.go() -> 1
        } finally {
            Outer.again()
        }
        inner + 10
    } on {
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
        facts: &crate::effect_facts::MaterializedEffectFacts,
        schema_id: crate::effect_facts::ContinuationSchemaId,
    ) -> String {
        let schema = facts
            .continuation_schemas()
            .get(&schema_id)
            .expect("continuation schema 应存在");
        facts.types().display(schema.surface_ty()).to_string()
    }

    fn continuation_surface_tys_for_step_schema(
        facts: &crate::effect_facts::MaterializedEffectFacts,
        step_schema: crate::effect_facts::StepSchemaId,
    ) -> BTreeSet<String> {
        facts
            .step_schemas()
            .get(&step_schema)
            .expect("step schema 应存在")
            .cases()
            .iter()
            .map(|case| continuation_surface_ty_string(facts, case.continuation_schema()))
            .collect()
    }

    #[test]
    fn site_effect_facts_capture_call_target_modes_and_resume_contracts() {
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
                    CallKind::Closure { .. }
                    | CallKind::FunValue { .. }
                    | CallKind::FunPtr { .. } => {}
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
        assert_eq!(fun_value_facts.precision(), EffectPrecision::Widened);
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
            continuation_surface_ty_string(&facts, resume_facts.continuation_schema()),
            "scoop.core.Continuation<Int, Int, eff sample.Boom>"
        );
        assert_eq!(
            continuation_surface_tys_for_step_schema(&facts, resume_facts.out_step_schema()),
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
    fn funptr_call_sites_stay_plain_native_dynamic_fallbacks() {
        let (materialized, facts) = build_facts_for_source(funptr_source());
        let (key, _) = callable_facts_for(&facts, "sample.use");
        let pass_view = materialized.pass_view();
        let body = pass_view
            .instance(key)
            .and_then(|family| family.root_body())
            .and_then(|fun| fun.body.as_ref())
            .expect("use 应有 canonical body");
        let body_facts = facts.body(key).expect("use 应有 body facts");

        let funptr_site_id = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| {
                let StatementKind::Assign { value, .. } = &stmt.kind else {
                    return None;
                };
                let Rvalue::Call {
                    site_id,
                    kind: CallKind::FunPtr { .. },
                    ..
                } = value
                else {
                    return None;
                };
                Some(*site_id)
            })
            .expect("use 应包含显式 FunPtr call site");

        let SiteEffectFacts::Call(funptr_facts) = body_facts
            .site(funptr_site_id)
            .expect("FunPtr site 应可通过 SiteId 查询")
        else {
            panic!("FunPtr site 应产生 CallSiteEffectFacts");
        };

        assert_eq!(funptr_facts.kind(), CallSiteKind::FunPtr);
        assert_eq!(funptr_facts.callee_abi_kind(), CallableAbiKind::Plain);
        assert_eq!(funptr_facts.target_mode(), CallTargetMode::DynamicFallback);
        assert!(matches!(
            funptr_facts.target(),
            CallSiteTarget::DynamicFallback
        ));
        assert_eq!(funptr_facts.precision(), EffectPrecision::Precise);
        assert!(funptr_facts.callee_step_schema().is_none());
        assert!(funptr_facts.resolved_cases().is_empty());
    }

    #[test]
    fn site_effect_facts_capture_perform_and_handle_contracts() {
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
    fn body_effect_facts_index_blocks_and_sites_by_stable_ids() {
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
    fn nested_handle_classification_distinguishes_self_contained_and_finally_outward() {
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
                | SiteEffectFacts::ClassCtor(_)
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
                | SiteEffectFacts::ClassCtor(_)
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
        let session = session();
        let source = sample_source();
        let frontend_artifact = frontend_artifact(&session, &source);
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
        let mir_facts = build_test_mir_facts(&materialized);
        let mut type_context = EffectOwnedTypeContext::from_mir_types(&materialized.types);

        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &frontend_artifact,
            &materialized,
            &mir_facts,
            &mut type_context,
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
    fn callable_effect_facts_shell_skips_effect_op_roots() {
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
    fn effect_schema_case_tags_are_stable_and_distinguish_generic_specialized_raise_cases() {
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
    fn continuation_schema_explicitly_records_unit_payload_resume_and_surface_type() {
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
            facts
                .types()
                .display(continuation_schema.surface_ty())
                .to_string(),
            "scoop.core.Continuation<Unit, Unit, eff sample.Flag>"
        );
        assert!(
            facts
                .types()
                .display(schema.continuation_obj_ty())
                .to_string()
                .contains("sample.pingFlag")
        );
    }

    #[test]
    fn callable_effect_facts_shell_uses_final_shape_and_runtime_error_case() {
        let (_, facts) = build_sample_facts();

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
            facts
                .types()
                .display(runtime_case.payload_tuple_ty())
                .to_string(),
            "scoop.core.RuntimeError"
        );
        assert_eq!(
            continuation_surface_tys_for_step_schema(&facts, resume_zero_facts.step_schema()),
            ["scoop.core.Continuation<Nothing, Unit, eff scoop.core.Raise<scoop.core.RuntimeError>>".to_string()]
                .into_iter()
                .collect(),
            "若源码 residual row 本来就包含 Raise<RuntimeError>，surface_ty 必须继续如实保留它"
        );
    }

    #[test]
    fn effect_schema_compiler_continuation_runtime_error_adds_runtime_error_case_to_step_schema() {
        let session = session();
        let source = compiler_continuation_runtime_error_source();
        let frontend_artifact = frontend_artifact(&session, &source);
        let materialized = materialize_for_dump(&session, &source).unwrap();
        let mir_facts = build_test_mir_facts(&materialized);
        let mut type_context = EffectOwnedTypeContext::from_mir_types(&materialized.types);
        let leaf_key = materialized
            .pass_view()
            .owner_of_callable("sample.leaf")
            .expect("leaf 应有 canonical owner")
            .clone();

        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &frontend_artifact,
            &materialized,
            &mir_facts,
            &mut type_context,
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
    fn continuation_schema_surface_ty_preserves_residual_out_row_for_compiler_runtime_error_upper_bound()
     {
        let session = session();
        let source = compiler_continuation_runtime_error_source();
        let frontend_artifact = frontend_artifact(&session, &source);
        let materialized = materialize_for_dump(&session, &source).unwrap();
        let mir_facts = build_test_mir_facts(&materialized);
        let mut type_context = EffectOwnedTypeContext::from_mir_types(&materialized.types);
        let leaf_key = materialized
            .pass_view()
            .owner_of_callable("sample.leaf")
            .expect("leaf 应有 canonical owner")
            .clone();

        let facts = MaterializedEffectFactsBuilder::from_materialized_snapshot(
            &frontend_artifact,
            &materialized,
            &mir_facts,
            &mut type_context,
        )
        .with_compiler_continuation_runtime_error_callables([leaf_key.clone()])
        .build()
        .unwrap();

        let leaf_facts = facts
            .callable_facts()
            .get(&leaf_key)
            .expect("leaf 应存在于 callable facts");
        assert_eq!(
            continuation_surface_tys_for_step_schema(&facts, leaf_facts.step_schema()),
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
    fn callable_effect_facts_shell_compiler_continuation_runtime_error_only_expands_selected_callables()
     {
        let session = session();
        let source = compiler_continuation_runtime_error_source();
        let frontend_artifact = frontend_artifact(&session, &source);
        let materialized = materialize_for_dump(&session, &source).unwrap();
        let mir_facts = build_test_mir_facts(&materialized);
        let mut type_context = EffectOwnedTypeContext::from_mir_types(&materialized.types);
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
            &frontend_artifact,
            &materialized,
            &mir_facts,
            &mut type_context,
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
    fn continuation_schema_surface_ty_preserves_pure_resume_surface_row() {
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
            continuation_surface_ty_string(&facts, resume_facts.continuation_schema()),
            "scoop.core.Continuation<Unit, Unit, eff Pure>"
        );
        assert_eq!(
            continuation_surface_tys_for_step_schema(&facts, resume_facts.out_step_schema()),
            ["scoop.core.Continuation<Nothing, Unit, eff Pure>".to_string()]
                .into_iter()
                .collect(),
            "resume synthetic step upper bound 即使额外带 runtime-error case，也不能把 Pure residual row 扩大回 surface_ty"
        );
    }

    #[test]
    fn callable_effect_facts_shell_instance_keys_distinguish_allowed_rows() {
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
    fn continuation_schema_identity_distinguishes_callable_instances() {
        let mut types = TypeStore::new();
        let _ = types.intern_builtins();

        let string_cont_ty = continuation_object_ty(&mut types, "instance::sample.forward<String>");
        let int_cont_ty = continuation_object_ty(&mut types, "instance::sample.forward<Int>");

        assert_ne!(string_cont_ty, int_cont_ty);
        assert_ne!(
            types.display(string_cont_ty).to_string(),
            types.display(int_cont_ty).to_string()
        );
    }
}
