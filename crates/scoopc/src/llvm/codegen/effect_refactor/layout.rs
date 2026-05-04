use std::collections::{BTreeMap, BTreeSet, HashMap};

use inkwell::module::Linkage;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum, StructType};

use crate::effect_facts::{
    CallSiteEffectFacts, CallSiteKind, CallTargetMode, ContinuationSchemaId,
    MaterializedEffectFacts, SiteEffectFacts, StepSchemaId,
};
use crate::effect_lowered::LateLoweredProgram;
use crate::effect_lowered::ir::{
    BoundaryId, BoundarySiteKind, ContinuationObjectId, LateLoweredBodyVersionKey,
    LateLoweredBoundaryLowering, LateLoweredBoundarySource, LateLoweredBoundarySourceConsumption,
    LateLoweredCallBoundaryContinuationComposition, LateLoweredCallable,
    LateLoweredCompletionPayloadBinding, LateLoweredCompletionPayloadSource,
    LateLoweredContinuationMethodReachability, LateLoweredContinuationObject,
    LateLoweredFrameSlotKind, LateLoweredHandlePendingCompletion,
    LateLoweredLocalRuntimeErrorTerminalAction, LateLoweredOperandSource,
    LateLoweredOperandValueSource, LateLoweredPublishedRuntimeEntry, LateLoweredResumeInterface,
    LateLoweredResumePayloadBinding, LateLoweredSourceStatementClassificationKind,
    LateLoweredStateSlice, LateLoweredStateTerminator, LateLoweredStepType,
    LateLoweredSurfaceResumeDispatchInventoryEntry, LateLoweredSurfaceResumeDispatchPublication,
    LateLoweredSurfaceResumeWrapperCaseProjection,
    LateLoweredSurfaceResumeWrapperCompletePayloadSource,
    LateLoweredSurfaceResumeWrapperCompleteProjection, LateLoweredSurfaceResumeWrapperProjection,
    ResumeInterfaceId, StateId, SystemSlotKind,
};
use crate::llvm::LlvmEmitError;
use crate::mir::{
    BasicBlockId, CallKind as MirCallKind, HandlerArm as MirHandlerArm, InstanceKey,
    Rvalue as MirRvalue, SiteId, StatementKind as MirStatementKind,
    TerminatorKind as MirTerminatorKind,
};
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::super::types::IntTy;
use super::super::{MainCodegen, RefactorCallableCarrierKind, sanitize_llvm_ident};
use super::types::{
    RefactorAbiQuery, RefactorAbiValue, RefactorCallBoundaryOperandLayout,
    RefactorCallableCarrierTargetLayout, RefactorCallableEntryLayout, RefactorCallableLayout,
    RefactorClosureCarrierLayout, RefactorCompletionPayloadBindingLayout,
    RefactorContinuationFieldKind, RefactorContinuationFieldLayout,
    RefactorContinuationObjectLayout, RefactorContinuationSurfaceResumeBinding,
    RefactorContinuationSurfaceResumeDispatchLayout,
    RefactorContinuationSurfaceResumeDispatchTarget,
    RefactorContinuationSurfaceResumeHandleBinderRoute, RefactorContinuationSurfaceResumeLayout,
    RefactorContinuationSurfaceResumeMethodLookup,
    RefactorContinuationSurfaceResumeOwnerTrampolineLayout, RefactorDispatchReceiverLayout,
    RefactorDynamicInvokeCarrierLayout, RefactorDynamicInvokeLayout, RefactorFrameFieldKind,
    RefactorFrameFieldLayout, RefactorFrameLayout, RefactorHandleArmLayout,
    RefactorHandleContinuationBinderLayout, RefactorHandleDispatchLayout,
    RefactorHandlePayloadBinderLayout, RefactorHandlePendingPayloadTransportLayout,
    RefactorLocalRuntimeErrorContract, RefactorLocalRuntimeErrorTerminalAction,
    RefactorPerformBoundaryOperandLayout, RefactorPublishedRuntimeEntryLayout,
    RefactorResumeBoundaryOperandLayout, RefactorResumeInterfaceLayout, RefactorResumeMethodLayout,
    RefactorResumePayloadBindingLayout, RefactorSourceAbiFieldLayout, RefactorSourceAbiLayout,
    RefactorSourceAbiLayoutKind, RefactorStepCaseLayout, RefactorStepLayout,
    RefactorStepVariantLayout,
};

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// P6-T02：把 P5 late-lowered contract 显式物化成 LLVM type/layout 查询面。
    pub(crate) fn materialize_refactor_program_abi(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a crate::mir::MaterializedMirPassView<'a>,
        effect_facts: &'a MaterializedEffectFacts,
    ) -> Result<RefactorAbiQuery<'ctx>, LlvmEmitError> {
        RefactorAbiMaterializer::new(self, program, source_types, pass_view, effect_facts)?
            .materialize()
    }
}

struct ProgramLayoutView {
    callable_stems_by_step_schema: BTreeMap<StepSchemaId, String>,
}

type BoundaryOperandKey = (StepSchemaId, crate::mir::SiteId);
type CallBoundaryOperandLayouts = BTreeMap<BoundaryOperandKey, RefactorCallBoundaryOperandLayout>;
type PerformBoundaryOperandLayouts =
    BTreeMap<BoundaryOperandKey, RefactorPerformBoundaryOperandLayout>;
type ResumeBoundaryOperandLayouts =
    BTreeMap<BoundaryOperandKey, RefactorResumeBoundaryOperandLayout>;
type ResumePayloadBindingBoundaryKey = (StepSchemaId, BoundaryId);
type ResumePayloadBindingStateKey = (StepSchemaId, StateId);
type ResumePayloadBindingLayouts =
    BTreeMap<ResumePayloadBindingBoundaryKey, RefactorResumePayloadBindingLayout>;
type ResumePayloadBindingLayoutsByState =
    BTreeMap<ResumePayloadBindingStateKey, RefactorResumePayloadBindingLayout>;
type CompletionPayloadBindingKey = (StepSchemaId, StateId);
type CompletionPayloadBindingLayouts<'ctx> =
    BTreeMap<CompletionPayloadBindingKey, RefactorCompletionPayloadBindingLayout<'ctx>>;
type BoundaryOperandLayoutSets = (
    CallBoundaryOperandLayouts,
    PerformBoundaryOperandLayouts,
    ResumeBoundaryOperandLayouts,
);

struct MaterializedDynamicCallSite {
    kind: MirCallKind,
    arg_count: usize,
}

impl ProgramLayoutView {
    fn new(program: &LateLoweredProgram) -> Result<Self, LlvmEmitError> {
        let mut step_types_by_schema = BTreeMap::new();
        for step_type in program.step_types() {
            if step_types_by_schema
                .insert(step_type.step_schema(), step_type)
                .is_some()
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 遇到重复 StepSchemaId {}",
                    step_type.step_schema().as_u32()
                )));
            }
        }

        let mut root_counts = HashMap::new();
        for callable in program.callables() {
            *root_counts.entry(callable.root_fqn()).or_insert(0usize) += 1;
        }

        let mut callables_by_step_schema = BTreeMap::new();
        let mut callable_stems_by_step_schema = BTreeMap::new();
        for callable in program.callables() {
            if callables_by_step_schema
                .insert(callable.step_schema(), callable)
                .is_some()
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 遇到重复 callable step schema {}（callable={})",
                    callable.step_schema().as_u32(),
                    callable.root_fqn()
                )));
            }

            let base = sanitize_llvm_ident(callable.root_fqn());
            let stem = if root_counts.get(callable.root_fqn()).copied().unwrap_or(0) > 1 {
                format!("{base}__schema{}", callable.step_schema().as_u32())
            } else {
                base
            };
            callable_stems_by_step_schema.insert(callable.step_schema(), stem);
        }

        for step_schema in step_types_by_schema.keys().copied() {
            callable_stems_by_step_schema
                .entry(step_schema)
                .or_insert_with(|| format!("schema{}", step_schema.as_u32()));
        }

        let mut continuation_objects_by_id = BTreeMap::new();
        for object in program.continuation_objects() {
            if continuation_objects_by_id
                .insert(object.object_id(), object)
                .is_some()
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 遇到重复 continuation object {}",
                    object.object_id().as_u32()
                )));
            }
        }

        let mut resume_packings_by_id = BTreeMap::new();
        for interface in program.resume_packings() {
            if resume_packings_by_id
                .insert(interface.interface_id(), interface)
                .is_some()
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 遇到重复 resume packing {}",
                    interface.interface_id().as_u32()
                )));
            }
        }

        Ok(Self {
            callable_stems_by_step_schema,
        })
    }

    fn step_stem(&self, step_schema: StepSchemaId) -> &str {
        self.callable_stems_by_step_schema
            .get(&step_schema)
            .map(String::as_str)
            .unwrap_or("schema")
    }
}

struct RefactorAbiMaterializer<'cg, 'a, 'ctx> {
    codegen: &'cg mut MainCodegen<'a, 'ctx>,
    program: &'a LateLoweredProgram,
    source_types: &'a TypeStore,
    pass_view: &'a crate::mir::MaterializedMirPassView<'a>,
    effect_facts: &'a MaterializedEffectFacts,
    view: ProgramLayoutView,
    source_value_layouts: BTreeMap<TypeId, RefactorSourceAbiLayout<'ctx>>,
}

impl<'cg, 'a, 'ctx> RefactorAbiMaterializer<'cg, 'a, 'ctx> {
    fn new(
        codegen: &'cg mut MainCodegen<'a, 'ctx>,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a crate::mir::MaterializedMirPassView<'a>,
        effect_facts: &'a MaterializedEffectFacts,
    ) -> Result<Self, LlvmEmitError> {
        Ok(Self {
            codegen,
            program,
            source_types,
            pass_view,
            effect_facts,
            view: ProgramLayoutView::new(program)?,
            source_value_layouts: BTreeMap::new(),
        })
    }

    fn materialize(self) -> Result<RefactorAbiQuery<'ctx>, LlvmEmitError> {
        let mut this = self;
        let mut step_layouts = BTreeMap::new();
        for step_type in this.program.step_types() {
            step_layouts.insert(
                step_type.step_schema(),
                this.materialize_step_layout(step_type)?,
            );
        }

        let mut resume_packing_layouts = BTreeMap::new();
        for interface in this.program.resume_packings() {
            resume_packing_layouts.insert(
                interface.interface_id(),
                this.materialize_resume_packing_layout(interface, &step_layouts)?,
            );
        }

        let surface_resume_layouts = this.materialize_surface_resume_layouts(&step_layouts)?;
        this.validate_resume_site_surface_contracts(&surface_resume_layouts)?;

        let mut frame_layouts = BTreeMap::new();
        for callable in this.program.callables() {
            frame_layouts.insert(
                callable.step_schema(),
                this.materialize_frame_layout(callable)?,
            );
        }

        let mut continuation_layouts = BTreeMap::new();
        for object in this.program.continuation_objects() {
            continuation_layouts.insert(
                object.object_id(),
                this.materialize_continuation_object_layout(object, &surface_resume_layouts)?,
            );
        }

        let mut callable_layouts = BTreeMap::new();
        for callable in this.program.callables() {
            callable_layouts.insert(
                callable.step_schema(),
                this.materialize_callable_layout(callable, &step_layouts)?,
            );
        }
        let callable_layouts_by_version_key =
            this.materialize_callable_version_layout_index(&callable_layouts)?;
        let known_instance_callable_versions =
            this.materialize_known_instance_callable_versions(&callable_layouts)?;

        let surface_resume_dispatch_layouts = this.materialize_surface_resume_dispatch_layouts(
            &surface_resume_layouts,
            &continuation_layouts,
            &resume_packing_layouts,
            &callable_layouts,
            &frame_layouts,
        )?;

        let callable_carrier_target_layouts =
            this.publish_callable_carrier_entry_shells(&callable_layouts, &step_layouts)?;

        let dynamic_invoke_layouts = this.materialize_dynamic_invoke_layouts(&step_layouts)?;
        let (
            call_boundary_operand_layouts,
            perform_boundary_operand_layouts,
            resume_boundary_operand_layouts,
        ) = this.materialize_boundary_operand_layouts(
            &dynamic_invoke_layouts,
            &surface_resume_layouts,
            &surface_resume_dispatch_layouts,
        )?;
        let (resume_payload_binding_layouts, resume_payload_bindings_by_state) =
            this.materialize_resume_payload_binding_layouts(&frame_layouts)?;
        let completion_payload_binding_layouts =
            this.materialize_completion_payload_binding_layouts(&step_layouts, &frame_layouts)?;
        let local_runtime_error_contracts = this.materialize_local_runtime_error_contracts()?;
        let handle_dispatch_layouts = this.materialize_handle_dispatch_layouts(
            &frame_layouts,
            &continuation_layouts,
            &surface_resume_layouts,
        )?;
        this.validate_source_statement_classifications()?;

        Ok(RefactorAbiQuery::new(
            this.source_value_layouts,
            step_layouts,
            frame_layouts,
            continuation_layouts,
            resume_packing_layouts,
            surface_resume_layouts,
            surface_resume_dispatch_layouts,
            callable_layouts,
            callable_layouts_by_version_key,
            known_instance_callable_versions,
            callable_carrier_target_layouts,
            dynamic_invoke_layouts,
            call_boundary_operand_layouts,
            perform_boundary_operand_layouts,
            resume_boundary_operand_layouts,
            resume_payload_binding_layouts,
            resume_payload_bindings_by_state,
            completion_payload_binding_layouts,
            local_runtime_error_contracts,
            handle_dispatch_layouts,
        ))
    }

    fn materialize_step_layout(
        &mut self,
        step_type: &LateLoweredStepType,
    ) -> Result<RefactorStepLayout<'ctx>, LlvmEmitError> {
        let stem = self.view.step_stem(step_type.step_schema()).to_string();
        let step_type_name = format!("scoop.refactor.Step__{stem}");
        let storage_type_name = format!("scoop.refactor.StepStorage__{stem}");
        let step_anchor_name = format!("__scoop_refactor_step_layout__{stem}");
        let complete_tag_name = format!("__scoop_refactor_step_case_tag__{stem}__complete");
        let complete_payload_name = format!("scoop.refactor.StepComplete__{stem}");
        let complete_payload_anchor =
            format!("__scoop_refactor_step_variant_payload__{stem}__complete");

        let complete_payload_layout = self.source_value_layout(step_type.complete_ty())?;
        let complete_payload_abi = *complete_payload_layout.abi();
        let complete_fields = if complete_payload_abi.is_elided() {
            Vec::new()
        } else {
            vec![complete_payload_abi.llvm_ty()]
        };
        let complete_payload_ty =
            self.define_named_struct(&complete_payload_name, &complete_fields);
        self.ensure_struct_anchor(&complete_payload_anchor, complete_payload_ty);
        self.ensure_case_tag_constant(&complete_tag_name, 0);

        let complete_variant = RefactorStepVariantLayout::new(
            0,
            step_type.complete_ty(),
            complete_payload_ty,
            usize::from(!complete_payload_abi.is_elided()),
            complete_payload_anchor,
            complete_payload_abi.is_elided(),
        );

        let mut case_layouts = BTreeMap::new();
        let mut payload_tys = vec![complete_payload_ty];
        for case in step_type.cases() {
            let case_payload_name = format!(
                "scoop.refactor.StepCase__{stem}__case{}",
                case.case_tag().as_u32()
            );
            let case_payload_anchor = format!(
                "__scoop_refactor_step_variant_payload__{stem}__case{}",
                case.case_tag().as_u32()
            );
            let case_tag_name = format!(
                "__scoop_refactor_step_case_tag__{stem}__case{}",
                case.case_tag().as_u32()
            );
            let payload_layout = self.source_value_layout(case.payload_tuple_ty())?;
            let payload_abi = *payload_layout.abi();
            let mut case_fields = Vec::new();
            if !payload_abi.is_elided() {
                case_fields.push(payload_abi.llvm_ty());
            }
            case_fields.push(self.codegen.llvm_gc_i8_ptr_type().into());
            let case_payload_ty = self.define_named_struct(&case_payload_name, &case_fields);
            self.ensure_struct_anchor(&case_payload_anchor, case_payload_ty);
            let tag_value = case.case_tag().as_u32().saturating_add(1);
            self.ensure_case_tag_constant(&case_tag_name, tag_value);

            payload_tys.push(case_payload_ty);
            case_layouts.insert(
                case.case_tag(),
                RefactorStepCaseLayout::new(
                    case.case_tag(),
                    case_tag_name,
                    RefactorStepVariantLayout::new(
                        tag_value,
                        case.payload_tuple_ty(),
                        case_payload_ty,
                        case_fields.len(),
                        case_payload_anchor,
                        payload_abi.is_elided(),
                    ),
                ),
            );
        }

        let storage_ty = self.define_union_storage_type(&storage_type_name, &payload_tys);
        let step_ty = self.define_named_struct(
            &step_type_name,
            &[self.codegen.context.i32_type().into(), storage_ty.into()],
        );
        self.ensure_struct_anchor(&step_anchor_name, step_ty);

        Ok(RefactorStepLayout::new(
            step_type.step_schema(),
            step_ty,
            step_anchor_name,
            complete_tag_name,
            complete_variant,
            case_layouts,
        ))
    }

    fn materialize_resume_packing_layout(
        &mut self,
        interface: &LateLoweredResumeInterface,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
    ) -> Result<RefactorResumeInterfaceLayout<'ctx>, LlvmEmitError> {
        let step_schema = interface.return_step_schema();
        let stem = self.view.step_stem(step_schema).to_string();
        let effect_stem = sanitize_llvm_ident(interface.effect_family().effect_fqn());
        let vtable_type_name = format!("scoop.refactor.ResumeVtable__{stem}__{effect_stem}");
        let vtable_anchor_name =
            format!("__scoop_refactor_resume_vtable_layout__{stem}__{effect_stem}");
        let return_step_ty = step_layouts
            .get(&step_schema)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 resume packing {} 的 return step schema {}",
                    interface.interface_id().as_u32(),
                    step_schema.as_u32()
                ))
            })?
            .llvm_ty();
        let step_type = self.program.step_type(step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 resume packing {} 的 step type {}",
                interface.interface_id().as_u32(),
                step_schema.as_u32()
            ))
        })?;
        let expected_case_tags = step_type
            .cases()
            .iter()
            .filter(|case| case.concrete_op_key().effect_family() == interface.effect_family())
            .map(|case| case.case_tag())
            .collect::<BTreeSet<_>>();

        let mut methods = BTreeMap::new();
        let mut vtable_fields = Vec::new();
        let mut published_case_tags = BTreeSet::new();
        for (index, method) in interface.methods().iter().enumerate() {
            let step_case = step_type.case(method.case_tag()).ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 resume packing {} method case {} 在 step schema {} 中不存在",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32(),
                    step_schema.as_u32()
                ))
            })?;
            if !published_case_tags.insert(method.case_tag()) {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 resume packing {} 重复发布 case {}",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32()
                )));
            }
            if method.concrete_op_key().effect_family() != interface.effect_family() {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 resume packing {} method case {} 的 effect family `{}` 与 packing family `{}` 不一致",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32(),
                    method.concrete_op_key().effect_family().effect_fqn(),
                    interface.effect_family().effect_fqn()
                )));
            }
            if step_case.concrete_op_key() != method.concrete_op_key() {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 resume packing {} method case {} 的 concrete op `{}` 与 step shell 发布 `{}` 不一致",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32(),
                    method.concrete_op_key().instance_key().template.fqn,
                    step_case.concrete_op_key().instance_key().template.fqn,
                )));
            }
            if step_case.concrete_op_key().effect_family() != interface.effect_family() {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 resume packing {} method case {} 指向的 step case family `{}` 与 packing family `{}` 不一致",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32(),
                    step_case.concrete_op_key().effect_family().effect_fqn(),
                    interface.effect_family().effect_fqn()
                )));
            }
            if method.out_step_schema() != step_schema {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 resume packing {} method case {} 的 out step schema {} 与 packing return step schema {} 不一致",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32(),
                    method.out_step_schema().as_u32(),
                    step_schema.as_u32()
                )));
            }
            if step_case.continuation_contract() != method.continuation_contract() {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 resume packing {} method case {} 的 continuation contract 与 step shell 不一致",
                    interface.interface_id().as_u32(),
                    method.case_tag().as_u32()
                )));
            }

            let symbol_name = format!(
                "__scoop_refactor_resume__{stem}__case{}",
                method.case_tag().as_u32()
            );
            let payload_layout = self.source_value_layout(method.resume_tuple_ty())?;
            let payload_abi = *payload_layout.abi();
            let _answer_layout = self.source_value_layout(method.answer_ty())?;
            let mut params: Vec<BasicMetadataTypeEnum<'ctx>> =
                vec![self.codegen.llvm_gc_i8_ptr_type().into()];
            if !payload_abi.is_elided() {
                params.push(payload_abi.llvm_ty().into());
            }
            let fn_ty = return_step_ty.fn_type(&params, false);
            self.ensure_declared_function(&symbol_name, fn_ty);
            vtable_fields.push(self.codegen.llvm_i8_ptr_type().into());
            methods.insert(
                method.case_tag(),
                RefactorResumeMethodLayout::new(
                    interface.interface_id(),
                    method.case_tag(),
                    symbol_name,
                    fn_ty,
                    params.len(),
                    index as u32,
                    payload_abi,
                    step_schema,
                ),
            );
        }
        let missing_case_tags = expected_case_tags
            .difference(&published_case_tags)
            .map(|case_tag| case_tag.as_u32().to_string())
            .collect::<Vec<_>>();
        if !missing_case_tags.is_empty() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 resume packing {} 在 step schema {} 的 effect family `{}` 下缺少 authoritative method cases [{}]",
                interface.interface_id().as_u32(),
                step_schema.as_u32(),
                interface.effect_family().effect_fqn(),
                missing_case_tags.join(", ")
            )));
        }

        let vtable_ty = self.define_named_struct(&vtable_type_name, &vtable_fields);
        self.ensure_struct_anchor(&vtable_anchor_name, vtable_ty);
        Ok(RefactorResumeInterfaceLayout::new(
            interface.interface_id(),
            interface.effect_family().effect_fqn().to_string(),
            vtable_ty,
            vtable_anchor_name,
            methods,
        ))
    }

    fn materialize_surface_resume_layouts(
        &mut self,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
    ) -> Result<
        BTreeMap<ContinuationSchemaId, RefactorContinuationSurfaceResumeLayout<'ctx>>,
        LlvmEmitError,
    > {
        let mut layouts = BTreeMap::new();
        for entry in self.program.surface_resume_dispatch_inventory() {
            let contract = entry.contract();
            self.register_surface_resume_layout(
                &mut layouts,
                entry.continuation_schema(),
                entry.source_kind(),
                contract.resume_tuple_ty(),
                contract.answer_ty(),
                contract.out_step_schema(),
                &format!(
                    "surface-resume dispatch inventory k{}",
                    entry.continuation_schema().as_u32()
                ),
                step_layouts,
            )?;
        }
        Ok(layouts)
    }

    #[allow(clippy::too_many_arguments)]
    fn register_surface_resume_layout(
        &mut self,
        layouts: &mut BTreeMap<ContinuationSchemaId, RefactorContinuationSurfaceResumeLayout<'ctx>>,
        continuation_schema: ContinuationSchemaId,
        dispatch_source_kind: crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind,
        resume_tuple_ty: crate::ty::TypeId,
        answer_ty: crate::ty::TypeId,
        return_step_schema: StepSchemaId,
        source_label: &str,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        let layout = self.materialize_surface_resume_layout(
            continuation_schema,
            dispatch_source_kind,
            resume_tuple_ty,
            answer_ty,
            return_step_schema,
            step_layouts,
        )?;
        match layouts.entry(continuation_schema) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(layout);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                let existing = entry.get();
                if existing.resume_tuple_ty() != layout.resume_tuple_ty()
                    || existing.answer_ty() != layout.answer_ty()
                    || existing.return_step_schema() != layout.return_step_schema()
                    || existing.param_count() != layout.param_count()
                    || existing.resume_payload_abi().is_elided()
                        != layout.resume_payload_abi().is_elided()
                {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 continuation schema k{} 的 surface-resume contract 漂移：已发布为 resume_tuple_ty={} answer_ty={} out_step_schema={}，但 {source_label} 重新发布为 resume_tuple_ty={} answer_ty={} out_step_schema={}",
                        continuation_schema.as_u32(),
                        existing.resume_tuple_ty().as_u32(),
                        existing.answer_ty().as_u32(),
                        existing.return_step_schema().as_u32(),
                        layout.resume_tuple_ty().as_u32(),
                        layout.answer_ty().as_u32(),
                        layout.return_step_schema().as_u32(),
                    )));
                }
            }
        }
        Ok(())
    }

    fn materialize_surface_resume_layout(
        &mut self,
        continuation_schema: ContinuationSchemaId,
        dispatch_source_kind: crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind,
        resume_tuple_ty: crate::ty::TypeId,
        answer_ty: crate::ty::TypeId,
        step_schema: StepSchemaId,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
    ) -> Result<RefactorContinuationSurfaceResumeLayout<'ctx>, LlvmEmitError> {
        let return_step_ty = step_layouts
            .get(&step_schema)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 continuation schema k{} 的 surface-resume return step schema {}",
                    continuation_schema.as_u32(),
                    step_schema.as_u32()
                ))
            })?
            .llvm_ty();
        let symbol_name = format!(
            "__scoop_refactor_surface_resume__k{}",
            continuation_schema.as_u32()
        );
        let payload_layout = self.source_value_layout(resume_tuple_ty)?;
        let payload_abi = *payload_layout.abi();
        let _answer_layout = self.source_value_layout(answer_ty)?;
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> =
            vec![self.codegen.llvm_gc_i8_ptr_type().into()];
        if !payload_abi.is_elided() {
            params.push(payload_abi.llvm_ty().into());
        }
        let fn_ty = return_step_ty.fn_type(&params, false);
        self.ensure_declared_function(&symbol_name, fn_ty);
        Ok(RefactorContinuationSurfaceResumeLayout::new(
            continuation_schema,
            dispatch_source_kind,
            symbol_name,
            fn_ty,
            params.len(),
            resume_tuple_ty,
            answer_ty,
            payload_abi,
            step_schema,
        ))
    }

    fn validate_resume_site_surface_contracts(
        &self,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        for callable in self.program.callables() {
            for boundary in callable.boundary_map().entries() {
                let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering()
                else {
                    continue;
                };
                let site_id = match boundary.source() {
                    LateLoweredBoundarySource::Site {
                        site_id,
                        kind: BoundarySiteKind::Resume,
                    } => site_id,
                    other => {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` 的 resume lowering 绑定到了非 Resume boundary source {other:?}",
                            callable.root_fqn(),
                        )));
                    }
                };
                let facts = lowering.facts();
                let layout = surface_resume_layouts.get(&facts.continuation_schema()).ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 缺少 callable `{}` resume site {} 所需的 continuation schema k{} surface-resume layout",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        facts.continuation_schema().as_u32(),
                    ))
                })?;
                if layout.resume_tuple_ty() != facts.resume_tuple_ty()
                    || layout.answer_ty() != facts.answer_ty()
                    || layout.return_step_schema() != facts.out_step_schema()
                {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` resume site {} 的 continuation schema k{} surface-resume contract 与 ResumeSiteEffectFacts 漂移：layout=(resume_tuple_ty={}, answer_ty={}, out_step_schema={})，facts=(resume_tuple_ty={}, answer_ty={}, out_step_schema={})",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        facts.continuation_schema().as_u32(),
                        layout.resume_tuple_ty().as_u32(),
                        layout.answer_ty().as_u32(),
                        layout.return_step_schema().as_u32(),
                        facts.resume_tuple_ty().as_u32(),
                        facts.answer_ty().as_u32(),
                        facts.out_step_schema().as_u32(),
                    )));
                }
                if matches!(
                    layout.dispatch_source_kind(),
                    crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
                        | crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::Unreachable
                ) {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` resume site {} 的 continuation schema k{} dispatch source kind 为 {:?}，无法作为 authoritative resume-site surface source",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        facts.continuation_schema().as_u32(),
                        layout.dispatch_source_kind(),
                    )));
                }
            }
        }
        Ok(())
    }

    fn materialize_frame_layout(
        &mut self,
        callable: &LateLoweredCallable,
    ) -> Result<RefactorFrameLayout<'ctx>, LlvmEmitError> {
        let stem = self.view.step_stem(callable.step_schema()).to_string();
        let frame_type_name = format!("scoop.refactor.Frame__{stem}");
        let frame_anchor_name = format!("__scoop_refactor_frame_layout__{stem}");
        let header_ty = self.codegen.llvm_gc_object_header_type();
        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = vec![header_ty.into()];
        let mut fields = vec![RefactorFrameFieldLayout::new(
            0,
            RefactorFrameFieldKind::Header,
            header_ty.into(),
        )];
        let mut slot_field_indices = BTreeMap::new();
        let mut system_field_indices = BTreeMap::new();

        for slot in callable.frame_schema().slots() {
            let field_index = llvm_fields.len() as u32;
            let slot_abi = self.abi_value(slot.ty())?;
            llvm_fields.push(slot_abi.llvm_ty());
            fields.push(RefactorFrameFieldLayout::new(
                field_index,
                RefactorFrameFieldKind::Slot(slot.slot_id()),
                slot_abi.llvm_ty(),
            ));
            slot_field_indices.insert(slot.slot_id(), field_index);
            if let LateLoweredFrameSlotKind::System(kind) = slot.kind() {
                system_field_indices.insert(kind, field_index);
            }
        }

        let frame_ty = self.define_named_struct(&frame_type_name, &llvm_fields);
        self.ensure_struct_anchor(&frame_anchor_name, frame_ty);
        Ok(RefactorFrameLayout::new(
            callable.step_schema(),
            frame_ty,
            frame_anchor_name,
            fields,
            slot_field_indices,
            system_field_indices,
        ))
    }

    fn materialize_continuation_object_layout(
        &mut self,
        object: &LateLoweredContinuationObject,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<RefactorContinuationObjectLayout<'ctx>, LlvmEmitError> {
        let owner_callable = self
            .program
            .callable_by_version_key(object.owner_version_key())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 continuation object {} 的 owner callable",
                    object.object_id().as_u32()
                ))
            })?;
        let stem = self
            .view
            .step_stem(owner_callable.step_schema())
            .to_string();
        let cont_type_name = format!("scoop.refactor.Continuation__{stem}");
        let cont_anchor_name = format!("__scoop_refactor_continuation_layout__{stem}");
        let header_ty = self.codegen.llvm_gc_object_header_type();
        let frame_ptr_ty = self.codegen.llvm_gc_i8_ptr_type();
        let resume_state_ty = self.codegen.context.i32_type();
        let one_shot_ty = self.codegen.context.bool_type();
        let composed_callee_ty = self.codegen.llvm_gc_i8_ptr_type();
        let vtable_ptr_ty = self.codegen.llvm_i8_ptr_type();
        self.validate_published_resume_packing_ids(
            &format!("continuation object {}", object.object_id().as_u32()),
            owner_callable.step_schema(),
            object.implemented_packings(),
        )?;
        let surface_resume_bindings = self.materialize_surface_resume_bindings(
            object,
            owner_callable,
            surface_resume_layouts,
        )?;
        if object.implemented_packings() != owner_callable.resume_packings() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 continuation object {} 的 implemented packings {} 与 owner callable `{}` 的 published resume packings {} 不一致",
                object.object_id().as_u32(),
                render_resume_packing_ids(object.implemented_packings()),
                owner_callable.root_fqn(),
                render_resume_packing_ids(owner_callable.resume_packings()),
            )));
        }
        let packing_ids = object.implemented_packings().to_vec();

        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = vec![
            header_ty.into(),
            frame_ptr_ty.into(),
            resume_state_ty.into(),
            one_shot_ty.into(),
            composed_callee_ty.into(),
        ];
        let mut fields = vec![
            RefactorContinuationFieldLayout::new(
                0,
                RefactorContinuationFieldKind::Header,
                header_ty.into(),
            ),
            RefactorContinuationFieldLayout::new(
                1,
                RefactorContinuationFieldKind::CapturedFrame,
                frame_ptr_ty.into(),
            ),
            RefactorContinuationFieldLayout::new(
                2,
                RefactorContinuationFieldKind::ResumeStateTag,
                resume_state_ty.into(),
            ),
            RefactorContinuationFieldLayout::new(
                3,
                RefactorContinuationFieldKind::OneShotFlag,
                one_shot_ty.into(),
            ),
            RefactorContinuationFieldLayout::new(
                4,
                RefactorContinuationFieldKind::ComposedCalleeContinuation,
                composed_callee_ty.into(),
            ),
        ];
        let mut packing_field_indices = BTreeMap::new();
        for interface_id in &packing_ids {
            let field_index = llvm_fields.len() as u32;
            llvm_fields.push(vtable_ptr_ty.into());
            fields.push(RefactorContinuationFieldLayout::new(
                field_index,
                RefactorContinuationFieldKind::PackingVtable(*interface_id),
                vtable_ptr_ty.into(),
            ));
            packing_field_indices.insert(*interface_id, field_index);
        }

        let cont_ty = self.define_named_struct(&cont_type_name, &llvm_fields);
        self.ensure_struct_anchor(&cont_anchor_name, cont_ty);
        Ok(RefactorContinuationObjectLayout::new(
            object.object_id(),
            owner_callable.step_schema(),
            cont_ty,
            cont_anchor_name,
            fields,
            packing_field_indices,
            surface_resume_bindings,
        ))
    }

    fn materialize_surface_resume_bindings(
        &self,
        object: &LateLoweredContinuationObject,
        owner_callable: &LateLoweredCallable,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<
        BTreeMap<ContinuationSchemaId, Vec<RefactorContinuationSurfaceResumeBinding>>,
        LlvmEmitError,
    > {
        let mut bindings =
            BTreeMap::<ContinuationSchemaId, Vec<RefactorContinuationSurfaceResumeBinding>>::new();
        let mut register_binding =
            |continuation_schema: ContinuationSchemaId,
             return_step_schema: StepSchemaId,
             case_tag: crate::effect_facts::CaseTag,
             reachability: crate::effect_lowered::ir::LateLoweredContinuationMethodReachability,
             source_label: &str|
             -> Result<(), LlvmEmitError> {
                let layout = surface_resume_layouts
                .get(&continuation_schema)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 缺少 continuation object {} 需要的 continuation schema k{} surface-resume layout（来源：{source_label}）",
                        object.object_id().as_u32(),
                        continuation_schema.as_u32(),
                    ))
                })?;
                if layout.return_step_schema() != return_step_schema {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 continuation object {} 的 continuation schema k{} 在 {source_label} 上声明 out_step_schema=s{}，但已发布 surface-resume layout 的 return step schema 为 s{}",
                        object.object_id().as_u32(),
                        continuation_schema.as_u32(),
                        return_step_schema.as_u32(),
                        layout.return_step_schema().as_u32(),
                    )));
                }
                bindings.entry(continuation_schema).or_default().push(
                    RefactorContinuationSurfaceResumeBinding::new(
                        continuation_schema,
                        return_step_schema,
                        case_tag,
                        reachability,
                    ),
                );
                Ok(())
            };

        for surface_resume in object.surface_resumes() {
            register_binding(
                surface_resume.continuation_schema(),
                surface_resume.out_step_schema(),
                surface_resume.case_tag(),
                surface_resume.reachability(),
                &format!(
                    "continuation object {} published surface resume case {}",
                    object.object_id().as_u32(),
                    surface_resume.case_tag().as_u32()
                ),
            )?;
        }

        let owner_step = self
            .program
            .step_type(owner_callable.step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 continuation object {} 的 owner step schema {}",
                    object.object_id().as_u32(),
                    owner_callable.step_schema().as_u32(),
                ))
            })?;
        for case in owner_step.cases() {
            if !bindings.contains_key(&case.continuation_schema()) {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 continuation object {} 缺少 owner step schema {} case {} 所需的 continuation schema k{} surface-resume 发布",
                    object.object_id().as_u32(),
                    owner_callable.step_schema().as_u32(),
                    case.case_tag().as_u32(),
                    case.continuation_schema().as_u32(),
                )));
            }
        }
        Ok(bindings)
    }

    fn materialize_callable_layout(
        &mut self,
        callable: &LateLoweredCallable,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
    ) -> Result<RefactorCallableLayout<'ctx>, LlvmEmitError> {
        let stem = self.view.step_stem(callable.step_schema()).to_string();
        let step_ty = step_layouts
            .get(&callable.step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{}` 的 step layout {}",
                    callable.root_fqn(),
                    callable.step_schema().as_u32()
                ))
            })?
            .llvm_ty();
        let args_layout =
            self.source_value_layout(callable.dynamic_invoke_entry().invoke_args_tuple_ty())?;
        let args_abi = *args_layout.abi();
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::new();
        if !args_abi.is_elided() {
            params.push(args_abi.llvm_ty().into());
        }
        let dynamic_ty = step_ty.fn_type(&params, false);
        let direct_ty = step_ty.fn_type(&params, false);
        let dynamic_name = format!("__scoop_refactor_dynamic_invoke__{stem}");
        let direct_name = format!("__scoop_refactor_direct_invoke__{stem}");
        self.ensure_declared_function(&dynamic_name, dynamic_ty);
        self.ensure_declared_function(&direct_name, direct_ty);
        self.validate_published_resume_packing_ids(
            &format!("callable `{}`", callable.root_fqn()),
            callable.step_schema(),
            callable.resume_packings(),
        )?;
        let continuation_object = self
            .program
            .continuation_object(callable.continuation_object())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{}` 的 continuation object {}",
                    callable.root_fqn(),
                    callable.continuation_object().as_u32()
                ))
            })?;
        if continuation_object.implemented_packings() != callable.resume_packings() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` 的 published resume packings {} 与 continuation object {} 的 implemented packings {} 不一致",
                callable.root_fqn(),
                render_resume_packing_ids(callable.resume_packings()),
                continuation_object.object_id().as_u32(),
                render_resume_packing_ids(continuation_object.implemented_packings()),
            )));
        }
        let resume_packings = callable.resume_packings().to_vec();

        Ok(RefactorCallableLayout::new(
            callable.root_fqn().to_string(),
            callable.body_version_key().clone(),
            callable.step_schema(),
            RefactorCallableEntryLayout::new(
                dynamic_name,
                dynamic_ty,
                params.len(),
                callable.dynamic_invoke_entry().invoke_args_tuple_ty(),
                args_abi,
                callable.step_schema(),
            ),
            RefactorCallableEntryLayout::new(
                direct_name,
                direct_ty,
                params.len(),
                callable.dynamic_invoke_entry().invoke_args_tuple_ty(),
                args_abi,
                callable.step_schema(),
            ),
            callable.continuation_object(),
            resume_packings,
        ))
    }

    fn materialize_callable_version_layout_index(
        &self,
        callable_layouts: &BTreeMap<StepSchemaId, RefactorCallableLayout<'ctx>>,
    ) -> Result<HashMap<LateLoweredBodyVersionKey, StepSchemaId>, LlvmEmitError> {
        let mut index = HashMap::with_capacity(callable_layouts.len());
        for layout in callable_layouts.values() {
            let version_key = layout.body_version_key().clone();
            if let Some(existing_step_schema) =
                index.insert(version_key.clone(), layout.step_schema())
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 body version key {:?} 同时指向 callable step schema s{} 与 s{}",
                    version_key,
                    existing_step_schema.as_u32(),
                    layout.step_schema().as_u32(),
                )));
            }
        }
        Ok(index)
    }

    fn materialize_known_instance_callable_versions(
        &self,
        callable_layouts: &BTreeMap<StepSchemaId, RefactorCallableLayout<'ctx>>,
    ) -> Result<HashMap<(InstanceKey, StepSchemaId), LateLoweredBodyVersionKey>, LlvmEmitError>
    {
        let mut selectors = HashMap::with_capacity(callable_layouts.len());
        for layout in callable_layouts.values() {
            let selector = (layout.surface_instance().clone(), layout.step_schema());
            let version_key = layout.body_version_key().clone();
            if let Some(existing) = selectors.insert(selector.clone(), version_key.clone()) {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 known-instance selector ({:?}, s{}) 同时指向多个 callable version：已有 {:?}，新值 {:?}",
                    selector.0,
                    selector.1.as_u32(),
                    existing,
                    version_key,
                )));
            }
        }
        Ok(selectors)
    }

    fn materialize_surface_resume_dispatch_layouts(
        &mut self,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeLayout<'ctx>,
        >,
        continuation_layouts: &BTreeMap<
            ContinuationObjectId,
            RefactorContinuationObjectLayout<'ctx>,
        >,
        resume_packing_layouts: &BTreeMap<ResumeInterfaceId, RefactorResumeInterfaceLayout<'ctx>>,
        callable_layouts: &BTreeMap<StepSchemaId, RefactorCallableLayout<'ctx>>,
        frame_layouts: &BTreeMap<StepSchemaId, RefactorFrameLayout<'ctx>>,
    ) -> Result<
        BTreeMap<ContinuationSchemaId, RefactorContinuationSurfaceResumeDispatchLayout<'ctx>>,
        LlvmEmitError,
    > {
        let mut layouts = BTreeMap::new();
        for entry in self.program.surface_resume_dispatch_inventory() {
            let continuation_schema = entry.continuation_schema();
            let surface_layout = surface_resume_layouts
                .get(&continuation_schema)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 缺少 continuation schema k{} 的 surface-resume layout，无法发布 owner dispatch contract",
                        continuation_schema.as_u32(),
                    ))
                })?;
            let method_targets = match entry.source_kind() {
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod => self
                    .materialize_surface_resume_method_targets(
                        entry,
                        surface_layout,
                        continuation_layouts,
                        resume_packing_layouts,
                    )?,
                _ => Vec::new(),
            };
            let target = match entry.source_kind() {
                crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::Unreachable => {
                    RefactorContinuationSurfaceResumeDispatchTarget::Unreachable
                }
                _ => RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(Box::new(
                    self.materialize_surface_resume_owner_trampoline_layout(
                        entry,
                        surface_layout,
                        callable_layouts,
                        frame_layouts,
                        &method_targets,
                    )?,
                )),
            };
            if layouts
                .insert(
                    continuation_schema,
                    RefactorContinuationSurfaceResumeDispatchLayout::new(
                        continuation_schema,
                        entry.source_kind(),
                        method_targets,
                        target,
                    ),
                )
                .is_some()
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 continuation schema k{} 的 surface-resume owner dispatch contract 重复发布",
                    continuation_schema.as_u32(),
                )));
            }
        }
        Ok(layouts)
    }

    fn materialize_surface_resume_method_targets(
        &self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        surface_layout: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        continuation_layouts: &BTreeMap<
            ContinuationObjectId,
            RefactorContinuationObjectLayout<'ctx>,
        >,
        resume_packing_layouts: &BTreeMap<ResumeInterfaceId, RefactorResumeInterfaceLayout<'ctx>>,
    ) -> Result<Vec<RefactorContinuationSurfaceResumeMethodLookup>, LlvmEmitError> {
        let mut candidates = BTreeSet::new();
        for publication in entry.publications() {
            let LateLoweredSurfaceResumeDispatchPublication::InternalMethod {
                object_id,
                packing_interface_id,
                case_tag,
                reachability,
            } = publication
            else {
                continue;
            };
            if *reachability == LateLoweredContinuationMethodReachability::Reachable {
                candidates.insert((*object_id, *packing_interface_id, *case_tag));
            }
        }

        let render_candidates = || {
            if candidates.is_empty() {
                "<none>".to_string()
            } else {
                candidates
                    .iter()
                    .map(|(object_id, interface_id, case_tag)| {
                        format!(
                            "ko{} ri{}::c{}",
                            object_id.as_u32(),
                            interface_id.as_u32(),
                            case_tag.as_u32()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        };

        let first_candidate = candidates.iter().next().copied().ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 发现 continuation schema k{} 已发布为 ContinuationObjectMethod，但缺少 reachable internal method target",
                entry.continuation_schema().as_u32(),
            ))
        })?;
        let expected_object = first_candidate.0;
        if candidates
            .iter()
            .any(|(object_id, _, _)| *object_id != expected_object)
        {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 continuation schema k{} 的 surface-resume owner dispatch contract 歧义：多个 continuation object 共享同一 schema [{}]",
                entry.continuation_schema().as_u32(),
                render_candidates(),
            )));
        }

        let continuation_layout = continuation_layouts.get(&expected_object).ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 continuation schema k{} 需要的 continuation object ko{} layout，无法发布 surface-resume owner dispatch contract",
                entry.continuation_schema().as_u32(),
                expected_object.as_u32(),
            ))
        })?;
        let bindings = continuation_layout
            .surface_resume_bindings(entry.continuation_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 continuation schema k{} 的 continuation object ko{} 缺少 object-side surface-resume binding",
                    entry.continuation_schema().as_u32(),
                    expected_object.as_u32(),
                ))
            })?;

        let mut method_targets = Vec::with_capacity(candidates.len());
        for (object_id, interface_id, case_tag) in candidates {
            let packing_field_index = continuation_layout
                .field_index_for_packing(interface_id)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 continuation schema k{} 的 internal method target ko{} ri{}::c{} 缺少 object-side packing field lookup",
                        entry.continuation_schema().as_u32(),
                        object_id.as_u32(),
                        interface_id.as_u32(),
                        case_tag.as_u32(),
                    ))
                })?;
            if !bindings.iter().any(|binding| {
                binding.case_tag() == case_tag
                    && binding.return_step_schema() == surface_layout.return_step_schema()
                    && binding.reachability()
                        == LateLoweredContinuationMethodReachability::Reachable
            }) {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 continuation schema k{} 的 internal method target ko{} ri{}::c{} 缺少匹配的 reachable object-side surface-resume binding",
                    entry.continuation_schema().as_u32(),
                    object_id.as_u32(),
                    interface_id.as_u32(),
                    case_tag.as_u32(),
                )));
            }

            let interface_layout = resume_packing_layouts.get(&interface_id).ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 continuation schema k{} internal method target 需要的 resume packing ri{} layout",
                    entry.continuation_schema().as_u32(),
                    interface_id.as_u32(),
                ))
            })?;
            let method_layout = interface_layout.method(case_tag).ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 continuation schema k{} internal method target 需要的 resume method ri{}::c{} layout",
                    entry.continuation_schema().as_u32(),
                    interface_id.as_u32(),
                    case_tag.as_u32(),
                ))
            })?;
            if method_layout.return_step_schema() != surface_layout.return_step_schema()
                || method_layout.param_count() != surface_layout.param_count()
                || method_layout.resume_payload_abi().is_elided()
                    != surface_layout.resume_payload_abi().is_elided()
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 continuation schema k{} 的 surface-resume method lookup contract 漂移：surface=(out_step_schema=s{}, param_count={}, payload_elided={})，method target ko{} ri{}::c{}=(out_step_schema=s{}, param_count={}, payload_elided={})",
                    entry.continuation_schema().as_u32(),
                    surface_layout.return_step_schema().as_u32(),
                    surface_layout.param_count(),
                    surface_layout.resume_payload_abi().is_elided(),
                    object_id.as_u32(),
                    interface_id.as_u32(),
                    case_tag.as_u32(),
                    method_layout.return_step_schema().as_u32(),
                    method_layout.param_count(),
                    method_layout.resume_payload_abi().is_elided(),
                )));
            }

            method_targets.push(RefactorContinuationSurfaceResumeMethodLookup::new(
                object_id,
                interface_id,
                packing_field_index,
                case_tag,
                method_layout.vtable_index(),
            ));
        }

        Ok(method_targets)
    }

    fn materialize_surface_resume_owner_trampoline_layout(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        surface_layout: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        callable_layouts: &BTreeMap<StepSchemaId, RefactorCallableLayout<'ctx>>,
        frame_layouts: &BTreeMap<StepSchemaId, RefactorFrameLayout<'ctx>>,
        method_targets: &[RefactorContinuationSurfaceResumeMethodLookup],
    ) -> Result<RefactorContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>, LlvmEmitError> {
        let mut owner_version_key = method_targets.first().map(|lookup| {
            self.program
                .continuation_object(lookup.continuation_object())
                .expect("method target continuation object 应存在")
                .owner_version_key()
                .clone()
        });
        let mut owner_continuation_object: Option<ContinuationObjectId> = None;
        if let Some(lookup) = method_targets.first() {
            owner_continuation_object = Some(lookup.continuation_object());
        }
        let mut resume_boundary_sites = BTreeSet::new();
        let mut handle_binder_routes = BTreeSet::new();

        for publication in entry.publications() {
            match publication {
                LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                    owner_version_key: published_owner,
                    owner_continuation_object: published_object,
                    site_id,
                } => {
                    match (&owner_version_key, owner_continuation_object) {
                        (Some(existing_owner), Some(existing_object))
                            if existing_owner != published_owner
                                || existing_object != *published_object =>
                        {
                            return Err(frontend_error(format!(
                                "refactor LLVM ABI materialization 发现 continuation schema k{} 的 surface-resume owner dispatch contract 歧义：存在多个 owner continuation object（ko{} 与 ko{}）",
                                entry.continuation_schema().as_u32(),
                                existing_object.as_u32(),
                                published_object.as_u32(),
                            )));
                        }
                        _ => {
                            owner_version_key = Some(published_owner.clone());
                            owner_continuation_object = Some(*published_object);
                        }
                    }
                    if !resume_boundary_sites.insert(*site_id) {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 continuation schema k{} 的 resume boundary site {} 重复发布到 owner trampoline contract",
                            entry.continuation_schema().as_u32(),
                            site_id.as_u32(),
                        )));
                    }
                }
                LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                    owner_version_key: published_owner,
                    owner_continuation_object: published_object,
                    site_id,
                    arm_ordinal,
                    handled_case,
                } => {
                    match (&owner_version_key, owner_continuation_object) {
                        (Some(existing_owner), Some(existing_object))
                            if existing_owner != published_owner
                                || existing_object != *published_object =>
                        {
                            return Err(frontend_error(format!(
                                "refactor LLVM ABI materialization 发现 continuation schema k{} 的 surface-resume owner dispatch contract 歧义：存在多个 owner continuation object（ko{} 与 ko{}）",
                                entry.continuation_schema().as_u32(),
                                existing_object.as_u32(),
                                published_object.as_u32(),
                            )));
                        }
                        _ => {
                            owner_version_key = Some(published_owner.clone());
                            owner_continuation_object = Some(*published_object);
                        }
                    }
                    if !handle_binder_routes.insert((*site_id, *arm_ordinal, *handled_case)) {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 continuation schema k{} 的 handle continuation binder site{} arm#{} case c{} 重复发布到 owner trampoline contract",
                            entry.continuation_schema().as_u32(),
                            site_id.as_u32(),
                            arm_ordinal,
                            handled_case.as_u32(),
                        )));
                    }
                }
                LateLoweredSurfaceResumeDispatchPublication::SurfaceCase { .. }
                | LateLoweredSurfaceResumeDispatchPublication::InternalMethod { .. } => {}
            }
        }

        let owner_version_key = owner_version_key.ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 发现 continuation schema k{} 已发布为 {:?}，但缺少 owner-specific surface-resume dispatch target",
                entry.continuation_schema().as_u32(),
                entry.source_kind(),
            ))
        })?;
        let owner_continuation_object = owner_continuation_object.ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 发现 continuation schema k{} 已发布 owner-specific surface-resume dispatch，但缺少 owner continuation object",
                entry.continuation_schema().as_u32(),
            ))
        })?;
        match entry.source_kind() {
            crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ResumeBoundaryOnly
                if resume_boundary_sites.is_empty() =>
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 continuation schema k{} 已发布为 ResumeBoundaryOnly，但 owner trampoline contract 缺少 resume boundary site",
                    entry.continuation_schema().as_u32(),
                )));
            }
            crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
                if handle_binder_routes.is_empty() =>
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 continuation schema k{} 已发布为 HandleContinuationBinderOnly，但 owner trampoline contract 缺少 handle binder route",
                    entry.continuation_schema().as_u32(),
                )));
            }
            crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::OwnerTrampolineMixed
                if resume_boundary_sites.is_empty() || handle_binder_routes.is_empty() =>
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 continuation schema k{} 已发布为 OwnerTrampolineMixed，但 owner trampoline contract 未同时覆盖 resume boundary 与 handle binder route",
                    entry.continuation_schema().as_u32(),
                )));
            }
            _ => {}
        }

        let owner_callable = self
            .program
            .callable_by_version_key(&owner_version_key)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 continuation schema k{} owner trampoline 需要的 owner callable",
                    entry.continuation_schema().as_u32(),
                ))
            })?;
        if owner_callable.continuation_object() != owner_continuation_object {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 continuation schema k{} 的 owner trampoline contract 漂移：owner callable `{}` 发布 continuation object ko{}，inventory 指向 ko{}",
                entry.continuation_schema().as_u32(),
                owner_callable.root_fqn(),
                owner_callable.continuation_object().as_u32(),
                owner_continuation_object.as_u32(),
            )));
        }
        let callable_layout = callable_layouts
            .get(&owner_callable.step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 continuation schema k{} owner callable `{}` 的 callable layout，无法发布 owner trampoline contract",
                    entry.continuation_schema().as_u32(),
                    owner_callable.root_fqn(),
                ))
            })?;
        if callable_layout.continuation_object() != owner_continuation_object {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 continuation schema k{} 的 callable layout continuation object 漂移：callable `{}` -> ko{}，owner trampoline inventory -> ko{}",
                entry.continuation_schema().as_u32(),
                owner_callable.root_fqn(),
                callable_layout.continuation_object().as_u32(),
                owner_continuation_object.as_u32(),
            )));
        }
        let wrapper_projection =
            self.validate_surface_resume_wrapper_projection(entry, owner_callable, frame_layouts)?;

        let symbol_name = format!(
            "__scoop_refactor_surface_resume_owner_dispatch__{}__k{}",
            self.view.step_stem(owner_callable.step_schema()),
            entry.continuation_schema().as_u32(),
        );
        self.ensure_declared_function(&symbol_name, surface_layout.llvm_ty());

        Ok(RefactorContinuationSurfaceResumeOwnerTrampolineLayout::new(
            owner_version_key,
            owner_callable.root_fqn().to_string(),
            owner_callable.step_schema(),
            owner_continuation_object,
            symbol_name,
            surface_layout.llvm_ty(),
            surface_layout.param_count(),
            resume_boundary_sites.into_iter().collect(),
            handle_binder_routes
                .into_iter()
                .map(|(site_id, arm_ordinal, handled_case)| {
                    RefactorContinuationSurfaceResumeHandleBinderRoute::new(
                        site_id,
                        arm_ordinal,
                        handled_case,
                    )
                })
                .collect(),
            wrapper_projection,
        ))
    }

    fn validate_surface_resume_wrapper_projection(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        owner_callable: &LateLoweredCallable,
        frame_layouts: &BTreeMap<StepSchemaId, RefactorFrameLayout<'ctx>>,
    ) -> Result<Option<LateLoweredSurfaceResumeWrapperProjection>, LlvmEmitError> {
        let mut derived_candidates = Vec::<LateLoweredSurfaceResumeWrapperProjection>::new();

        for boundary in owner_callable.boundary_map().entries() {
            let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering() else {
                continue;
            };
            if lowering.facts().continuation_schema() != entry.continuation_schema() {
                continue;
            }
            let derived = self.derive_surface_resume_wrapper_projection(
                entry,
                owner_callable,
                frame_layouts,
                lowering,
            )?;
            let Some(derived) = derived else {
                continue;
            };
            if !derived_candidates
                .iter()
                .any(|candidate| same_surface_resume_wrapper_projection_shape(candidate, &derived))
            {
                derived_candidates.push(derived);
            }
        }

        if derived_candidates.len() > 1 {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 continuation schema k{} 的 owner-step -> wrapper-step projection contract 歧义：不同 resume boundary 发布了多个 shared surface wrapper projection",
                entry.continuation_schema().as_u32(),
            )));
        }

        match (entry.wrapper_projection(), derived_candidates.pop()) {
            (Some(published), Some(derived)) => {
                if !same_surface_resume_wrapper_projection_shape(published, &derived) {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 continuation schema k{} 的 owner-step -> wrapper-step projection contract 漂移：published={published:?}，derived={derived:?}",
                        entry.continuation_schema().as_u32(),
                    )));
                }
                Ok(Some(published.clone()))
            }
            (None, Some(derived)) => Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 continuation schema k{} 已桥接到 underlying route k{}，但缺少 published owner-step -> wrapper-step projection contract：derived={derived:?}",
                entry.continuation_schema().as_u32(),
                derived.underlying_route().continuation_schema().as_u32(),
            ))),
            (Some(published), None) => {
                self.validate_surface_resume_wrapper_complete_projection(
                    entry,
                    owner_callable,
                    frame_layouts,
                    published.complete(),
                )?;
                Ok(Some(published.clone()))
            }
            (None, None) => Ok(None),
        }
    }

    fn derive_surface_resume_wrapper_projection(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        owner_callable: &LateLoweredCallable,
        frame_layouts: &BTreeMap<StepSchemaId, RefactorFrameLayout<'ctx>>,
        lowering: &crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering,
    ) -> Result<Option<LateLoweredSurfaceResumeWrapperProjection>, LlvmEmitError> {
        let underlying_route = lowering.operand_contract().underlying_continuation_route();
        let owner_step = self
            .program
            .step_type(owner_callable.step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 continuation schema k{} owner-step -> wrapper-step projection 需要的 owner step schema s{}",
                    entry.continuation_schema().as_u32(),
                    owner_callable.step_schema().as_u32(),
                ))
            })?;
        let wrapper_step = self
            .program
            .step_type(lowering.facts().out_step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 continuation schema k{} owner-step -> wrapper-step projection 需要的 wrapper step schema s{}",
                    entry.continuation_schema().as_u32(),
                    lowering.facts().out_step_schema().as_u32(),
                ))
            })?;
        if underlying_route.continuation_schema() == entry.continuation_schema()
            && owner_step.step_schema() == wrapper_step.step_schema()
        {
            return Ok(None);
        }
        let underlying_inventory = self
            .program
            .surface_resume_dispatch(underlying_route.continuation_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 continuation schema k{} owner-step -> wrapper-step projection 需要的 underlying route schema k{} inventory",
                    entry.continuation_schema().as_u32(),
                    underlying_route.continuation_schema().as_u32(),
                ))
            })?;
        if underlying_route.continuation_schema() != entry.continuation_schema()
            && underlying_inventory.contract().out_step_schema() != owner_step.step_schema()
        {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 continuation schema k{} 的 wrapper projection underlying route k{} 漂移：underlying owner step=s{}，callable owner step=s{}",
                entry.continuation_schema().as_u32(),
                underlying_route.continuation_schema().as_u32(),
                underlying_inventory.contract().out_step_schema().as_u32(),
                owner_step.step_schema().as_u32(),
            )));
        }

        let outward_cases = lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|forwarding| {
                let wrapper_case = wrapper_step
                    .case(forwarding.input_case_tag())
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 continuation schema k{} 的 wrapper projection 缺少 wrapper step s{} case c{}",
                            entry.continuation_schema().as_u32(),
                            wrapper_step.step_schema().as_u32(),
                            forwarding.input_case_tag().as_u32(),
                        ))
                    })?;
                if wrapper_case.concrete_op_key() != forwarding.input_concrete_op_key() {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 continuation schema k{} 的 wrapper projection 输入 case 漂移：dispatch in c{} op={}，wrapper step s{} case op={}",
                        entry.continuation_schema().as_u32(),
                        forwarding.input_case_tag().as_u32(),
                        forwarding.input_concrete_op_key().instance_key().template.fqn,
                        wrapper_step.step_schema().as_u32(),
                        wrapper_case.concrete_op_key().instance_key().template.fqn,
                    )));
                }
                Ok(LateLoweredSurfaceResumeWrapperCaseProjection::new(
                    forwarding.emission().case_tag(),
                    forwarding.emission().concrete_op_key().clone(),
                    forwarding.emission().payload_tuple_ty(),
                    forwarding.input_case_tag(),
                    forwarding.input_concrete_op_key().clone(),
                    wrapper_case.payload_tuple_ty(),
                    wrapper_case.continuation_contract(),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(LateLoweredSurfaceResumeWrapperProjection::new(
            underlying_route.clone(),
            owner_step.step_schema(),
            wrapper_step.step_schema(),
            self.derive_surface_resume_wrapper_complete_projection(
                entry,
                owner_callable,
                frame_layouts,
                underlying_route,
                owner_step.complete_ty(),
                lowering.dispatch().complete().answer_ty(),
            )?,
            outward_cases,
        )))
    }

    fn derive_surface_resume_wrapper_complete_projection(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        owner_callable: &LateLoweredCallable,
        frame_layouts: &BTreeMap<StepSchemaId, RefactorFrameLayout<'ctx>>,
        underlying_route: &crate::effect_lowered::ir::LateLoweredContinuationRoute,
        owner_answer_ty: TypeId,
        wrapper_answer_ty: TypeId,
    ) -> Result<LateLoweredSurfaceResumeWrapperCompleteProjection, LlvmEmitError> {
        let payload_source = if owner_answer_ty == wrapper_answer_ty {
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::owner_complete(owner_answer_ty)
        } else {
            let source = self
                .wrapper_complete_payload_source_from_handle_binder(
                    entry,
                    owner_callable,
                    underlying_route,
                    wrapper_answer_ty,
                )?
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete projection 需要 t{} payload，但缺少 published wrapper payload source",
                        entry.continuation_schema().as_u32(),
                        wrapper_answer_ty.as_u32(),
                    ))
                })?;
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::wrapper_payload(source)
        };
        let projection = LateLoweredSurfaceResumeWrapperCompleteProjection::new(
            owner_answer_ty,
            wrapper_answer_ty,
            payload_source,
        );
        self.validate_surface_resume_wrapper_complete_projection(
            entry,
            owner_callable,
            frame_layouts,
            &projection,
        )?;
        Ok(projection)
    }

    fn wrapper_complete_payload_source_from_handle_binder(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        owner_callable: &LateLoweredCallable,
        underlying_route: &crate::effect_lowered::ir::LateLoweredContinuationRoute,
        wrapper_answer_ty: TypeId,
    ) -> Result<Option<LateLoweredCompletionPayloadSource>, LlvmEmitError> {
        let LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
            site_id,
            arm_ordinal,
            handled_case,
            ..
        } = underlying_route.publication()
        else {
            return Ok(None);
        };
        let source = owner_callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| {
                let LateLoweredStateTerminator::HandleDispatch {
                    site_id: state_site,
                    contract,
                    ..
                } = state.terminator()
                else {
                    return None;
                };
                if state_site != site_id {
                    return None;
                }
                contract
                    .handled_arms()
                    .iter()
                    .find(|arm| {
                        arm.arm_ordinal() == *arm_ordinal && arm.handled_case() == *handled_case
                    })
                    .map(|arm| arm.completion_payload_source().clone())
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete projection 找不到 handle binder site{} arm#{} case c{} 的 completion payload source",
                    entry.continuation_schema().as_u32(),
                    site_id.as_u32(),
                    arm_ordinal,
                    handled_case.as_u32(),
                ))
            })?;
        if source.source_ty() != wrapper_answer_ty {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete payload source type t{} 与 wrapper answer t{} 不一致",
                entry.continuation_schema().as_u32(),
                source.source_ty().as_u32(),
                wrapper_answer_ty.as_u32(),
            )));
        }
        Ok(Some(source))
    }

    fn validate_surface_resume_wrapper_complete_projection(
        &mut self,
        entry: &LateLoweredSurfaceResumeDispatchInventoryEntry,
        owner_callable: &LateLoweredCallable,
        frame_layouts: &BTreeMap<StepSchemaId, RefactorFrameLayout<'ctx>>,
        complete: &LateLoweredSurfaceResumeWrapperCompleteProjection,
    ) -> Result<(), LlvmEmitError> {
        if complete.payload_source().source_ty() != complete.wrapper_answer_ty() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete payload source type t{} 与 wrapper answer t{} 不一致",
                entry.continuation_schema().as_u32(),
                complete.payload_source().source_ty().as_u32(),
                complete.wrapper_answer_ty().as_u32(),
            )));
        }
        match complete.payload_source() {
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { answer_ty } => {
                if *answer_ty != complete.owner_answer_ty()
                    || *answer_ty != complete.wrapper_answer_ty()
                {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 continuation schema k{} 的 owner-complete wrapper payload 漂移：owner=t{} wrapper=t{} source=t{}",
                        entry.continuation_schema().as_u32(),
                        complete.owner_answer_ty().as_u32(),
                        complete.wrapper_answer_ty().as_u32(),
                        answer_ty.as_u32(),
                    )));
                }
                self.source_value_layout(*answer_ty)?;
            }
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(source) => {
                if source.source_ty() != complete.wrapper_answer_ty() {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 continuation schema k{} 的 wrapper payload source type t{} 与 wrapper answer t{} 不一致",
                        entry.continuation_schema().as_u32(),
                        source.source_ty().as_u32(),
                        complete.wrapper_answer_ty().as_u32(),
                    )));
                }
                if matches!(source, LateLoweredCompletionPayloadSource::Unit { .. })
                    && !matches!(
                        self.source_types.kind(complete.wrapper_answer_ty()),
                        TypeKind::Value(ValueTypeKind::Unit)
                    )
                {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 continuation schema k{} 对 non-Unit wrapper answer t{} 发布了 Unit wrapper complete payload source",
                        entry.continuation_schema().as_u32(),
                        complete.wrapper_answer_ty().as_u32(),
                    )));
                }
                self.source_value_layout(source.source_ty())?;
                if let LateLoweredCompletionPayloadSource::Operand(source) = source
                    && let LateLoweredOperandValueSource::Local(local) = source.value()
                    && let Some(slot_id) =
                        Self::published_frame_slot_for_local(owner_callable.frame_schema(), *local)
                {
                    let slot = owner_callable
                        .frame_schema()
                        .slots()
                        .iter()
                        .find(|slot| slot.slot_id() == slot_id)
                        .expect("published_frame_slot_for_local returned existing slot");
                    if slot.ty() != source.source_ty() {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete payload home slot fs{} 类型 t{} 与 source type t{} 不一致",
                            entry.continuation_schema().as_u32(),
                            slot_id.as_u32(),
                            slot.ty().as_u32(),
                            source.source_ty().as_u32(),
                        )));
                    }
                    let frame_layout = frame_layouts.get(&owner_callable.step_schema()).ok_or_else(|| {
                        frontend_error(format!(
                            "refactor LLVM ABI materialization 缺少 callable `{}` frame layout，无法校验 wrapper complete payload source",
                            owner_callable.root_fqn(),
                        ))
                    })?;
                    if frame_layout.field_index_for_slot(slot_id).is_none() {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 continuation schema k{} 的 wrapper complete payload home slot fs{} 在 frame layout 中缺少 field",
                            entry.continuation_schema().as_u32(),
                            slot_id.as_u32(),
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    fn publish_callable_carrier_entry_shells(
        &mut self,
        callable_layouts: &BTreeMap<StepSchemaId, RefactorCallableLayout<'ctx>>,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
    ) -> Result<
        HashMap<(RefactorCallableCarrierKind, String), RefactorCallableCarrierTargetLayout>,
        LlvmEmitError,
    > {
        let published_callable_roots = self
            .program
            .callables()
            .iter()
            .map(|callable| callable.root_fqn())
            .collect::<BTreeSet<_>>();
        let closure_targets = published_callable_roots.clone();
        let class_vtable_targets = self
            .codegen
            .class_vtables
            .values()
            .flat_map(|slots| slots.iter().map(|slot| slot.impl_member_fqn.as_str()))
            .filter(|impl_fqn| published_callable_roots.contains(impl_fqn))
            .collect::<BTreeSet<_>>();
        let interface_itable_targets = self
            .codegen
            .class_itables
            .values()
            .flat_map(|entries| {
                entries.iter().flat_map(|entry| {
                    entry
                        .method_impl_fqns
                        .iter()
                        .filter(|impl_fqn| !impl_fqn.is_empty())
                        .map(String::as_str)
                })
            })
            .filter(|impl_fqn| published_callable_roots.contains(impl_fqn))
            .collect::<BTreeSet<_>>();

        let mut carrier_layouts = HashMap::new();
        for callable_fqn in closure_targets {
            self.publish_closure_carrier_entry_shell(
                callable_fqn,
                callable_layouts,
                step_layouts,
                &mut carrier_layouts,
            )?;
        }
        for impl_fqn in class_vtable_targets {
            self.publish_dispatch_carrier_entry_shell(
                RefactorCallableCarrierKind::ClassVtable,
                impl_fqn,
                callable_layouts,
                step_layouts,
                &mut carrier_layouts,
            )?;
        }
        for impl_fqn in interface_itable_targets {
            self.publish_dispatch_carrier_entry_shell(
                RefactorCallableCarrierKind::InterfaceItable,
                impl_fqn,
                callable_layouts,
                step_layouts,
                &mut carrier_layouts,
            )?;
        }

        self.codegen.enable_refactor_callable_carrier_contract();
        Ok(carrier_layouts)
    }

    fn publish_closure_carrier_entry_shell(
        &mut self,
        callable_fqn: &str,
        callable_layouts: &BTreeMap<StepSchemaId, RefactorCallableLayout<'ctx>>,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
        carrier_layouts: &mut HashMap<
            (RefactorCallableCarrierKind, String),
            RefactorCallableCarrierTargetLayout,
        >,
    ) -> Result<(), LlvmEmitError> {
        let callable_layout = self.callable_layout_for_carrier_target(
            callable_layouts,
            RefactorCallableCarrierKind::ClosureObject,
            callable_fqn,
        )?;
        let step_ty = step_layouts
            .get(&callable_layout.step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{}` closure carrier target 的 step layout {}",
                    callable_fqn,
                    callable_layout.step_schema().as_u32(),
                ))
            })?
            .llvm_ty();
        let args_abi = self.closure_carrier_args_abi(callable_fqn)?;
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> =
            vec![self.codegen.llvm_gc_i8_ptr_type().into()];
        if !args_abi.is_elided() {
            params.push(args_abi.llvm_ty().into());
        }
        let symbol_name = format!(
            "__scoop_refactor_closure_dynamic_entry__{}",
            self.view.step_stem(callable_layout.step_schema())
        );
        self.ensure_declared_function(&symbol_name, step_ty.fn_type(&params, false));
        self.register_callable_carrier_target_contract(
            RefactorCallableCarrierKind::ClosureObject,
            callable_fqn,
            callable_layout,
            &symbol_name,
            carrier_layouts,
        )?;
        if let Some(alias) = legacy_hir_closure_carrier_alias(callable_fqn) {
            self.register_callable_carrier_target_contract(
                RefactorCallableCarrierKind::ClosureObject,
                &alias,
                callable_layout,
                &symbol_name,
                carrier_layouts,
            )?;
        }
        Ok(())
    }

    fn publish_dispatch_carrier_entry_shell(
        &mut self,
        kind: RefactorCallableCarrierKind,
        impl_fqn: &str,
        callable_layouts: &BTreeMap<StepSchemaId, RefactorCallableLayout<'ctx>>,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
        carrier_layouts: &mut HashMap<
            (RefactorCallableCarrierKind, String),
            RefactorCallableCarrierTargetLayout,
        >,
    ) -> Result<(), LlvmEmitError> {
        let callable_layout =
            self.callable_layout_for_carrier_target(callable_layouts, kind, impl_fqn)?;
        let step_ty = step_layouts
            .get(&callable_layout.step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 {} `{}` target 的 step layout {}",
                    kind.label(),
                    impl_fqn,
                    callable_layout.step_schema().as_u32(),
                ))
            })?
            .llvm_ty();
        let (receiver_abi, args_abi) = self.dispatch_carrier_receiver_and_args_abi(impl_fqn)?;
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> = vec![receiver_abi.llvm_ty().into()];
        if !args_abi.is_elided() {
            params.push(args_abi.llvm_ty().into());
        }
        let symbol_name = format!(
            "__scoop_refactor_{}_dynamic_entry__{}",
            match kind {
                RefactorCallableCarrierKind::ClassVtable => "vtable",
                RefactorCallableCarrierKind::InterfaceItable => "itable",
                RefactorCallableCarrierKind::ClosureObject => "closure",
            },
            self.view.step_stem(callable_layout.step_schema())
        );
        self.ensure_declared_function(&symbol_name, step_ty.fn_type(&params, false));
        self.register_callable_carrier_target_contract(
            kind,
            impl_fqn,
            callable_layout,
            &symbol_name,
            carrier_layouts,
        )?;
        Ok(())
    }

    fn register_callable_carrier_target_contract(
        &self,
        kind: RefactorCallableCarrierKind,
        callable_fqn: &str,
        callable_layout: &RefactorCallableLayout<'ctx>,
        symbol_name: &str,
        carrier_layouts: &mut HashMap<
            (RefactorCallableCarrierKind, String),
            RefactorCallableCarrierTargetLayout,
        >,
    ) -> Result<(), LlvmEmitError> {
        self.codegen
            .register_refactor_callable_carrier_entry_symbol(kind, callable_fqn, symbol_name)?;

        let key = (kind, callable_fqn.to_string());
        let published = RefactorCallableCarrierTargetLayout::new(
            callable_fqn.to_string(),
            callable_layout.body_version_key().clone(),
            callable_layout.step_schema(),
            symbol_name.to_string(),
        );
        if let Some(existing) = carrier_layouts.get(&key) {
            if existing != &published {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 {} `{}` 重复发布了不兼容的 callable version contract：已有 {:?}，新值 {:?}",
                    kind.label(),
                    callable_fqn,
                    existing,
                    published,
                )));
            }
            return Ok(());
        }
        carrier_layouts.insert(key, published);
        Ok(())
    }

    fn callable_layout_for_carrier_target<'b>(
        &self,
        callable_layouts: &'b BTreeMap<StepSchemaId, RefactorCallableLayout<'ctx>>,
        kind: RefactorCallableCarrierKind,
        callable_fqn: &str,
    ) -> Result<&'b RefactorCallableLayout<'ctx>, LlvmEmitError> {
        let matches = callable_layouts
            .values()
            .filter(|layout| layout.root_fqn() == callable_fqn)
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [] => Err(frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 {} `{}` 的 published callable version，无法发布 carrier target",
                kind.label(),
                callable_fqn,
            ))),
            [layout] => Ok(*layout),
            _ => Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 {} `{}` 存在多个 published callable version {:?}，缺少 authoritative version selector，无法发布 carrier target",
                kind.label(),
                callable_fqn,
                matches
                    .iter()
                    .map(|layout| layout.body_version_key())
                    .collect::<Vec<_>>(),
            ))),
        }
    }

    fn closure_carrier_args_abi(
        &mut self,
        root_fqn: &str,
    ) -> Result<RefactorAbiValue<'ctx>, LlvmEmitError> {
        if let Some(callable) = self.pass_view.callable(root_fqn) {
            let skip = usize::from(callable.name.starts_with("$lambda"));
            let component_tys = callable
                .params
                .iter()
                .skip(skip)
                .map(|param| param.ty)
                .collect::<Vec<_>>();
            return self.canonical_tuple_abi_from_types(
                &self.pass_view.materialized().types,
                &component_tys,
            );
        }
        if let Some(fun) = self.codegen.fun_index.get(root_fqn).copied() {
            let component_tys = fun.params.iter().map(|param| param.ty).collect::<Vec<_>>();
            return self.canonical_tuple_abi_from_types(self.codegen.types, &component_tys);
        }
        Err(frontend_error(format!(
            "refactor LLVM ABI materialization 缺少 closure-like callable `{root_fqn}` 的 authoritative signature，无法发布 closure carrier target"
        )))
    }

    fn dispatch_carrier_receiver_and_args_abi(
        &mut self,
        impl_fqn: &str,
    ) -> Result<(RefactorAbiValue<'ctx>, RefactorAbiValue<'ctx>), LlvmEmitError> {
        let fun = self.codegen.fun_index.get(impl_fqn).copied().ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 dispatch target `{impl_fqn}` 的 authoritative HIR signature，无法发布 vtable/itable carrier target"
            ))
        })?;
        let Some((receiver, explicit_params)) = fun.params.split_first() else {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 dispatch target `{impl_fqn}` 没有 receiver 参数，无法发布 vtable/itable carrier target"
            )));
        };
        let args = explicit_params
            .iter()
            .map(|param| param.ty)
            .collect::<Vec<_>>();
        Ok((
            self.abi_value_from_types(self.codegen.types, receiver.ty)?,
            self.canonical_tuple_abi_from_types(self.codegen.types, &args)?,
        ))
    }

    fn canonical_tuple_abi_from_types(
        &mut self,
        types: &TypeStore,
        components: &[TypeId],
    ) -> Result<RefactorAbiValue<'ctx>, LlvmEmitError> {
        match components {
            [] => Ok(RefactorAbiValue::new(
                self.codegen.context.struct_type(&[], false).into(),
                true,
            )),
            [single] => self.abi_value_from_types(types, *single),
            _ => {
                let mut fields = Vec::with_capacity(components.len());
                for component in components {
                    let llvm_ty = self.llvm_abi_type_of_types(types, *component)?;
                    if self.codegen.target_data.get_store_size(&llvm_ty) == 0 {
                        continue;
                    }
                    fields.push(llvm_ty);
                }
                let llvm_ty = self.codegen.context.struct_type(&fields, false).into();
                Ok(RefactorAbiValue::new(
                    llvm_ty,
                    self.codegen.target_data.get_store_size(&llvm_ty) == 0,
                ))
            }
        }
    }

    fn materialize_dynamic_invoke_layouts(
        &mut self,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
    ) -> Result<
        BTreeMap<(StepSchemaId, crate::mir::SiteId), RefactorDynamicInvokeLayout<'ctx>>,
        LlvmEmitError,
    > {
        let mut layouts = BTreeMap::new();
        for callable in self.program.callables() {
            self.publish_boundary_dynamic_invoke_layouts(callable, step_layouts, &mut layouts)?;
            self.publish_source_slice_dynamic_invoke_layouts(callable, step_layouts, &mut layouts)?;
        }
        Ok(layouts)
    }

    fn materialize_boundary_operand_layouts(
        &mut self,
        dynamic_invoke_layouts: &BTreeMap<
            (StepSchemaId, crate::mir::SiteId),
            RefactorDynamicInvokeLayout<'ctx>,
        >,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeLayout<'ctx>,
        >,
        surface_resume_dispatch_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeDispatchLayout<'ctx>,
        >,
    ) -> Result<BoundaryOperandLayoutSets, LlvmEmitError> {
        let mut call_layouts = BTreeMap::new();
        let mut perform_layouts = BTreeMap::new();
        let mut resume_layouts = BTreeMap::new();

        for callable in self.program.callables() {
            for boundary in callable.boundary_map().entries() {
                match (boundary.source(), boundary.lowering()) {
                    (
                        LateLoweredBoundarySource::Site {
                            site_id,
                            kind: BoundarySiteKind::Call,
                        },
                        Some(LateLoweredBoundaryLowering::Call(lowering)),
                    ) => {
                        self.validate_call_boundary_operand_contract(
                            callable,
                            boundary,
                            site_id,
                            lowering,
                            dynamic_invoke_layouts,
                            surface_resume_layouts,
                        )?;
                        let key = (callable.step_schema(), site_id);
                        if call_layouts.contains_key(&key) {
                            return Err(frontend_error(format!(
                                "refactor LLVM ABI materialization 发现 owner step schema {} call site {} 的 boundary operand contract 重复发布",
                                callable.step_schema().as_u32(),
                                site_id.as_u32(),
                            )));
                        }
                        call_layouts.insert(
                            key,
                            RefactorCallBoundaryOperandLayout::new(
                                callable.step_schema(),
                                site_id,
                                lowering.operand_contract().clone(),
                            ),
                        );
                    }
                    (
                        LateLoweredBoundarySource::Site {
                            site_id,
                            kind: BoundarySiteKind::Perform,
                        },
                        Some(LateLoweredBoundaryLowering::Perform(lowering)),
                    ) => {
                        self.validate_perform_boundary_operand_contract(
                            callable, boundary, site_id, lowering,
                        )?;
                        let key = (callable.step_schema(), site_id);
                        if perform_layouts.contains_key(&key) {
                            return Err(frontend_error(format!(
                                "refactor LLVM ABI materialization 发现 owner step schema {} perform site {} 的 boundary operand contract 重复发布",
                                callable.step_schema().as_u32(),
                                site_id.as_u32(),
                            )));
                        }
                        perform_layouts.insert(
                            key,
                            RefactorPerformBoundaryOperandLayout::new(
                                callable.step_schema(),
                                site_id,
                                lowering.operand_contract().clone(),
                            ),
                        );
                    }
                    (
                        LateLoweredBoundarySource::Site {
                            site_id,
                            kind: BoundarySiteKind::Resume,
                        },
                        Some(LateLoweredBoundaryLowering::Resume(lowering)),
                    ) => {
                        self.validate_resume_boundary_operand_contract(
                            callable,
                            boundary,
                            site_id,
                            lowering,
                            surface_resume_layouts,
                            surface_resume_dispatch_layouts,
                        )?;
                        let key = (callable.step_schema(), site_id);
                        if resume_layouts.contains_key(&key) {
                            return Err(frontend_error(format!(
                                "refactor LLVM ABI materialization 发现 owner step schema {} resume site {} 的 boundary operand contract 重复发布",
                                callable.step_schema().as_u32(),
                                site_id.as_u32(),
                            )));
                        }
                        resume_layouts.insert(
                            key,
                            RefactorResumeBoundaryOperandLayout::new(
                                callable.step_schema(),
                                site_id,
                                lowering.operand_contract().clone(),
                            ),
                        );
                    }
                    _ => {}
                }
            }
        }

        Ok((call_layouts, perform_layouts, resume_layouts))
    }

    fn validate_boundary_source_consumption(
        &self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
        kind: &'static str,
        owner_slices: &[LateLoweredStateSlice],
        consumption: LateLoweredBoundarySourceConsumption,
        expect_statement: bool,
    ) -> Result<(), LlvmEmitError> {
        let source_slice = consumption.source_slice();
        if !owner_slices.contains(&source_slice) {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 published anchor slice {:?} 不属于 owner state source_slices",
                site_id.as_u32(),
                source_slice,
            )));
        }
        match consumption {
            LateLoweredBoundarySourceConsumption::Statement {
                source_slice,
                statement_index,
                consumes_last_statement,
            } => {
                if !expect_statement {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 错误地发布了 statement anchor",
                        site_id.as_u32(),
                    )));
                }
                if statement_index < source_slice.start_statement_index()
                    || statement_index >= source_slice.end_statement_index()
                {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 statement anchor {} 越界于 published source slice {:?}",
                        site_id.as_u32(),
                        statement_index,
                        source_slice,
                    )));
                }
                let expected_last =
                    statement_index.saturating_add(1) == source_slice.end_statement_index();
                if consumes_last_statement != expected_last {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 consumes_last_statement 漂移：published={}，expected={expected_last}",
                        site_id.as_u32(),
                        consumes_last_statement,
                    )));
                }
            }
            LateLoweredBoundarySourceConsumption::Terminator { source_slice } => {
                if expect_statement {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 错误地发布了 terminator anchor",
                        site_id.as_u32(),
                    )));
                }
                if !source_slice.includes_terminator() {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 terminator anchor 所在 source slice 没有包含 terminator",
                        site_id.as_u32(),
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_source_statement_classifications(&self) -> Result<(), LlvmEmitError> {
        for callable in self.program.callables() {
            let Some(fun) = self.pass_view.callable(callable.root_fqn()) else {
                if callable.source_statement_classifications().is_empty() {
                    continue;
                }
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` 发布了 source-slice statement classification，但 pass-view 缺少对应 body",
                    callable.root_fqn(),
                )));
            };
            let Some(body) = fun.body.as_ref() else {
                if callable.source_statement_classifications().is_empty() {
                    continue;
                }
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` 发布了 source-slice statement classification，但 callable 无 body",
                    callable.root_fqn(),
                )));
            };

            let mut expected = BTreeSet::<(BasicBlockId, u32)>::new();
            for state in callable.state_graph().states() {
                for slice in state.source_slices() {
                    let block = body
                        .blocks
                        .get(slice.block_id().as_u32() as usize)
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "refactor LLVM ABI materialization 发现 callable `{}` state st{} source slice 指向缺失 block bb{}",
                                callable.root_fqn(),
                                state.state_id().as_u32(),
                                slice.block_id().as_u32(),
                            ))
                        })?;
                    let start = slice.start_statement_index() as usize;
                    let end = slice.end_statement_index() as usize;
                    if start > end || end > block.stmts.len() {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` state st{} source slice [{}..{}) 越界于 bb{}（stmt_count={}）",
                            callable.root_fqn(),
                            state.state_id().as_u32(),
                            slice.start_statement_index(),
                            slice.end_statement_index(),
                            slice.block_id().as_u32(),
                            block.stmts.len(),
                        )));
                    }
                    for stmt_index in slice.start_statement_index()..slice.end_statement_index() {
                        expected.insert((slice.block_id(), stmt_index));
                    }
                }
            }

            let mut classified =
                BTreeMap::<(BasicBlockId, u32), LateLoweredSourceStatementClassificationKind>::new(
                );
            for classification in callable.source_statement_classifications() {
                let key = (
                    classification.source_slice().block_id(),
                    classification.statement_index(),
                );
                if !expected.contains(&key) {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` 的 source-slice statement classification bb{} stmt{} 不属于任何 published source_slices",
                        callable.root_fqn(),
                        key.0.as_u32(),
                        key.1,
                    )));
                }
                if classified.insert(key, classification.kind()).is_some() {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` 的 source-slice statement classification bb{} stmt{} 重复发布",
                        callable.root_fqn(),
                        key.0.as_u32(),
                        key.1,
                    )));
                }
            }
            if classified.len() != expected.len() {
                let missing = expected
                    .iter()
                    .find(|key| !classified.contains_key(key))
                    .expect("classified length drift should expose a missing key");
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` source-slice statement bb{} stmt{} 缺少 classification",
                    callable.root_fqn(),
                    missing.0.as_u32(),
                    missing.1,
                )));
            }

            self.validate_boundary_anchor_classifications(callable, &classified)?;
            self.validate_resume_payload_classifications(callable, &classified)?;
        }
        Ok(())
    }

    fn validate_boundary_anchor_classifications(
        &self,
        callable: &LateLoweredCallable,
        classified: &BTreeMap<(BasicBlockId, u32), LateLoweredSourceStatementClassificationKind>,
    ) -> Result<(), LlvmEmitError> {
        for boundary in callable.boundary_map().entries() {
            let Some(LateLoweredBoundarySourceConsumption::Statement {
                source_slice,
                statement_index,
                ..
            }) = boundary_source_consumption(boundary)
            else {
                continue;
            };
            let key = (source_slice.block_id(), statement_index);
            match classified.get(&key) {
                Some(LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor {
                    boundary_id,
                }) if *boundary_id == boundary.boundary_id() => {}
                other => {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 consumed anchor bb{} stmt{} classification 漂移：{:?}",
                        callable.root_fqn(),
                        boundary.boundary_id().as_u32(),
                        key.0.as_u32(),
                        key.1,
                        other,
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_resume_payload_classifications(
        &self,
        callable: &LateLoweredCallable,
        classified: &BTreeMap<(BasicBlockId, u32), LateLoweredSourceStatementClassificationKind>,
    ) -> Result<(), LlvmEmitError> {
        for kind in classified.values() {
            let LateLoweredSourceStatementClassificationKind::ResumePayloadInjection {
                boundary_id,
                resume_state,
                consumer_local,
            } = kind
            else {
                continue;
            };
            let Some(binding) = callable.frame_schema().resume_payload_binding(*boundary_id) else {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} resume payload injection classification 缺少对应 binding",
                    callable.root_fqn(),
                    boundary_id.as_u32(),
                )));
            };
            if binding.resume_state() != *resume_state
                || binding.consumer_local() != *consumer_local
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} resume payload injection classification 漂移",
                    callable.root_fqn(),
                    boundary_id.as_u32(),
                )));
            }
        }
        Ok(())
    }

    fn validate_boundary_operand_source_layout(
        &mut self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
        kind: &'static str,
        label: &'static str,
        source: &LateLoweredOperandSource,
    ) -> Result<(), LlvmEmitError> {
        self.source_value_layout(source.source_ty()).map(|_| ()).map_err(|err| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` {kind} site {} {label} source type t{} 的 ABI value lowering contract：{err}",
                site_id.as_u32(),
                source.source_ty().as_u32(),
            ))
        })
    }

    fn validate_ordered_boundary_sources(
        &mut self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
        kind: &'static str,
        label: &'static str,
        sources: &[LateLoweredOperandSource],
        expected_tuple_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        let expected_components =
            expected_source_types_for_carrier(self.source_types, expected_tuple_ty, sources.len())
                .map_err(|detail| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 {label} contract 非法：{detail}",
                        site_id.as_u32(),
                    ))
                })?;
        if sources.len() != expected_components.len() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 {label} 数量({}) 与 published carrier t{} 的 component 数量({}) 不一致",
                site_id.as_u32(),
                sources.len(),
                expected_tuple_ty.as_u32(),
                expected_components.len(),
            )));
        }
        for (index, (source, expected_ty)) in sources.iter().zip(expected_components).enumerate() {
            if source.source_ty() != expected_ty {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` {kind} site {} 的 {label}[{}] source_ty 漂移：published=t{}，expected=t{}",
                    site_id.as_u32(),
                    index,
                    source.source_ty().as_u32(),
                    expected_ty.as_u32(),
                )));
            }
            self.validate_boundary_operand_source_layout(
                owner_root_fqn,
                site_id,
                kind,
                label,
                source,
            )?;
        }
        Ok(())
    }

    fn validate_call_boundary_operand_contract(
        &mut self,
        callable: &LateLoweredCallable,
        boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
        site_id: crate::mir::SiteId,
        lowering: &crate::effect_lowered::ir::LateLoweredCallBoundaryLowering,
        dynamic_invoke_layouts: &BTreeMap<
            (StepSchemaId, crate::mir::SiteId),
            RefactorDynamicInvokeLayout<'ctx>,
        >,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        let owner_state = callable.state_graph().state(boundary.owner_state()).ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{}` call site {} owner state st{}，无法发布 boundary operand contract",
                callable.root_fqn(),
                site_id.as_u32(),
                boundary.owner_state().as_u32(),
            ))
        })?;
        self.validate_boundary_source_consumption(
            callable.root_fqn(),
            site_id,
            "call",
            owner_state.source_slices(),
            lowering.operand_contract().source_consumption(),
            true,
        )?;
        self.validate_ordered_boundary_sources(
            callable.root_fqn(),
            site_id,
            "call",
            "ordered args",
            lowering.operand_contract().arg_sources(),
            lowering.facts().invoke_args_tuple_ty(),
        )?;
        match lowering.facts().kind() {
            CallSiteKind::Direct => {
                if lowering.operand_contract().carrier_source().is_some() {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 direct call boundary 错误地发布了 carrier source",
                        callable.root_fqn(),
                        site_id.as_u32(),
                    )));
                }
            }
            CallSiteKind::Closure
            | CallSiteKind::FunValue
            | CallSiteKind::Virtual
            | CallSiteKind::Interface => {
                let carrier = lowering.operand_contract().carrier_source().ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 non-KnownInstance boundary 缺少 carrier source contract",
                        callable.root_fqn(),
                        site_id.as_u32(),
                    ))
                })?;
                self.validate_boundary_operand_source_layout(
                    callable.root_fqn(),
                    site_id,
                    "call",
                    "carrier",
                    carrier,
                )?;
                let layout = dynamic_invoke_layouts.get(&(callable.step_schema(), site_id)).ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 缺少 callable `{}` call site {} 的 dynamic-invoke contract，无法校验 carrier source",
                        callable.root_fqn(),
                        site_id.as_u32(),
                    ))
                })?;
                let source_layout = self.source_value_layout(carrier.source_ty())?;
                if source_layout.abi().is_elided() != layout.carrier().receiver_abi().is_elided()
                    || source_layout.abi().llvm_ty() != layout.carrier().receiver_abi().llvm_ty()
                {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 carrier source ABI 与 published dynamic-invoke carrier 漂移",
                        callable.root_fqn(),
                        site_id.as_u32(),
                    )));
                }
            }
        }
        self.validate_call_boundary_continuation_compositions(
            callable,
            boundary,
            site_id,
            lowering,
            surface_resume_layouts,
        )?;
        Ok(())
    }

    fn validate_call_boundary_continuation_compositions(
        &self,
        callable: &LateLoweredCallable,
        boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
        site_id: crate::mir::SiteId,
        lowering: &crate::effect_lowered::ir::LateLoweredCallBoundaryLowering,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        let input_step = self
            .program
            .step_type(lowering.dispatch().input_step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 continuation composition 缺少 input StepSchema s{}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    lowering.dispatch().input_step_schema().as_u32(),
                ))
            })?;
        let mut seen_input_cases = BTreeSet::new();
        for composition in lowering.continuation_compositions() {
            if !seen_input_cases.insert(composition.input_case_tag()) {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` call site {} 对 input case c{} 重复发布 call-boundary continuation composition",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    composition.input_case_tag().as_u32(),
                )));
            }
            self.validate_call_boundary_continuation_composition(
                callable,
                boundary,
                site_id,
                lowering,
                input_step,
                composition,
                surface_resume_layouts,
            )?;
        }
        for forwarding in lowering.dispatch().outward_cases() {
            if lowering
                .continuation_composition_for_input_case(forwarding.input_case_tag())
                .is_none()
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` call site {} outward case c{} 缺少 call-boundary continuation composition contract",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    forwarding.input_case_tag().as_u32(),
                )));
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_call_boundary_continuation_composition(
        &self,
        callable: &LateLoweredCallable,
        boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
        site_id: crate::mir::SiteId,
        lowering: &crate::effect_lowered::ir::LateLoweredCallBoundaryLowering,
        input_step: &LateLoweredStepType,
        composition: &LateLoweredCallBoundaryContinuationComposition,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        if composition.boundary_id() != boundary.boundary_id()
            || composition.input_step_schema() != lowering.dispatch().input_step_schema()
            || composition.caller_resume_state() != boundary.resume_state()
            || composition.caller_result_local() != lowering.result_local()
        {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition 与 boundary/result contract 漂移：composition={:?}",
                callable.root_fqn(),
                site_id.as_u32(),
                composition,
            )));
        }
        let binding = callable
            .frame_schema()
            .resume_payload_binding(boundary.boundary_id())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition 缺少 result home binding",
                    callable.root_fqn(),
                    site_id.as_u32(),
                ))
            })?;
        if binding.resume_state() != composition.caller_resume_state()
            || binding.consumer_local() != composition.caller_result_local()
            || binding.consumer_frame_slot() != composition.caller_result_frame_slot()
        {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition result home 漂移：binding={:?} composition={:?}",
                callable.root_fqn(),
                site_id.as_u32(),
                binding,
                composition,
            )));
        }
        if let Some(frame_slot) = composition.caller_result_frame_slot() {
            let slot = callable
                .frame_schema()
                .slots()
                .iter()
                .find(|slot| slot.slot_id() == frame_slot)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition result frame fs{} 不存在",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        frame_slot.as_u32(),
                    ))
                })?;
            if slot.ty() != composition.caller_result_ty() {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition result frame fs{} 类型 t{} 与 result t{} 不一致",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    frame_slot.as_u32(),
                    slot.ty().as_u32(),
                    composition.caller_result_ty().as_u32(),
                )));
            }
        }
        let input_case = input_step
            .case(composition.input_case_tag())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition 引用缺失 input case c{}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    composition.input_case_tag().as_u32(),
                ))
            })?;
        if input_case.continuation_contract() != composition.callee_continuation_contract()
            || input_step.complete_ty() != composition.caller_result_ty()
        {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition callee contract 漂移：composition={:?}",
                callable.root_fqn(),
                site_id.as_u32(),
                composition,
            )));
        }
        let forwarding = lowering
            .dispatch()
            .outward_cases()
            .iter()
            .find(|forwarding| forwarding.input_case_tag() == composition.input_case_tag())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition 没有对应 dispatch forwarding c{}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    composition.input_case_tag().as_u32(),
                ))
            })?;
        if forwarding.emission().case_tag() != composition.output_case_tag()
            || forwarding.emission().continuation_contract()
                != composition.caller_continuation_contract()
        {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition caller contract 漂移：composition={:?}",
                callable.root_fqn(),
                site_id.as_u32(),
                composition,
            )));
        }
        if input_case.resume_tuple_ty()
            != forwarding
                .emission()
                .continuation_contract()
                .resume_tuple_ty()
        {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition resume payload type 漂移：callee=t{} caller=t{}",
                callable.root_fqn(),
                site_id.as_u32(),
                input_case.resume_tuple_ty().as_u32(),
                forwarding
                    .emission()
                    .continuation_contract()
                    .resume_tuple_ty()
                    .as_u32(),
            )));
        }
        let callee_surface = surface_resume_layouts
            .get(&composition.callee_continuation_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition 缺少 callee continuation schema k{} surface resume ABI",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    composition.callee_continuation_schema().as_u32(),
                ))
            })?;
        if callee_surface.resume_tuple_ty()
            != composition.callee_continuation_contract().resume_tuple_ty()
            || callee_surface.return_step_schema()
                != composition.callee_continuation_contract().out_step_schema()
        {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 call-boundary continuation composition callee surface ABI 漂移：composition={:?}",
                callable.root_fqn(),
                site_id.as_u32(),
                composition,
            )));
        }
        Ok(())
    }

    fn validate_perform_boundary_operand_contract(
        &mut self,
        callable: &LateLoweredCallable,
        boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
        site_id: crate::mir::SiteId,
        lowering: &crate::effect_lowered::ir::LateLoweredPerformBoundaryLowering,
    ) -> Result<(), LlvmEmitError> {
        let owner_state = callable.state_graph().state(boundary.owner_state()).ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{}` perform site {} owner state st{}，无法发布 boundary operand contract",
                callable.root_fqn(),
                site_id.as_u32(),
                boundary.owner_state().as_u32(),
            ))
        })?;
        self.validate_boundary_source_consumption(
            callable.root_fqn(),
            site_id,
            "perform",
            owner_state.source_slices(),
            lowering.operand_contract().source_consumption(),
            false,
        )?;
        self.validate_ordered_boundary_sources(
            callable.root_fqn(),
            site_id,
            "perform",
            "payload sources",
            lowering.operand_contract().payload_sources(),
            lowering.facts().payload_tuple_ty(),
        )
    }

    fn validate_resume_boundary_operand_contract(
        &mut self,
        callable: &LateLoweredCallable,
        boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
        site_id: crate::mir::SiteId,
        lowering: &crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeLayout<'ctx>,
        >,
        surface_resume_dispatch_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeDispatchLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        let owner_state = callable.state_graph().state(boundary.owner_state()).ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{}` resume site {} owner state st{}，无法发布 boundary operand contract",
                callable.root_fqn(),
                site_id.as_u32(),
                boundary.owner_state().as_u32(),
            ))
        })?;
        self.validate_boundary_source_consumption(
            callable.root_fqn(),
            site_id,
            "resume",
            owner_state.source_slices(),
            lowering.operand_contract().source_consumption(),
            true,
        )?;
        self.validate_boundary_operand_source_layout(
            callable.root_fqn(),
            site_id,
            "resume",
            "continuation",
            lowering.operand_contract().continuation_source(),
        )?;
        self.validate_ordered_boundary_sources(
            callable.root_fqn(),
            site_id,
            "resume",
            "ordered args",
            lowering.operand_contract().arg_sources(),
            lowering.facts().resume_tuple_ty(),
        )?;
        let route = lowering.operand_contract().underlying_continuation_route();
        let inventory = self
            .program
            .surface_resume_dispatch(route.continuation_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{}` resume site {} underlying continuation schema k{} 的 published surface-resume dispatch inventory",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    route.continuation_schema().as_u32(),
                ))
            })?;
        if !inventory.publications().contains(route.publication()) {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` resume site {} 的 underlying continuation route 漂移：schema k{} 缺少 publication {:?}",
                callable.root_fqn(),
                site_id.as_u32(),
                route.continuation_schema().as_u32(),
                route.publication(),
            )));
        }
        let surface_layout = surface_resume_layouts
            .get(&lowering.facts().continuation_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{}` resume site {} continuation schema k{} 的 surface-resume layout",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    lowering.facts().continuation_schema().as_u32(),
                ))
            })?;
        if surface_layout.resume_tuple_ty() != lowering.facts().resume_tuple_ty()
            || surface_layout.answer_ty() != lowering.facts().answer_ty()
            || surface_layout.return_step_schema() != lowering.facts().out_step_schema()
        {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` resume site {} 的 surface-resume layout 与 published facts 漂移",
                callable.root_fqn(),
                site_id.as_u32(),
            )));
        }
        if !surface_resume_dispatch_layouts.contains_key(&lowering.facts().continuation_schema()) {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{}` resume site {} continuation schema k{} 的 surface-resume owner dispatch contract",
                callable.root_fqn(),
                site_id.as_u32(),
                lowering.facts().continuation_schema().as_u32(),
            )));
        }
        Ok(())
    }

    fn materialize_resume_payload_binding_layouts(
        &mut self,
        frame_layouts: &BTreeMap<StepSchemaId, RefactorFrameLayout<'ctx>>,
    ) -> Result<
        (
            ResumePayloadBindingLayouts,
            ResumePayloadBindingLayoutsByState,
        ),
        LlvmEmitError,
    > {
        let mut bindings_by_boundary = ResumePayloadBindingLayouts::new();
        let mut bindings_by_state = ResumePayloadBindingLayoutsByState::new();

        for callable in self.program.callables() {
            let frame_layout = frame_layouts.get(&callable.step_schema()).ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{}` step schema s{} 的 frame layout，无法发布 resumed local/home contract",
                    callable.root_fqn(),
                    callable.step_schema().as_u32(),
                ))
            })?;

            for boundary in callable.boundary_map().entries() {
                let requires_binding = matches!(
                    boundary.lowering(),
                    Some(
                        LateLoweredBoundaryLowering::Call(_)
                            | LateLoweredBoundaryLowering::Perform(_)
                            | LateLoweredBoundaryLowering::Resume(_)
                            | LateLoweredBoundaryLowering::RuntimeError(_)
                    )
                );
                if requires_binding
                    && callable
                        .frame_schema()
                        .resume_payload_binding(boundary.boundary_id())
                        .is_none()
                {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} 缺少 resumed local/home contract",
                        callable.root_fqn(),
                        boundary.boundary_id().as_u32(),
                    )));
                }
            }

            for binding in callable.frame_schema().resume_payload_bindings() {
                let frame_field_index =
                    self.validate_resume_payload_binding(callable, frame_layout, binding)?;
                let layout = RefactorResumePayloadBindingLayout::new(
                    callable.step_schema(),
                    *binding,
                    frame_field_index,
                );
                let boundary_key = (callable.step_schema(), binding.boundary_id());
                if bindings_by_boundary.insert(boundary_key, layout).is_some() {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 owner step schema s{} boundary bd{} 的 resumed local/home contract 重复发布",
                        callable.step_schema().as_u32(),
                        binding.boundary_id().as_u32(),
                    )));
                }
                let state_key = (callable.step_schema(), binding.resume_state());
                match bindings_by_state.get(&state_key) {
                    Some(existing)
                        if existing.consumer_local() == layout.consumer_local()
                            && existing.consumer_frame_slot() == layout.consumer_frame_slot()
                            && existing.frame_field_index() == layout.frame_field_index() => {}
                    Some(existing) => {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 owner step schema s{} resume state st{} 的 resumed local/home contract 冲突：已发布 boundary bd{} -> local{} home={:?}，当前 boundary bd{} -> local{} home={:?}",
                            callable.step_schema().as_u32(),
                            binding.resume_state().as_u32(),
                            existing.boundary_id().as_u32(),
                            existing.consumer_local().as_u32(),
                            existing.consumer_frame_slot(),
                            binding.boundary_id().as_u32(),
                            binding.consumer_local().as_u32(),
                            binding.consumer_frame_slot(),
                        )));
                    }
                    None => {
                        bindings_by_state.insert(state_key, layout);
                    }
                }
            }
        }

        Ok((bindings_by_boundary, bindings_by_state))
    }

    fn validate_resume_payload_binding(
        &mut self,
        callable: &LateLoweredCallable,
        frame_layout: &RefactorFrameLayout<'ctx>,
        binding: &LateLoweredResumePayloadBinding,
    ) -> Result<Option<u32>, LlvmEmitError> {
        let boundary = callable
            .boundary_map()
            .boundary(binding.boundary_id())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` 的 resumed local/home contract 引用了不存在的 boundary bd{}",
                    callable.root_fqn(),
                    binding.boundary_id().as_u32(),
                ))
            })?;
        if binding.resume_state() != boundary.resume_state() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home contract resume_state 漂移：published=st{}，boundary=st{}",
                callable.root_fqn(),
                binding.boundary_id().as_u32(),
                binding.resume_state().as_u32(),
                boundary.resume_state().as_u32(),
            )));
        }

        let (expected_local, expected_home_boundary) = match boundary.lowering() {
            Some(LateLoweredBoundaryLowering::Call(lowering)) => {
                (lowering.result_local(), binding.boundary_id())
            }
            Some(LateLoweredBoundaryLowering::Perform(_)) => {
                let (local, _) = Self::published_boundary_result_slot(
                    callable.frame_schema(),
                    binding.boundary_id(),
                )
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` perform boundary bd{} 缺少 BoundaryResult slot，无法校验 resumed local/home contract",
                        callable.root_fqn(),
                        binding.boundary_id().as_u32(),
                    ))
                })?;
                (local, binding.boundary_id())
            }
            Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                (lowering.result_local(), binding.boundary_id())
            }
            Some(LateLoweredBoundaryLowering::RuntimeError(lowering)) => {
                let paired_binding = callable
                    .frame_schema()
                    .resume_payload_binding(lowering.resume_boundary())
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` runtime-error boundary bd{} 的 paired resume boundary bd{} 缺少 resumed local/home contract",
                            callable.root_fqn(),
                            binding.boundary_id().as_u32(),
                            lowering.resume_boundary().as_u32(),
                        ))
                    })?;
                if paired_binding.consumer_local() != binding.consumer_local()
                    || paired_binding.consumer_frame_slot() != binding.consumer_frame_slot()
                {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` runtime-error boundary bd{} 的 resumed local/home contract 与 paired resume boundary bd{} 漂移",
                        callable.root_fqn(),
                        binding.boundary_id().as_u32(),
                        lowering.resume_boundary().as_u32(),
                    )));
                }
                (paired_binding.consumer_local(), lowering.resume_boundary())
            }
            Some(LateLoweredBoundaryLowering::Handle(_)) | None => {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} 不应发布 resumed local/home contract",
                    callable.root_fqn(),
                    binding.boundary_id().as_u32(),
                )));
            }
        };

        if binding.consumer_local() != expected_local {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home contract local 漂移：published=local{}，expected=local{}",
                callable.root_fqn(),
                binding.boundary_id().as_u32(),
                binding.consumer_local().as_u32(),
                expected_local.as_u32(),
            )));
        }

        let boundary_result_slot =
            Self::published_boundary_result_slot(callable.frame_schema(), expected_home_boundary);
        match (boundary_result_slot, binding.consumer_frame_slot()) {
            (Some((slot_local, slot_id)), Some(binding_slot)) => {
                if slot_local != binding.consumer_local() || slot_id != binding_slot {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home slot 漂移：published=slot{}，expected BoundaryResult home=slot{}",
                        callable.root_fqn(),
                        binding.boundary_id().as_u32(),
                        binding_slot.as_u32(),
                        slot_id.as_u32(),
                    )));
                }
            }
            (Some((_slot_local, slot_id)), None) => {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} 已有 BoundaryResult home slot{}，但 resumed local/home contract 未发布 frame home",
                    callable.root_fqn(),
                    binding.boundary_id().as_u32(),
                    slot_id.as_u32(),
                )));
            }
            (None, Some(binding_slot)) => {
                let slot = callable
                    .frame_schema()
                    .slots()
                    .iter()
                    .find(|slot| slot.slot_id() == binding_slot)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home contract 引用了不存在的 frame slot fs{}",
                            callable.root_fqn(),
                            binding.boundary_id().as_u32(),
                            binding_slot.as_u32(),
                        ))
                    })?;
                let Some(slot_local) = Self::frame_slot_local(slot.kind()) else {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home contract 引用了不能承载 local 的 frame slot fs{} kind={:?}",
                        callable.root_fqn(),
                        binding.boundary_id().as_u32(),
                        binding_slot.as_u32(),
                        slot.kind(),
                    )));
                };
                if slot_local != binding.consumer_local() {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home contract frame slot fs{} 绑定到了 local{}，但 published local 为 local{}",
                        callable.root_fqn(),
                        binding.boundary_id().as_u32(),
                        binding_slot.as_u32(),
                        slot_local.as_u32(),
                        binding.consumer_local().as_u32(),
                    )));
                }
            }
            (None, None) => {}
        }

        binding
            .consumer_frame_slot()
            .map(|slot_id| {
                frame_layout
                    .field_index_for_slot(slot_id)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` boundary bd{} 的 resumed local/home contract 引用了 frame slot fs{}，但 frame layout 中缺少对应 field",
                            callable.root_fqn(),
                            binding.boundary_id().as_u32(),
                            slot_id.as_u32(),
                        ))
                    })
            })
            .transpose()
    }

    fn materialize_completion_payload_binding_layouts(
        &mut self,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
        frame_layouts: &BTreeMap<StepSchemaId, RefactorFrameLayout<'ctx>>,
    ) -> Result<CompletionPayloadBindingLayouts<'ctx>, LlvmEmitError> {
        let mut layouts = CompletionPayloadBindingLayouts::new();

        for callable in self.program.callables() {
            let step_type = self.program.step_type(callable.step_schema()).ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{}` step schema s{} 的 Step shell，无法发布 completion payload contract",
                    callable.root_fqn(),
                    callable.step_schema().as_u32(),
                ))
            })?;
            let step_layout = step_layouts.get(&callable.step_schema()).ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{}` step schema s{} 的 Step layout，无法发布 completion payload contract",
                    callable.root_fqn(),
                    callable.step_schema().as_u32(),
                ))
            })?;
            if step_layout.complete_variant().payload_source_ty() != step_type.complete_ty() {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` 的 Step complete variant 类型漂移：layout=t{}，step=t{}",
                    callable.root_fqn(),
                    step_layout.complete_variant().payload_source_ty().as_u32(),
                    step_type.complete_ty().as_u32(),
                )));
            }
            let frame_layout = frame_layouts.get(&callable.step_schema()).ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{}` step schema s{} 的 frame layout，无法发布 completion payload contract",
                    callable.root_fqn(),
                    callable.step_schema().as_u32(),
                ))
            })?;

            for state in callable.state_graph().states() {
                if !matches!(
                    state.terminator(),
                    LateLoweredStateTerminator::Return { .. }
                ) {
                    continue;
                }
                if callable
                    .frame_schema()
                    .completion_payload_binding_for_state(state.state_id())
                    .is_none()
                {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` return state st{} 缺少 completion payload contract",
                        callable.root_fqn(),
                        state.state_id().as_u32(),
                    )));
                }
            }

            for binding in callable.frame_schema().completion_payload_bindings() {
                let (payload_abi, frame_field_index) = self.validate_completion_payload_binding(
                    callable,
                    step_type,
                    frame_layout,
                    binding,
                )?;
                let layout = RefactorCompletionPayloadBindingLayout::new(
                    callable.step_schema(),
                    binding.clone(),
                    payload_abi,
                    frame_field_index,
                );
                let key = (callable.step_schema(), binding.return_state());
                if layouts.insert(key, layout).is_some() {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 owner step schema s{} return state st{} 的 completion payload contract 重复发布",
                        callable.step_schema().as_u32(),
                        binding.return_state().as_u32(),
                    )));
                }
            }
        }

        Ok(layouts)
    }

    fn validate_completion_payload_binding(
        &mut self,
        callable: &LateLoweredCallable,
        step_type: &LateLoweredStepType,
        frame_layout: &RefactorFrameLayout<'ctx>,
        binding: &LateLoweredCompletionPayloadBinding,
    ) -> Result<(RefactorAbiValue<'ctx>, Option<u32>), LlvmEmitError> {
        let state = callable
            .state_graph()
            .states()
            .iter()
            .find(|state| state.state_id() == binding.return_state())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` 的 completion payload contract 引用了不存在的 return state st{}",
                    callable.root_fqn(),
                    binding.return_state().as_u32(),
                ))
            })?;
        let LateLoweredStateTerminator::Return {
            payload_source,
            complete_state,
        } = state.terminator()
        else {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` state st{} 不是 Return，却发布了 completion payload contract",
                callable.root_fqn(),
                binding.return_state().as_u32(),
            )));
        };
        if binding.complete_state() != *complete_state
            || binding.complete_state() != callable.state_graph().complete_state()
        {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` return state st{} 的 complete_state 漂移：binding=st{}，state_graph_return=st{}，callable_complete=st{}",
                callable.root_fqn(),
                binding.return_state().as_u32(),
                binding.complete_state().as_u32(),
                complete_state.as_u32(),
                callable.state_graph().complete_state().as_u32(),
            )));
        }
        if binding.payload_source() != payload_source {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` return state st{} 的 completion payload source 漂移：binding={:?}，state_graph={:?}",
                callable.root_fqn(),
                binding.return_state().as_u32(),
                binding.payload_source(),
                payload_source,
            )));
        }
        if binding.payload_source().source_ty() != step_type.complete_ty() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` return state st{} 的 completion payload source type t{} 与 StepSchema s{} complete_ty t{} 不一致",
                callable.root_fqn(),
                binding.return_state().as_u32(),
                binding.payload_source().source_ty().as_u32(),
                step_type.step_schema().as_u32(),
                step_type.complete_ty().as_u32(),
            )));
        }
        if matches!(
            binding.payload_source(),
            LateLoweredCompletionPayloadSource::Unit { .. }
        ) && !matches!(
            self.source_types.kind(step_type.complete_ty()),
            TypeKind::Value(ValueTypeKind::Unit)
        ) {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` return state st{} 对 non-Unit complete_ty t{} 发布了 Unit completion payload source",
                callable.root_fqn(),
                binding.return_state().as_u32(),
                step_type.complete_ty().as_u32(),
            )));
        }

        let expected_frame_slot = match binding.payload_source() {
            LateLoweredCompletionPayloadSource::Operand(source) => match source.value() {
                LateLoweredOperandValueSource::Local(local) => {
                    Self::published_frame_slot_for_local(callable.frame_schema(), *local)
                }
                LateLoweredOperandValueSource::Const(_) => None,
            },
            LateLoweredCompletionPayloadSource::Unit { .. } => None,
        };
        if binding.payload_frame_slot() != expected_frame_slot {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` return state st{} 的 completion payload frame home 漂移：binding={:?}，expected={:?}",
                callable.root_fqn(),
                binding.return_state().as_u32(),
                binding.payload_frame_slot(),
                expected_frame_slot,
            )));
        }

        let frame_field_index = binding
            .payload_frame_slot()
            .map(|slot_id| {
                let slot = callable
                    .frame_schema()
                    .slots()
                    .iter()
                    .find(|slot| slot.slot_id() == slot_id)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` return state st{} 的 completion payload contract 引用了不存在的 frame slot fs{}",
                            callable.root_fqn(),
                            binding.return_state().as_u32(),
                            slot_id.as_u32(),
                        ))
                    })?;
                if slot.ty() != binding.payload_source().source_ty() {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` return state st{} 的 completion payload home slot fs{} 类型 t{} 与 payload source type t{} 不一致",
                        callable.root_fqn(),
                        binding.return_state().as_u32(),
                        slot_id.as_u32(),
                        slot.ty().as_u32(),
                        binding.payload_source().source_ty().as_u32(),
                    )));
                }
                frame_layout.field_index_for_slot(slot_id).ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` return state st{} 的 completion payload frame slot fs{} 在 frame layout 中缺少对应 field",
                        callable.root_fqn(),
                        binding.return_state().as_u32(),
                        slot_id.as_u32(),
                    ))
                })
            })
            .transpose()?;
        let payload_layout = self.source_value_layout(binding.payload_source().source_ty())?;
        Ok((*payload_layout.abi(), frame_field_index))
    }

    fn published_boundary_result_slot(
        frame_schema: &crate::effect_lowered::ir::LateLoweredFrameSchema,
        boundary_id: BoundaryId,
    ) -> Option<(crate::mir::LocalId, crate::effect_lowered::ir::FrameSlotId)> {
        frame_schema
            .slots()
            .iter()
            .find_map(|slot| match slot.kind() {
                LateLoweredFrameSlotKind::BoundaryResult { boundary, local }
                    if boundary == boundary_id =>
                {
                    Some((local, slot.slot_id()))
                }
                _ => None,
            })
    }

    fn published_frame_slot_for_local(
        frame_schema: &crate::effect_lowered::ir::LateLoweredFrameSchema,
        local: crate::mir::LocalId,
    ) -> Option<crate::effect_lowered::ir::FrameSlotId> {
        frame_schema.slots().iter().find_map(|slot| {
            (Self::frame_slot_local(slot.kind()) == Some(local)).then_some(slot.slot_id())
        })
    }

    fn frame_slot_local(kind: LateLoweredFrameSlotKind) -> Option<crate::mir::LocalId> {
        match kind {
            LateLoweredFrameSlotKind::SourceLocal(local)
            | LateLoweredFrameSlotKind::CompilerTemporary(local)
            | LateLoweredFrameSlotKind::JoinValue { local, .. }
            | LateLoweredFrameSlotKind::BoundaryResult { local, .. }
            | LateLoweredFrameSlotKind::HandleBinder { local, .. } => Some(local),
            LateLoweredFrameSlotKind::HandlePendingPayload { .. }
            | LateLoweredFrameSlotKind::ResumePayload { .. }
            | LateLoweredFrameSlotKind::System(_) => None,
        }
    }

    fn publish_boundary_dynamic_invoke_layouts(
        &mut self,
        callable: &LateLoweredCallable,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
        layouts: &mut BTreeMap<
            (StepSchemaId, crate::mir::SiteId),
            RefactorDynamicInvokeLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        for boundary in callable.boundary_map().entries() {
            let (
                LateLoweredBoundarySource::Site {
                    site_id,
                    kind: BoundarySiteKind::Call,
                },
                Some(LateLoweredBoundaryLowering::Call(lowering)),
            ) = (boundary.source(), boundary.lowering())
            else {
                continue;
            };
            if lowering.facts().target_mode() == CallTargetMode::KnownInstance {
                continue;
            }
            let call_site = self.lookup_materialized_call_site(callable.root_fqn(), site_id)?;

            self.publish_dynamic_invoke_layout(
                callable,
                site_id,
                lowering.facts(),
                &call_site.kind,
                call_site.arg_count,
                step_layouts,
                layouts,
            )?;
        }
        Ok(())
    }

    fn publish_source_slice_dynamic_invoke_layouts(
        &mut self,
        callable: &LateLoweredCallable,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
        layouts: &mut BTreeMap<
            (StepSchemaId, crate::mir::SiteId),
            RefactorDynamicInvokeLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        let boundary_call_sites = callable
            .boundary_map()
            .entries()
            .iter()
            .filter_map(|boundary| match boundary.source() {
                LateLoweredBoundarySource::Site {
                    site_id,
                    kind: BoundarySiteKind::Call,
                } => Some(site_id),
                LateLoweredBoundarySource::RuntimeError { .. }
                | LateLoweredBoundarySource::Site { .. } => None,
            })
            .collect::<BTreeSet<_>>();

        let source_slice_sites = {
            let body = self.lookup_materialized_callable_body(callable.root_fqn())?;
            let body_facts = self.body_effect_facts(callable)?;
            let mut sites = Vec::new();
            for state in callable.state_graph().states() {
                for slice in state.source_slices() {
                    let Some(block) = body.blocks.get(slice.block_id().as_u32() as usize) else {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` 的 source slice 指向缺失的 canonical MIR block bb{}",
                            callable.root_fqn(),
                            slice.block_id().as_u32(),
                        )));
                    };
                    let start = slice.start_statement_index() as usize;
                    let end = slice.end_statement_index() as usize;
                    if start > end || end > block.stmts.len() {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` 的 source slice [{start}..{end}) 越界于 canonical MIR block bb{}（stmt_count={}）",
                            callable.root_fqn(),
                            slice.block_id().as_u32(),
                            block.stmts.len(),
                        )));
                    }
                    for stmt in &block.stmts[start..end] {
                        let MirStatementKind::Assign {
                            value:
                                MirRvalue::Call {
                                    site_id,
                                    kind,
                                    args,
                                    ..
                                },
                            ..
                        } = &stmt.kind
                        else {
                            continue;
                        };
                        if boundary_call_sites.contains(site_id) {
                            continue;
                        }
                        if !matches!(
                            kind,
                            MirCallKind::FunValue { .. }
                                | MirCallKind::Closure { .. }
                                | MirCallKind::Virtual { .. }
                                | MirCallKind::Interface { .. }
                        ) {
                            continue;
                        }

                        let site = body_facts.site(*site_id).ok_or_else(|| {
                            frontend_error(format!(
                                "refactor LLVM ABI materialization 缺少 callable `{}` source-slice call site {} 的 published effect facts，无法发布 non-boundary dynamic-invoke contract",
                                callable.root_fqn(),
                                site_id.as_u32(),
                            ))
                        })?;
                        let SiteEffectFacts::Call(facts) = site else {
                            return Err(frontend_error(format!(
                                "refactor LLVM ABI materialization 发现 callable `{}` source-slice call site {} 的 canonical MIR kind {:?} 不是普通 Call site，而 published facts 为 {site:?}",
                                callable.root_fqn(),
                                site_id.as_u32(),
                                kind,
                            )));
                        };
                        if !facts.resolved_cases().is_empty() {
                            return Err(frontend_error(format!(
                                "refactor LLVM ABI materialization 发现 callable `{}` source-slice dynamic call site {} 仍暴露 outward cases，但 late-lowered handoff 没有对应 call boundary",
                                callable.root_fqn(),
                                site_id.as_u32(),
                            )));
                        }
                        if facts.target_mode() == CallTargetMode::KnownInstance {
                            continue;
                        }

                        sites.push((*site_id, kind.clone(), args.len(), facts.clone()));
                    }
                }
            }
            sites
        };

        for (site_id, kind, arg_count, facts) in source_slice_sites {
            self.publish_dynamic_invoke_layout(
                callable,
                site_id,
                &facts,
                &kind,
                arg_count,
                step_layouts,
                layouts,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn publish_dynamic_invoke_layout(
        &mut self,
        callable: &LateLoweredCallable,
        site_id: crate::mir::SiteId,
        facts: &CallSiteEffectFacts,
        call_kind: &MirCallKind,
        arg_count: usize,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
        layouts: &mut BTreeMap<
            (StepSchemaId, crate::mir::SiteId),
            RefactorDynamicInvokeLayout<'ctx>,
        >,
    ) -> Result<(), LlvmEmitError> {
        let key = (callable.step_schema(), site_id);
        if layouts.contains_key(&key) {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 owner step schema {} call site {} 的 dynamic-invoke contract 重复发布",
                callable.step_schema().as_u32(),
                site_id.as_u32(),
            )));
        }
        let layout = self.materialize_dynamic_invoke_layout(
            callable.root_fqn(),
            callable.step_schema(),
            site_id,
            facts,
            call_kind,
            arg_count,
            step_layouts,
        )?;
        layouts.insert(key, layout);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn materialize_dynamic_invoke_layout(
        &mut self,
        owner_root_fqn: &str,
        owner_step_schema: StepSchemaId,
        site_id: crate::mir::SiteId,
        facts: &CallSiteEffectFacts,
        call_kind: &MirCallKind,
        arg_count: usize,
        step_layouts: &BTreeMap<StepSchemaId, RefactorStepLayout<'ctx>>,
    ) -> Result<RefactorDynamicInvokeLayout<'ctx>, LlvmEmitError> {
        self.validate_dynamic_call_site_kind(owner_root_fqn, site_id, facts, call_kind)?;
        let step_ty = step_layouts
            .get(&facts.callee_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` call site {} dynamic-invoke return step schema {} 的 step layout",
                    site_id.as_u32(),
                    facts.callee_schema().as_u32(),
                ))
            })?
            .llvm_ty();
        let args_layout = self.source_value_layout(facts.invoke_args_tuple_ty())?;
        let args_abi = *args_layout.abi();
        let carrier = match call_kind {
            MirCallKind::FunValue { .. } | MirCallKind::Closure { .. } => {
                if facts.target_mode() != CallTargetMode::DynamicFallback {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` call site {} 的 {:?} lowering 只能绑定 DynamicFallback，但实际 target_mode 为 {:?}",
                        site_id.as_u32(),
                        call_kind,
                        facts.target_mode(),
                    )));
                }
                RefactorDynamicInvokeCarrierLayout::ClosureObject(
                    RefactorClosureCarrierLayout::new(
                        self.codegen.llvm_closure_object_type(),
                        RefactorAbiValue::new(self.codegen.llvm_gc_i8_ptr_type().into(), false),
                        1,
                        2,
                    ),
                )
            }
            MirCallKind::Virtual { dispatch, .. } => {
                let method_slot = self.resolve_virtual_dispatch_slot(
                    owner_root_fqn,
                    site_id,
                    dispatch,
                    arg_count,
                )?;
                if let crate::effect_facts::CallSiteTarget::CandidateSet(targets) = facts.target() {
                    for target in targets {
                        if self.program.callable(&target.template.fqn).is_none() {
                            return Err(frontend_error(format!(
                                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` call site {} CandidateSet target `{}` 的 published callable shell",
                                site_id.as_u32(),
                                target.template.fqn,
                            )));
                        }
                    }
                }
                RefactorDynamicInvokeCarrierLayout::VirtualReceiver(
                    RefactorDispatchReceiverLayout::new(
                        dispatch.receiver_ty,
                        *self.source_value_layout(dispatch.receiver_ty)?.abi(),
                        dispatch.owner_fqn.clone(),
                        dispatch.member_name.clone(),
                        method_slot,
                        None,
                    ),
                )
            }
            MirCallKind::Interface { dispatch, .. } => {
                let (interface_id, method_slot) = self.resolve_interface_dispatch_slot(
                    owner_root_fqn,
                    site_id,
                    dispatch,
                    arg_count,
                )?;
                if let crate::effect_facts::CallSiteTarget::CandidateSet(targets) = facts.target() {
                    for target in targets {
                        if self.program.callable(&target.template.fqn).is_none() {
                            return Err(frontend_error(format!(
                                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` call site {} CandidateSet target `{}` 的 published callable shell",
                                site_id.as_u32(),
                                target.template.fqn,
                            )));
                        }
                    }
                }
                RefactorDynamicInvokeCarrierLayout::InterfaceReceiver(
                    RefactorDispatchReceiverLayout::new(
                        dispatch.receiver_ty,
                        *self.source_value_layout(dispatch.receiver_ty)?.abi(),
                        dispatch.owner_fqn.clone(),
                        dispatch.member_name.clone(),
                        method_slot,
                        Some(interface_id),
                    ),
                )
            }
            other => {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` call site {} 的 canonical MIR kind {other:?} 无法为 {:?} 发布 dynamic-invoke contract",
                    site_id.as_u32(),
                    facts.target_mode(),
                )));
            }
        };

        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> =
            vec![carrier.receiver_abi().llvm_ty().into()];
        if !args_abi.is_elided() {
            params.push(args_abi.llvm_ty().into());
        }
        let llvm_ty = step_ty.fn_type(&params, false);

        Ok(RefactorDynamicInvokeLayout::new(
            owner_step_schema,
            site_id,
            facts.target_mode(),
            facts.invoke_args_tuple_ty(),
            llvm_ty,
            params.len(),
            args_abi,
            facts.callee_schema(),
            carrier,
        ))
    }

    fn materialize_local_runtime_error_contracts(
        &mut self,
    ) -> Result<
        BTreeMap<(StepSchemaId, crate::mir::SiteId), RefactorLocalRuntimeErrorContract<'ctx>>,
        LlvmEmitError,
    > {
        let mut contracts = BTreeMap::new();
        for callable in self.program.callables() {
            for boundary in callable.boundary_map().entries() {
                let (
                    LateLoweredBoundarySource::Site {
                        site_id,
                        kind: BoundarySiteKind::Call,
                    },
                    Some(LateLoweredBoundaryLowering::Call(lowering)),
                ) = (boundary.source(), boundary.lowering())
                else {
                    continue;
                };
                let Some(contract) = lowering.consumed_runtime_error_case() else {
                    continue;
                };
                let Some(target_state) = callable
                    .state_graph()
                    .states()
                    .iter()
                    .find(|state| state.state_id() == contract.target_state())
                else {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 缺少 callable `{}` call site {} local runtime-error target state st{}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.target_state().as_u32(),
                    )));
                };
                let terminal_action = match target_state.terminator() {
                    LateLoweredStateTerminator::LocalRuntimeError {
                        payload_tuple_ty,
                        terminal_action,
                    } if *payload_tuple_ty == contract.payload_tuple_ty()
                        && *terminal_action == contract.terminal_action() =>
                    {
                        *terminal_action
                    }
                    LateLoweredStateTerminator::LocalRuntimeError {
                        payload_tuple_ty,
                        terminal_action,
                    } => {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 local runtime-error target state st{} contract 漂移：state_graph=(payload_tuple_ty=t{}, terminal_action={terminal_action:?})，boundary lowering=(payload_tuple_ty=t{}, terminal_action={:?})",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            contract.target_state().as_u32(),
                            payload_tuple_ty.as_u32(),
                            contract.payload_tuple_ty().as_u32(),
                            contract.terminal_action(),
                        )));
                    }
                    other => {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` call site {} 的 local runtime-error target state st{} 不是 LocalRuntimeError terminator，而是 {other:?}",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            contract.target_state().as_u32(),
                        )));
                    }
                };
                let key = (callable.step_schema(), site_id);
                if contracts.contains_key(&key) {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 owner step schema {} call site {} 的 local runtime-error contract 重复发布",
                        callable.step_schema().as_u32(),
                        site_id.as_u32(),
                    )));
                }
                let payload_layout = self.source_value_layout(contract.payload_tuple_ty())?;
                let payload_abi = *payload_layout.abi();
                let terminal_action = self.materialize_local_runtime_error_terminal_action(
                    terminal_action,
                    payload_abi,
                )?;
                contracts.insert(
                    key,
                    RefactorLocalRuntimeErrorContract::new(
                        callable.step_schema(),
                        site_id,
                        contract.input_case_tag(),
                        contract.payload_tuple_ty(),
                        payload_abi,
                        terminal_action,
                        contract.target_state(),
                    ),
                );
            }
        }
        Ok(contracts)
    }

    fn materialize_handle_dispatch_layouts(
        &mut self,
        frame_layouts: &BTreeMap<StepSchemaId, RefactorFrameLayout<'ctx>>,
        continuation_layouts: &BTreeMap<
            crate::effect_lowered::ir::ContinuationObjectId,
            RefactorContinuationObjectLayout<'ctx>,
        >,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<
        BTreeMap<(StepSchemaId, crate::mir::SiteId), RefactorHandleDispatchLayout>,
        LlvmEmitError,
    > {
        let mut layouts = BTreeMap::new();
        for callable in self.program.callables() {
            let handle_states = callable
                .state_graph()
                .states()
                .iter()
                .filter(|state| {
                    matches!(
                        state.terminator(),
                        LateLoweredStateTerminator::HandleDispatch { .. }
                    )
                })
                .collect::<Vec<_>>();
            if handle_states.is_empty() {
                continue;
            }
            let frame_layout = frame_layouts.get(&callable.step_schema()).ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{}` 的 frame layout，无法发布 HandleDispatch contract",
                    callable.root_fqn(),
                ))
            })?;
            let state_tag_field_index = frame_layout
                .field_index_for_system(SystemSlotKind::StateTag)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` 的 frame layout 缺少 StateTag system field，无法发布 HandleDispatch contract",
                        callable.root_fqn(),
                    ))
                })?;
            let completion_tag_field_index = frame_layout
                .field_index_for_system(SystemSlotKind::CompletionTag)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` 的 frame layout 缺少 CompletionTag system field，无法发布 HandleDispatch contract",
                        callable.root_fqn(),
                    ))
                })?;
            let payload_carrier_field_index = frame_layout
                .field_index_for_system(SystemSlotKind::ResumePayloadCarrier)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` 的 frame layout 缺少 ResumePayloadCarrier system field，无法发布 HandleDispatch contract",
                        callable.root_fqn(),
                    ))
                })?;

            for state in handle_states {
                let LateLoweredStateTerminator::HandleDispatch {
                    site_id,
                    body_state,
                    arm_states,
                    finally_state,
                    exit_state,
                    contract,
                    drop_state,
                    ..
                } = state.terminator()
                else {
                    continue;
                };

                let expected_complete_target = finally_state.unwrap_or(*exit_state);
                if contract.body_complete_target() != expected_complete_target {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 body complete target 漂移：contract=st{}，state_graph=st{}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.body_complete_target().as_u32(),
                        expected_complete_target.as_u32(),
                    )));
                }
                if contract.arm_complete_target() != expected_complete_target {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 arm complete target 漂移：contract=st{}，state_graph=st{}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.arm_complete_target().as_u32(),
                        expected_complete_target.as_u32(),
                    )));
                }
                if contract.finally_complete_target() != finally_state.map(|_| *exit_state) {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 finally complete target 漂移：contract={:?}，state_graph={:?}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.finally_complete_target(),
                        finally_state.map(|_| *exit_state),
                    )));
                }
                if contract.abandon_target() != *drop_state {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 abandon target 漂移：contract={:?}，state_graph={:?}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.abandon_target(),
                        drop_state,
                    )));
                }
                if contract.handled_arms().len() != arm_states.len() {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled-arm 数量({}) 与 state_graph arm 数量({}) 不一致",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.handled_arms().len(),
                        arm_states.len(),
                    )));
                }
                let mut published_arm_ordinals = BTreeSet::new();
                for arm in contract.handled_arms() {
                    let arm_ordinal = arm.arm_ordinal() as usize;
                    let Some(expected_state) = arm_states.get(arm_ordinal) else {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} 引用了越界 arm ordinal {}（state_graph arm 数量={})",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            arm.arm_ordinal(),
                            arm_states.len(),
                        )));
                    };
                    if !published_arm_ordinals.insert(arm.arm_ordinal()) {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 重复发布 arm ordinal {}",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.arm_ordinal(),
                        )));
                    }
                    if arm.arm_state() != *expected_state {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} arm state 漂移：contract=st{}，state_graph=st{}（ordinal={})",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            arm.arm_state().as_u32(),
                            expected_state.as_u32(),
                            arm.arm_ordinal(),
                        )));
                    }
                }
                let published_handled_arms = self.materialize_published_handle_arm_layouts(
                    callable,
                    *site_id,
                    contract,
                    frame_layout,
                    continuation_layouts,
                    surface_resume_layouts,
                )?;

                let expected_outward_cases = collect_handle_contract_total_outward_cases(contract);
                let published_outward_cases = contract
                    .outward_emissions()
                    .iter()
                    .map(|emission| emission.case_tag())
                    .collect::<BTreeSet<_>>();
                if !published_outward_cases.is_subset(&expected_outward_cases) {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 outward emission 包含未在 HandleDispatch contract 中声明的 case：contract={}，emissions={}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        render_case_tags(&expected_outward_cases),
                        render_case_tags(&published_outward_cases),
                    )));
                }

                let expected_pending_outward =
                    collect_handle_contract_pending_outward_cases(contract);
                let published_pending_outward = contract
                    .pending_completions()
                    .iter()
                    .filter_map(|pending| match pending {
                        LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) => {
                            Some(*case_tag)
                        }
                        LateLoweredHandlePendingCompletion::ContinueToExit
                        | LateLoweredHandlePendingCompletion::ReturnFromFunction => None,
                    })
                    .collect::<BTreeSet<_>>();
                if expected_pending_outward != published_pending_outward {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending outward completion 集合漂移：contract={}，pending={}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        render_case_tags(&expected_pending_outward),
                        render_case_tags(&published_pending_outward),
                    )));
                }

                if finally_state.is_some() {
                    for required in [
                        LateLoweredHandlePendingCompletion::ContinueToExit,
                        LateLoweredHandlePendingCompletion::ReturnFromFunction,
                    ] {
                        if !contract.pending_completions().contains(&required) {
                            return Err(frontend_error(format!(
                                "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 缺少 required pending completion {:?}",
                                callable.root_fqn(),
                                site_id.as_u32(),
                                required,
                            )));
                        }
                    }
                } else if !contract.pending_completions().is_empty() {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 没有 finally state，却发布了 pending completion {:?}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        contract.pending_completions(),
                    )));
                }

                let expected_state_regions = build_expected_handle_state_regions(
                    callable.root_fqn(),
                    *site_id,
                    callable.state_graph(),
                    state.state_id(),
                    *body_state,
                    contract,
                    *finally_state,
                    *exit_state,
                )?;
                validate_published_handle_state_regions(
                    callable.root_fqn(),
                    *site_id,
                    contract,
                    &expected_state_regions,
                )?;
                let expected_boundary_routings = build_expected_handle_boundary_routings(
                    callable.root_fqn(),
                    *site_id,
                    contract,
                    &expected_state_regions,
                    callable.boundary_map(),
                )?;
                validate_published_handle_boundary_routings(
                    callable.root_fqn(),
                    *site_id,
                    contract,
                    &expected_boundary_routings,
                )?;

                let mut completion_tags = BTreeMap::new();
                let mut next_completion_tag = 1u32;
                for pending in contract.pending_completions() {
                    if let LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) = pending
                        && contract.outward_emission(*case_tag).is_none()
                    {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending completion c{} 缺少 outward emission",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            case_tag.as_u32(),
                        )));
                    }
                    if completion_tags
                        .insert(*pending, next_completion_tag)
                        .is_some()
                    {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 重复发布 pending completion {:?}",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            pending,
                        )));
                    }
                    next_completion_tag = next_completion_tag.saturating_add(1);
                }
                let pending_payload_transports = self
                    .materialize_published_handle_pending_payload_transports(
                        callable,
                        *site_id,
                        contract,
                        frame_layout,
                    )?;

                let key = (callable.step_schema(), *site_id);
                if layouts.contains_key(&key) {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 owner step schema s{} handle site {} 的 HandleDispatch contract 重复发布",
                        callable.step_schema().as_u32(),
                        site_id.as_u32(),
                    )));
                }
                layouts.insert(
                    key,
                    RefactorHandleDispatchLayout::new(
                        callable.step_schema(),
                        *site_id,
                        contract.clone(),
                        state_tag_field_index,
                        completion_tag_field_index,
                        payload_carrier_field_index,
                        completion_tags,
                        pending_payload_transports,
                        published_handled_arms,
                    ),
                );
            }
        }
        Ok(layouts)
    }

    fn materialize_published_handle_arm_layouts(
        &self,
        callable: &LateLoweredCallable,
        site_id: SiteId,
        contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
        frame_layout: &RefactorFrameLayout<'ctx>,
        continuation_layouts: &BTreeMap<
            crate::effect_lowered::ir::ContinuationObjectId,
            RefactorContinuationObjectLayout<'ctx>,
        >,
        surface_resume_layouts: &BTreeMap<
            ContinuationSchemaId,
            RefactorContinuationSurfaceResumeLayout<'ctx>,
        >,
    ) -> Result<Vec<RefactorHandleArmLayout>, LlvmEmitError> {
        let materialized_arms =
            self.lookup_materialized_handle_arms(callable.root_fqn(), site_id)?;
        if materialized_arms.len() != contract.handled_arms().len() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 canonical MIR arm 数量({}) 与 published HandleDispatch arm 数量({}) 不一致",
                callable.root_fqn(),
                site_id.as_u32(),
                materialized_arms.len(),
                contract.handled_arms().len(),
            )));
        }

        let mut layouts = Vec::with_capacity(contract.handled_arms().len());
        for arm in contract.handled_arms() {
            let materialized_arm = materialized_arms
                .get(arm.arm_ordinal() as usize)
                .ok_or_else(|| frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} 引用了不存在的 canonical MIR arm ordinal {}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    arm.handled_case().as_u32(),
                    arm.arm_ordinal(),
                )))?;
            if materialized_arm.payload_tuple_ty != Some(arm.payload_tuple_ty()) {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} payload tuple ty 漂移：contract=t{}，canonical_mir={:?}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    arm.handled_case().as_u32(),
                    arm.payload_tuple_ty().as_u32(),
                    materialized_arm.payload_tuple_ty,
                )));
            }
            if materialized_arm.binder_count != arm.payload_binders().len() {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} payload binder 数量漂移：contract={}，canonical_mir={}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    arm.handled_case().as_u32(),
                    arm.payload_binders().len(),
                    materialized_arm.binder_count,
                )));
            }

            let mut payload_binders = Vec::with_capacity(arm.payload_binders().len());
            for (expected_ordinal, binder) in arm.payload_binders().iter().enumerate() {
                if binder.ordinal() != expected_ordinal as u32 {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} payload binder ordinal 漂移：contract=#{}，expected=#{}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        arm.handled_case().as_u32(),
                        binder.ordinal(),
                        expected_ordinal,
                    )));
                }
                let expected_local = materialized_arm
                    .binder_locals
                    .get(expected_ordinal)
                    .copied()
                    .ok_or_else(|| frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} 缺少 canonical MIR payload binder #{}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        arm.handled_case().as_u32(),
                        expected_ordinal,
                    )))?;
                if binder.local() != expected_local {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} payload binder #{} local 漂移：contract=local{}，canonical_mir=local{}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        arm.handled_case().as_u32(),
                        expected_ordinal,
                        binder.local().as_u32(),
                        expected_local.as_u32(),
                    )));
                }
                let frame_field_index = match binder.frame_slot() {
                    Some(frame_slot) => Some(frame_layout.field_index_for_slot(frame_slot).ok_or_else(
                        || frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} payload binder #{} 引用了 frame slot fs{}，但 frame layout 中缺少对应 field",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            expected_ordinal,
                            frame_slot.as_u32(),
                        )),
                    )?),
                    None => None,
                };
                payload_binders.push(RefactorHandlePayloadBinderLayout::new(
                    binder.ordinal(),
                    binder.local(),
                    binder.frame_slot(),
                    frame_field_index,
                ));
            }

            let continuation_binder = match (
                materialized_arm.continuation_local,
                arm.continuation_binder(),
            ) {
                (Some(expected_local), Some(binder)) => {
                    if binder.local() != expected_local {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} continuation binder local 漂移：contract=local{}，canonical_mir=local{}",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            binder.local().as_u32(),
                            expected_local.as_u32(),
                        )));
                    }
                    if binder.continuation_object() != callable.continuation_object() {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} continuation object 漂移：contract=ko{}，owner=ko{}",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            binder.continuation_object().as_u32(),
                            callable.continuation_object().as_u32(),
                        )));
                    }
                    let continuation_layout = continuation_layouts.get(&binder.continuation_object()).ok_or_else(
                        || frontend_error(format!(
                            "refactor LLVM ABI materialization 缺少 callable `{}` handle site {} 的 handled case c{} continuation object ko{} layout",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            binder.continuation_object().as_u32(),
                        )),
                    )?;
                    let surface_layout = surface_resume_layouts
                        .get(&binder.continuation_schema())
                        .ok_or_else(|| frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} continuation binder 缺少 continuation schema k{} 的 authoritative surface-resume dispatch inventory",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            binder.continuation_schema().as_u32(),
                        )))?;
                    if matches!(
                        surface_layout.dispatch_source_kind(),
                        crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ResumeBoundaryOnly
                            | crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::Unreachable
                    ) {
                        return Err(frontend_error(format!(
                            "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} continuation schema k{} dispatch source kind 为 {:?}，无法作为 authoritative handle-binder surface source",
                            callable.root_fqn(),
                            site_id.as_u32(),
                            arm.handled_case().as_u32(),
                            binder.continuation_schema().as_u32(),
                            surface_layout.dispatch_source_kind(),
                        )));
                    }
                    let _ = continuation_layout;
                    let frame_field_index = match binder.frame_slot() {
                        Some(frame_slot) => Some(frame_layout.field_index_for_slot(frame_slot).ok_or_else(
                            || frontend_error(format!(
                                "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} continuation binder 引用了 frame slot fs{}，但 frame layout 中缺少对应 field",
                                callable.root_fqn(),
                                site_id.as_u32(),
                                arm.handled_case().as_u32(),
                                frame_slot.as_u32(),
                            )),
                        )?),
                        None => None,
                    };
                    Some(RefactorHandleContinuationBinderLayout::new(
                        binder.local(),
                        binder.frame_slot(),
                        frame_field_index,
                        binder.continuation_schema(),
                        binder.continuation_object(),
                        surface_layout.dispatch_source_kind(),
                        surface_layout.return_step_schema(),
                    ))
                }
                (Some(_), None) => {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} 缺少 published continuation binder contract",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        arm.handled_case().as_u32(),
                    )));
                }
                (None, Some(_)) => {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 handled case c{} 不存在 canonical MIR continuation binder，却发布了 continuation binder contract",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        arm.handled_case().as_u32(),
                    )));
                }
                (None, None) => None,
            };

            layouts.push(RefactorHandleArmLayout::new(
                arm.handled_case(),
                arm.arm_state(),
                arm.arm_ordinal(),
                arm.payload_tuple_ty(),
                payload_binders,
                continuation_binder,
                arm.arm_outward_cases().to_vec(),
            ));
        }

        Ok(layouts)
    }

    fn materialize_published_handle_pending_payload_transports(
        &self,
        callable: &LateLoweredCallable,
        site_id: SiteId,
        contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
        frame_layout: &RefactorFrameLayout<'ctx>,
    ) -> Result<
        BTreeMap<LateLoweredHandlePendingCompletion, RefactorHandlePendingPayloadTransportLayout>,
        LlvmEmitError,
    > {
        let expected_pending_cases = collect_handle_contract_pending_outward_cases(contract);
        let mut published_pending_cases = BTreeSet::new();
        let mut layouts = BTreeMap::new();

        for transport in contract.pending_payload_transports() {
            let completion = transport.completion();
            let case_tag = match completion {
                LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) => case_tag,
                LateLoweredHandlePendingCompletion::ContinueToExit
                | LateLoweredHandlePendingCompletion::ReturnFromFunction => {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 为 {:?} 发布了 pending payload transport；只有 PropagateOutward(case) 才允许发布 typed payload transport",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        completion,
                    )));
                }
            };
            if !contract.pending_completions().contains(&completion) {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport {:?} 没有对应的 pending completion",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    completion,
                )));
            }
            let emission = contract.outward_emission(case_tag).ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport c{} 缺少 outward emission",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    case_tag.as_u32(),
                ))
            })?;
            if emission.payload_tuple_ty() != transport.payload_tuple_ty() {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport c{} payload tuple ty 漂移：transport=t{}，outward emission=t{}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    case_tag.as_u32(),
                    transport.payload_tuple_ty().as_u32(),
                    emission.payload_tuple_ty().as_u32(),
                )));
            }
            let slot = callable
                .frame_schema()
                .slots()
                .iter()
                .find(|slot| slot.slot_id() == transport.frame_slot())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport c{} 引用了不存在的 frame slot fs{}",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        case_tag.as_u32(),
                        transport.frame_slot().as_u32(),
                    ))
            })?;
            if slot.kind() != (LateLoweredFrameSlotKind::HandlePendingPayload { site_id, case_tag })
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport c{} 引用的 frame slot fs{} kind 漂移：published={:?}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    case_tag.as_u32(),
                    transport.frame_slot().as_u32(),
                    slot.kind(),
                )));
            }
            if slot.ty() != transport.payload_tuple_ty() {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport c{} frame slot fs{} 类型漂移：slot=t{}，transport=t{}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    case_tag.as_u32(),
                    transport.frame_slot().as_u32(),
                    slot.ty().as_u32(),
                    transport.payload_tuple_ty().as_u32(),
                )));
            }
            let frame_field_index = frame_layout
                .field_index_for_slot(transport.frame_slot())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport c{} 引用了 frame slot fs{}，但 frame layout 中缺少对应 field",
                        callable.root_fqn(),
                        site_id.as_u32(),
                        case_tag.as_u32(),
                        transport.frame_slot().as_u32(),
                    ))
                })?;
            if layouts
                .insert(
                    completion,
                    RefactorHandlePendingPayloadTransportLayout::new(*transport, frame_field_index),
                )
                .is_some()
            {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 重复发布 pending payload transport {:?}",
                    callable.root_fqn(),
                    site_id.as_u32(),
                    completion,
                )));
            }
            published_pending_cases.insert(case_tag);
        }

        if published_pending_cases != expected_pending_cases {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{}` handle site {} 的 pending payload transport 集合漂移：published={}，expected={}",
                callable.root_fqn(),
                site_id.as_u32(),
                render_case_tags(&published_pending_cases),
                render_case_tags(&expected_pending_cases),
            )));
        }

        Ok(layouts)
    }

    fn materialize_local_runtime_error_terminal_action(
        &mut self,
        action: LateLoweredLocalRuntimeErrorTerminalAction,
        payload_abi: RefactorAbiValue<'ctx>,
    ) -> Result<RefactorLocalRuntimeErrorTerminalAction<'ctx>, LlvmEmitError> {
        match action {
            LateLoweredLocalRuntimeErrorTerminalAction::RuntimeFatal { runtime_entry } => {
                Ok(RefactorLocalRuntimeErrorTerminalAction::RuntimeFatal {
                    runtime_entry: self
                        .materialize_published_runtime_entry(runtime_entry, payload_abi)?,
                })
            }
        }
    }

    fn materialize_published_runtime_entry(
        &mut self,
        runtime_entry: LateLoweredPublishedRuntimeEntry,
        payload_abi: RefactorAbiValue<'ctx>,
    ) -> Result<RefactorPublishedRuntimeEntryLayout<'ctx>, LlvmEmitError> {
        match runtime_entry {
            LateLoweredPublishedRuntimeEntry::RuntimeErrorFatal => {
                if payload_abi.is_elided() {
                    return Err(frontend_error(
                        "refactor LLVM ABI materialization 不允许把 local runtime-error payload 退化成零载荷 runtime fatal contract"
                            .to_string(),
                    ));
                }
                self.codegen.declare_runtime_error_fatal();
                let params: [BasicMetadataTypeEnum<'ctx>; 1] = [payload_abi.llvm_ty().into()];
                let llvm_ty = self.codegen.context.void_type().fn_type(&params, false);
                Ok(RefactorPublishedRuntimeEntryLayout::new(
                    runtime_entry,
                    runtime_entry.symbol_name().to_string(),
                    llvm_ty,
                    params.len(),
                ))
            }
        }
    }

    fn body_effect_facts(
        &self,
        callable: &LateLoweredCallable,
    ) -> Result<&crate::effect_facts::BodyEffectFacts, LlvmEmitError> {
        self.effect_facts
            .body(callable.instance_key())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{}` 的 BodyEffectFacts，无法发布 source-slice dynamic-invoke contract",
                    callable.root_fqn(),
                ))
            })
    }

    fn validate_dynamic_call_site_kind(
        &self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
        facts: &CallSiteEffectFacts,
        call_kind: &MirCallKind,
    ) -> Result<(), LlvmEmitError> {
        let expected_kind = match call_kind {
            MirCallKind::Closure { .. } => CallSiteKind::Closure,
            MirCallKind::FunValue { .. } => CallSiteKind::FunValue,
            MirCallKind::Virtual { .. } => CallSiteKind::Virtual,
            MirCallKind::Interface { .. } => CallSiteKind::Interface,
            other => {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` call site {} 的 canonical MIR kind {other:?} 无法为 {:?} 发布 dynamic-invoke contract",
                    site_id.as_u32(),
                    facts.target_mode(),
                )));
            }
        };
        if facts.kind() != expected_kind {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` call site {} 的 call kind contract 漂移：canonical MIR={call_kind:?}，effect facts={:?}",
                site_id.as_u32(),
                facts.kind(),
            )));
        }
        Ok(())
    }

    fn lookup_materialized_callable_body(
        &self,
        owner_root_fqn: &str,
    ) -> Result<&crate::mir::Body, LlvmEmitError> {
        let callable = self.pass_view.callable(owner_root_fqn).ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` 的 canonical MIR body，无法发布 dynamic-invoke contract"
            ))
        })?;
        callable.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` 的 canonical MIR body 内容，无法发布 dynamic-invoke contract"
            ))
        })
    }

    fn lookup_materialized_call_site(
        &self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
    ) -> Result<MaterializedDynamicCallSite, LlvmEmitError> {
        let body = self.lookup_materialized_callable_body(owner_root_fqn)?;
        for block in &body.blocks {
            for stmt in &block.stmts {
                let MirStatementKind::Assign {
                    value:
                        MirRvalue::Call {
                            site_id: stmt_site_id,
                            kind,
                            args,
                            ..
                        },
                    ..
                } = &stmt.kind
                else {
                    continue;
                };
                if *stmt_site_id == site_id {
                    return Ok(MaterializedDynamicCallSite {
                        kind: kind.clone(),
                        arg_count: args.len(),
                    });
                }
            }
        }
        Err(frontend_error(format!(
            "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` call site {} 的 canonical MIR call metadata，无法发布 dynamic-invoke contract",
            site_id.as_u32(),
        )))
    }

    fn resolve_virtual_dispatch_slot(
        &self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
        dispatch: &crate::mir::DispatchMetadata,
        explicit_arg_count: usize,
    ) -> Result<u32, LlvmEmitError> {
        let slots = self
            .codegen
            .class_vtables
            .get(&dispatch.owner_fqn)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` virtual call site {} owner `{}` 的 class vtable，无法发布 dispatch slot",
                    site_id.as_u32(),
                    dispatch.owner_fqn,
                ))
            })?;
        let mut candidates = slots.iter().filter(|slot| {
            slot.name == dispatch.member_name && slot.params_len == explicit_arg_count as u32
        });
        let first = candidates.next().ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` virtual call site {} `{}`.`{}`/{} 的 vtable slot",
                site_id.as_u32(),
                dispatch.owner_fqn,
                dispatch.member_name,
                explicit_arg_count,
            ))
        })?;
        if candidates.next().is_some() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` virtual call site {} `{}`.`{}`/{} 的 vtable slot 多义",
                site_id.as_u32(),
                dispatch.owner_fqn,
                dispatch.member_name,
                explicit_arg_count,
            )));
        }
        Ok(first.slot)
    }

    fn resolve_interface_dispatch_slot(
        &self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
        dispatch: &crate::mir::DispatchMetadata,
        explicit_arg_count: usize,
    ) -> Result<(u64, u32), LlvmEmitError> {
        let iface = self
            .codegen
            .interfaces
            .get(&dispatch.owner_fqn)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` interface call site {} owner `{}` 的 interface metadata，无法发布 itable slot",
                    site_id.as_u32(),
                    dispatch.owner_fqn,
                ))
            })?;
        let mut candidates = iface.method_slots.iter().filter(|slot| {
            slot.name == dispatch.member_name && slot.params_len == explicit_arg_count as u32
        });
        let first = candidates.next().ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` interface call site {} `{}`.`{}`/{} 的 itable slot",
                site_id.as_u32(),
                dispatch.owner_fqn,
                dispatch.member_name,
                explicit_arg_count,
            ))
        })?;
        if candidates.next().is_some() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` interface call site {} `{}`.`{}`/{} 的 itable slot 多义",
                site_id.as_u32(),
                dispatch.owner_fqn,
                dispatch.member_name,
                explicit_arg_count,
            )));
        }
        Ok((iface.interface_id, first.slot))
    }

    fn lookup_materialized_handle_arms(
        &self,
        owner_root_fqn: &str,
        site_id: crate::mir::SiteId,
    ) -> Result<&[MirHandlerArm], LlvmEmitError> {
        let body = self.lookup_materialized_callable_body(owner_root_fqn)?;
        let mut found = None;
        for block in &body.blocks {
            let MirTerminatorKind::Handle {
                site_id: terminator_site,
                arms,
                ..
            } = &block.terminator.kind
            else {
                continue;
            };
            if *terminator_site != site_id {
                continue;
            }
            if found.replace(arms.as_slice()).is_some() {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 在 canonical MIR 中重复出现多个 Handle terminator",
                    site_id.as_u32(),
                )));
            }
        }
        found.ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 callable `{owner_root_fqn}` handle site {} 的 canonical MIR arm metadata，无法校验 HandleDispatch arm binder contract",
                site_id.as_u32(),
            ))
        })
    }

    fn validate_published_resume_packing_ids(
        &self,
        owner_label: &str,
        expected_step_schema: StepSchemaId,
        interface_ids: &[ResumeInterfaceId],
    ) -> Result<(), LlvmEmitError> {
        let mut seen = BTreeSet::new();
        for &interface_id in interface_ids {
            if !seen.insert(interface_id) {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 {owner_label} 重复发布 resume packing {}",
                    interface_id.as_u32()
                )));
            }
            let interface = self.program.resume_packing(interface_id).ok_or_else(|| {
                frontend_error(format!(
                    "refactor LLVM ABI materialization 缺少 {owner_label} 发布的 resume packing {}",
                    interface_id.as_u32()
                ))
            })?;
            if interface.return_step_schema() != expected_step_schema {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 {owner_label} 发布的 resume packing {} return step schema 为 {}，但当前 step schema 为 {}",
                    interface_id.as_u32(),
                    interface.return_step_schema().as_u32(),
                    expected_step_schema.as_u32()
                )));
            }
        }
        Ok(())
    }

    fn abi_value(&mut self, ty: TypeId) -> Result<RefactorAbiValue<'ctx>, LlvmEmitError> {
        self.abi_value_from_types(self.source_types, ty)
    }

    fn source_value_layout(
        &mut self,
        ty: TypeId,
    ) -> Result<RefactorSourceAbiLayout<'ctx>, LlvmEmitError> {
        if let Some(layout) = self.source_value_layouts.get(&ty) {
            return Ok(layout.clone());
        }

        let source_kind = self.source_types.kind(ty).clone();
        let layout = match source_kind {
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                let abi = self
                    .abi_value_from_types(self.source_types, ty)
                    .map_err(|err| self.wrap_source_value_layout_error(ty, err))?;
                let mut next_abi_field_index = 0u32;
                let mut fields = Vec::with_capacity(elements.len());
                for (source_index, element_ty) in elements.into_iter().enumerate() {
                    let element_layout = self.source_value_layout(element_ty)?;
                    let abi_field_index = if element_layout.abi().is_elided() {
                        None
                    } else {
                        let field_index = next_abi_field_index;
                        next_abi_field_index = next_abi_field_index.saturating_add(1);
                        Some(field_index)
                    };
                    fields.push(RefactorSourceAbiFieldLayout::new(
                        source_index as u32,
                        element_ty,
                        abi_field_index,
                        *element_layout.abi(),
                    ));
                }
                RefactorSourceAbiLayout::new(ty, RefactorSourceAbiLayoutKind::Tuple, abi, fields)
            }
            _ => {
                let abi = self
                    .abi_value_from_types(self.source_types, ty)
                    .map_err(|err| self.wrap_source_value_layout_error(ty, err))?;
                RefactorSourceAbiLayout::new(
                    ty,
                    RefactorSourceAbiLayoutKind::Scalar,
                    abi,
                    Vec::new(),
                )
            }
        };
        self.source_value_layouts.insert(ty, layout.clone());
        Ok(layout)
    }

    fn wrap_source_value_layout_error(&self, ty: TypeId, err: LlvmEmitError) -> LlvmEmitError {
        match err {
            LlvmEmitError::Frontend { message } => frontend_error(format!(
                "refactor LLVM source-type ABI value lowering 无法为 `{}`（t{}）建立 authoritative contract: {message}",
                self.source_types.display(ty),
                ty.as_u32()
            )),
            other => other,
        }
    }

    fn abi_value_from_types(
        &mut self,
        types: &TypeStore,
        ty: TypeId,
    ) -> Result<RefactorAbiValue<'ctx>, LlvmEmitError> {
        let llvm_ty = self.llvm_abi_type_of_types(types, ty)?;
        let elided = self.codegen.target_data.get_store_size(&llvm_ty) == 0;
        Ok(RefactorAbiValue::new(llvm_ty, elided))
    }

    fn llvm_abi_type_of_types(
        &mut self,
        types: &TypeStore,
        ty: TypeId,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        match types.kind(ty) {
            TypeKind::Ref(RefTypeKind::String) => {
                Ok(self.codegen.llvm_scoop_string_ptr_type().into())
            }
            TypeKind::Ref(_) => Ok(self.codegen.llvm_gc_i8_ptr_type().into()),
            TypeKind::StarProjection(star) => self.llvm_abi_type_of_types(types, star.read_ty),
            TypeKind::Value(ValueTypeKind::Nothing) => Ok(self.codegen.context.i8_type().into()),
            TypeKind::Value(ValueTypeKind::Unit) => {
                Ok(self.codegen.context.struct_type(&[], false).into())
            }
            TypeKind::Value(ValueTypeKind::Bool) => Ok(self.codegen.context.bool_type().into()),
            TypeKind::Value(ValueTypeKind::Char) => Ok(self.codegen.context.i32_type().into()),
            TypeKind::Value(ValueTypeKind::Float64) => Ok(self.codegen.context.f64_type().into()),
            TypeKind::Value(ValueTypeKind::Float32) => Ok(self.codegen.context.f32_type().into()),
            TypeKind::Value(ValueTypeKind::Int) => Ok(self
                .codegen
                .int_type(IntTy {
                    bits: self.codegen.host.word_bit_width(),
                    signed: true,
                })
                .into()),
            TypeKind::Value(ValueTypeKind::UInt) => Ok(self
                .codegen
                .int_type(IntTy {
                    bits: self.codegen.host.word_bit_width(),
                    signed: false,
                })
                .into()),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => Ok(self
                .codegen
                .int_type(IntTy {
                    bits: u32::from(*bits),
                    signed: true,
                })
                .into()),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => Ok(self
                .codegen
                .int_type(IntTy {
                    bits: u32::from(*bits),
                    signed: false,
                })
                .into()),
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                let mut fields = Vec::with_capacity(elements.len());
                for element in elements {
                    let element_ty = self.llvm_abi_type_of_types(types, *element)?;
                    if self.codegen.target_data.get_store_size(&element_ty) == 0 {
                        continue;
                    }
                    fields.push(element_ty);
                }
                Ok(self.codegen.context.struct_type(&fields, false).into())
            }
            TypeKind::Value(ValueTypeKind::Option(inner)) => {
                if let Some(codegen_ty) = self.equivalent_codegen_type_id_from_types(types, ty) {
                    let cg_ty = self.codegen.cg_ty_of(codegen_ty).ok_or_else(|| {
                        frontend_error(format!(
                            "refactor LLVM ABI materialization 无法为 `{}` 恢复 codegen 类型",
                            types.display(ty)
                        ))
                    })?;
                    return self.codegen.llvm_basic_type_of(dummy_span(), cg_ty);
                }
                let key = crate::hir::mangle_nominal_fqn("scoop.core.Option", &[*inner], types);
                let layout = self.codegen.enum_layouts.get(&key).ok_or_else(|| {
                    frontend_error(format!(
                        "refactor LLVM ABI materialization 缺少 `{}` 的 enum layout",
                        types.display(ty)
                    ))
                })?;
                self.llvm_enum_value_type_from_layout(layout)
            }
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                if nominal.fqn == "scoop.unsafe.__AtomicInt" {
                    return Ok(self
                        .codegen
                        .int_type(IntTy {
                            bits: self.codegen.host.word_bit_width(),
                            signed: true,
                        })
                        .into());
                }
                if nominal.fqn == "scoop.core.UIntPtr" || nominal.fqn == "scoop.unsafe.FunPtr" {
                    return Ok(self
                        .codegen
                        .int_type(IntTy {
                            bits: self.codegen.host.word_bit_width(),
                            signed: false,
                        })
                        .into());
                }
                if let Some(codegen_ty) = self.equivalent_codegen_type_id_from_types(types, ty) {
                    let cg_ty = self.codegen.cg_ty_of(codegen_ty).ok_or_else(|| {
                        frontend_error(format!(
                            "refactor LLVM ABI materialization 无法为 `{}` 恢复 codegen 类型",
                            types.display(ty)
                        ))
                    })?;
                    return self.codegen.llvm_basic_type_of(dummy_span(), cg_ty);
                }
                self.llvm_nominal_value_type_from_layout(nominal)
            }
            TypeKind::Param(_) => Err(frontend_error(format!(
                "refactor LLVM ABI materialization 遇到尚未实例化的类型参数 `{}`",
                types.display(ty)
            ))),
        }
    }

    fn llvm_nominal_value_type_from_layout(
        &mut self,
        nominal: &crate::ty::NominalType,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        let key = crate::hir::mangle_nominal_fqn(&nominal.fqn, &nominal.args, self.source_types);
        let layout = self.codegen.enum_layouts.get(&key).ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 缺少 nominal value `{}` 的等价 codegen TypeId 或 enum layout",
                nominal.fqn
            ))
        })?;
        self.llvm_enum_value_type_from_layout(layout)
    }

    fn llvm_enum_value_type_from_layout(
        &self,
        layout: &crate::hir::EnumLayout,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        match &layout.repr {
            crate::hir::EnumRepr::TaggedUnion => {
                if let Some(existing) = self.codegen.context.get_struct_type(&layout.fqn) {
                    return Ok(existing.into());
                }
                let enum_ty = self.codegen.context.opaque_struct_type(&layout.fqn);
                let tag_ty = self.codegen.context.i32_type();
                let payload_word_ty = self.codegen.int_type(IntTy {
                    bits: self.codegen.host.word_bit_width(),
                    signed: false,
                });
                let payload_ptr_ty = self.codegen.llvm_gc_i8_ptr_type();
                enum_ty.set_body(
                    &[tag_ty.into(), payload_word_ty.into(), payload_ptr_ty.into()],
                    false,
                );
                Ok(enum_ty.into())
            }
            crate::hir::EnumRepr::ValueOnly { underlying_ty_fqn } => {
                self.llvm_builtin_integer_from_fqn(underlying_ty_fqn.as_deref())
            }
        }
    }

    fn llvm_builtin_integer_from_fqn(
        &self,
        underlying_ty_fqn: Option<&str>,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        let fqn = underlying_ty_fqn.ok_or_else(|| {
            frontend_error(
                "refactor LLVM ABI materialization 缺少 value-only enum 的底层整数类型".to_string(),
            )
        })?;
        let int_ty = match fqn {
            "scoop.core.Int" | "scoop.unsafe.__AtomicInt" => IntTy {
                bits: self.codegen.host.word_bit_width(),
                signed: true,
            },
            "scoop.core.UInt" | "scoop.core.UIntPtr" => IntTy {
                bits: self.codegen.host.word_bit_width(),
                signed: false,
            },
            other => {
                if let Some(bits) = other
                    .strip_prefix("scoop.core.Int")
                    .and_then(|suffix| suffix.parse::<u32>().ok())
                {
                    IntTy { bits, signed: true }
                } else if let Some(bits) = other
                    .strip_prefix("scoop.core.UInt")
                    .and_then(|suffix| suffix.parse::<u32>().ok())
                {
                    IntTy {
                        bits,
                        signed: false,
                    }
                } else {
                    return Err(frontend_error(format!(
                        "refactor LLVM ABI materialization 目前只支持 integer-backed value-only enum，实际底层类型为 `{other}`"
                    )));
                }
            }
        };
        Ok(self.codegen.int_type(int_ty).into())
    }

    fn equivalent_codegen_type_id_from_types(
        &self,
        types: &TypeStore,
        source_ty: TypeId,
    ) -> Option<TypeId> {
        let source_display = types.display(source_ty).to_string();
        self.codegen
            .types
            .iter_ids()
            .find(|&candidate| self.codegen.types.display(candidate).to_string() == source_display)
    }

    fn define_named_struct(&self, name: &str, fields: &[BasicTypeEnum<'ctx>]) -> StructType<'ctx> {
        let struct_ty = self
            .codegen
            .context
            .get_struct_type(name)
            .unwrap_or_else(|| self.codegen.context.opaque_struct_type(name));
        if struct_ty.is_opaque() {
            struct_ty.set_body(fields, false);
        }
        struct_ty
    }

    fn define_union_storage_type(
        &self,
        name: &str,
        payload_tys: &[StructType<'ctx>],
    ) -> StructType<'ctx> {
        let storage_ty = self
            .codegen
            .context
            .get_struct_type(name)
            .unwrap_or_else(|| self.codegen.context.opaque_struct_type(name));
        if !storage_ty.is_opaque() {
            return storage_ty;
        }

        let mut max_size = 0u64;
        let mut max_align = 1u64;
        let mut anchor_ty = None;
        for payload_ty in payload_tys {
            let size = self.codegen.target_data.get_store_size(payload_ty);
            let align = u64::from(self.codegen.target_data.get_abi_alignment(payload_ty));
            if anchor_ty.is_none() || align > max_align || (align == max_align && size > max_size) {
                anchor_ty = Some(*payload_ty);
                max_size = size;
                max_align = align;
            } else if size > max_size {
                max_size = size;
            }
        }

        if max_size == 0 {
            storage_ty.set_body(&[], false);
            return storage_ty;
        }

        let _anchor_ty = anchor_ty.expect("payload_tys 至少包含 Complete variant");
        let unit_size = if max_align > 8 {
            16
        } else if max_align > 4 {
            8
        } else if max_align > 2 {
            4
        } else if max_align > 1 {
            2
        } else {
            1
        };
        let unit_count = max_size.div_ceil(unit_size) as u32;
        let storage_field: BasicTypeEnum<'ctx> = match unit_size {
            16 => self
                .codegen
                .context
                .i128_type()
                .array_type(unit_count)
                .into(),
            8 => self
                .codegen
                .context
                .i64_type()
                .array_type(unit_count)
                .into(),
            4 => self
                .codegen
                .context
                .i32_type()
                .array_type(unit_count)
                .into(),
            2 => self
                .codegen
                .context
                .i16_type()
                .array_type(unit_count)
                .into(),
            _ => self.codegen.context.i8_type().array_type(unit_count).into(),
        };
        let fields: Vec<BasicTypeEnum<'ctx>> = vec![storage_field];
        storage_ty.set_body(&fields, false);
        storage_ty
    }

    fn ensure_declared_function(&self, name: &str, fn_ty: inkwell::types::FunctionType<'ctx>) {
        if self.codegen.module.get_function(name).is_none() {
            self.codegen.module.add_function(name, fn_ty, None);
        }
    }

    fn ensure_struct_anchor(&self, name: &str, struct_ty: StructType<'ctx>) {
        if self.codegen.module.get_global(name).is_some() {
            return;
        }
        let global = self.codegen.module.add_global(struct_ty, None, name);
        global.set_linkage(Linkage::Internal);
        global.set_constant(true);
        global.set_initializer(&struct_ty.const_zero());
    }

    fn ensure_case_tag_constant(&self, name: &str, tag_value: u32) {
        if self.codegen.module.get_global(name).is_some() {
            return;
        }
        let i32_ty = self.codegen.context.i32_type();
        let global = self.codegen.module.add_global(i32_ty, None, name);
        global.set_linkage(Linkage::Internal);
        global.set_constant(true);
        global.set_initializer(&i32_ty.const_int(u64::from(tag_value), false));
    }
}

fn dummy_span() -> crate::span::Span {
    crate::span::Span::new(0, 0)
}

fn boundary_source_consumption(
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
) -> Option<LateLoweredBoundarySourceConsumption> {
    match boundary.lowering()? {
        LateLoweredBoundaryLowering::Call(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::Perform(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::Resume(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::RuntimeError(_) | LateLoweredBoundaryLowering::Handle(_) => {
            None
        }
    }
}

fn collect_handle_contract_total_outward_cases(
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
) -> BTreeSet<crate::effect_facts::CaseTag> {
    let mut tags = contract
        .body_outward_cases()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    for arm in contract.handled_arms() {
        tags.extend(arm.arm_outward_cases().iter().copied());
    }
    tags.extend(contract.finally_outward_cases().iter().copied());
    tags
}

fn collect_handle_contract_pending_outward_cases(
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
) -> BTreeSet<crate::effect_facts::CaseTag> {
    let emitted_cases = contract
        .outward_emissions()
        .iter()
        .map(|emission| emission.case_tag())
        .collect::<BTreeSet<_>>();
    let mut tags = contract
        .body_outward_cases()
        .iter()
        .copied()
        .filter(|case_tag| emitted_cases.contains(case_tag))
        .collect::<BTreeSet<_>>();
    for arm in contract.handled_arms() {
        tags.extend(
            arm.arm_outward_cases()
                .iter()
                .copied()
                .filter(|case_tag| emitted_cases.contains(case_tag)),
        );
    }
    tags
}

#[allow(clippy::too_many_arguments)]
fn build_expected_handle_state_regions(
    owner_root_fqn: &str,
    site_id: crate::mir::SiteId,
    state_graph: &crate::effect_lowered::ir::LateLoweredStateGraph,
    dispatch_state: crate::effect_lowered::ir::StateId,
    body_state: crate::effect_lowered::ir::StateId,
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
    finally_state: Option<crate::effect_lowered::ir::StateId>,
    exit_state: crate::effect_lowered::ir::StateId,
) -> Result<
    BTreeMap<
        crate::effect_lowered::ir::StateId,
        crate::effect_lowered::ir::LateLoweredHandleStateRegion,
    >,
    LlvmEmitError,
> {
    let mut regions = BTreeMap::new();
    insert_expected_handle_state_region(
        owner_root_fqn,
        site_id,
        &mut regions,
        dispatch_state,
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Dispatch,
    )?;
    insert_expected_handle_state_region(
        owner_root_fqn,
        site_id,
        &mut regions,
        exit_state,
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Exit,
    )?;

    let mut stop_states = BTreeSet::from([dispatch_state, exit_state]);
    stop_states.extend(
        contract
            .handled_arms()
            .iter()
            .map(crate::effect_lowered::ir::LateLoweredHandleArmDispatch::arm_state),
    );
    if let Some(finally_state) = finally_state {
        stop_states.insert(finally_state);
    }

    for state_id in collect_expected_handle_region_states(
        owner_root_fqn,
        site_id,
        state_graph,
        body_state,
        &stop_states,
    )? {
        insert_expected_handle_state_region(
            owner_root_fqn,
            site_id,
            &mut regions,
            state_id,
            crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body,
        )?;
    }

    for arm in contract.handled_arms() {
        let mut arm_stops = stop_states.clone();
        arm_stops.remove(&arm.arm_state());
        let region = crate::effect_lowered::ir::LateLoweredHandleStateRegion::Arm {
            handled_case: arm.handled_case(),
            arm_ordinal: arm.arm_ordinal(),
        };
        for state_id in collect_expected_handle_region_states(
            owner_root_fqn,
            site_id,
            state_graph,
            arm.arm_state(),
            &arm_stops,
        )? {
            insert_expected_handle_state_region(
                owner_root_fqn,
                site_id,
                &mut regions,
                state_id,
                region,
            )?;
        }
    }

    if let Some(finally_state) = finally_state {
        let mut finally_stops = stop_states;
        finally_stops.remove(&finally_state);
        for state_id in collect_expected_handle_region_states(
            owner_root_fqn,
            site_id,
            state_graph,
            finally_state,
            &finally_stops,
        )? {
            insert_expected_handle_state_region(
                owner_root_fqn,
                site_id,
                &mut regions,
                state_id,
                crate::effect_lowered::ir::LateLoweredHandleStateRegion::Finally,
            )?;
        }
    }

    Ok(regions)
}

fn collect_expected_handle_region_states(
    owner_root_fqn: &str,
    site_id: crate::mir::SiteId,
    state_graph: &crate::effect_lowered::ir::LateLoweredStateGraph,
    entry_state: crate::effect_lowered::ir::StateId,
    stop_states: &BTreeSet<crate::effect_lowered::ir::StateId>,
) -> Result<BTreeSet<crate::effect_lowered::ir::StateId>, LlvmEmitError> {
    if state_graph.state(entry_state).is_none() {
        return Err(frontend_error(format!(
            "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 region root st{} 不存在于 state graph 中",
            site_id.as_u32(),
            entry_state.as_u32(),
        )));
    }

    let mut visited = BTreeSet::new();
    let mut worklist = vec![entry_state];
    while let Some(state_id) = worklist.pop() {
        if stop_states.contains(&state_id) || !visited.insert(state_id) {
            continue;
        }
        let state = state_graph.state(state_id).ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 region traversal 命中了不存在的 state st{}",
                site_id.as_u32(),
                state_id.as_u32(),
            ))
        })?;
        worklist.extend(state.successors().iter().rev().copied());
    }
    Ok(visited)
}

fn insert_expected_handle_state_region(
    owner_root_fqn: &str,
    site_id: crate::mir::SiteId,
    regions: &mut BTreeMap<
        crate::effect_lowered::ir::StateId,
        crate::effect_lowered::ir::LateLoweredHandleStateRegion,
    >,
    state_id: crate::effect_lowered::ir::StateId,
    region: crate::effect_lowered::ir::LateLoweredHandleStateRegion,
) -> Result<(), LlvmEmitError> {
    match regions.insert(state_id, region) {
        Some(existing) if existing != region => Err(frontend_error(format!(
            "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 state st{} 同时归属于 {:?} 和 {:?}",
            site_id.as_u32(),
            state_id.as_u32(),
            existing,
            region,
        ))),
        Some(_) | None => Ok(()),
    }
}

fn validate_published_handle_state_regions(
    owner_root_fqn: &str,
    site_id: crate::mir::SiteId,
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
    expected_regions: &BTreeMap<
        crate::effect_lowered::ir::StateId,
        crate::effect_lowered::ir::LateLoweredHandleStateRegion,
    >,
) -> Result<(), LlvmEmitError> {
    let mut published = BTreeMap::new();
    for entry in contract.state_regions() {
        if published.insert(entry.state_id(), entry.region()).is_some() {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 published state region 重复声明 st{}",
                site_id.as_u32(),
                entry.state_id().as_u32(),
            )));
        }
    }
    if &published != expected_regions {
        return Err(frontend_error(format!(
            "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 state-region contract 漂移：published={published:?}，state_graph={expected_regions:?}",
            site_id.as_u32(),
        )));
    }
    Ok(())
}

fn build_expected_handle_boundary_routings(
    owner_root_fqn: &str,
    site_id: crate::mir::SiteId,
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
    expected_regions: &BTreeMap<
        crate::effect_lowered::ir::StateId,
        crate::effect_lowered::ir::LateLoweredHandleStateRegion,
    >,
    boundary_map: &crate::effect_lowered::ir::LateLoweredBoundaryMap,
) -> Result<
    BTreeMap<
        crate::effect_lowered::ir::BoundaryId,
        crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting,
    >,
    LlvmEmitError,
> {
    let handled_arms = contract
        .handled_arms()
        .iter()
        .map(|arm| (arm.handled_case(), arm))
        .collect::<BTreeMap<_, _>>();
    let body_outward_cases = contract
        .body_outward_cases()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let finally_outward_cases = contract
        .finally_outward_cases()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let outward_emission_cases = contract
        .outward_emissions()
        .iter()
        .map(|emission| emission.case_tag())
        .collect::<BTreeSet<_>>();
    let pending_outward_cases = contract
        .pending_completions()
        .iter()
        .filter_map(|pending| match pending {
            crate::effect_lowered::ir::LateLoweredHandlePendingCompletion::PropagateOutward(
                case_tag,
            ) => Some((*case_tag, *pending)),
            crate::effect_lowered::ir::LateLoweredHandlePendingCompletion::ContinueToExit
            | crate::effect_lowered::ir::LateLoweredHandlePendingCompletion::ReturnFromFunction => {
                None
            }
        })
        .collect::<BTreeMap<_, _>>();
    let mut routes = BTreeMap::new();

    for boundary in boundary_map.entries() {
        let owner_region = expected_regions
            .get(&boundary.owner_state())
            .copied()
            .unwrap_or(crate::effect_lowered::ir::LateLoweredHandleStateRegion::OutsideHandle);
        if matches!(
            owner_region,
            crate::effect_lowered::ir::LateLoweredHandleStateRegion::OutsideHandle
                | crate::effect_lowered::ir::LateLoweredHandleStateRegion::Exit
        ) {
            continue;
        }
        if matches!(
            owner_region,
            crate::effect_lowered::ir::LateLoweredHandleStateRegion::Dispatch
        ) && !matches!(
            boundary.source(),
            crate::effect_lowered::ir::LateLoweredBoundarySource::Site {
                site_id: boundary_site,
                kind: crate::effect_lowered::ir::BoundarySiteKind::Handle,
            } if boundary_site == site_id
        ) {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 dispatch boundary bd{} source 漂移：{:?}",
                site_id.as_u32(),
                boundary.boundary_id().as_u32(),
                boundary.source(),
            )));
        }
        let case_tags =
            collect_expected_handle_boundary_case_tags(owner_root_fqn, site_id, boundary)?;
        let case_routings = case_tags
            .into_iter()
            .map(|case_tag| {
                build_expected_handle_boundary_case_routing(
                    owner_root_fqn,
                    site_id,
                    boundary,
                    owner_region,
                    case_tag,
                    &handled_arms,
                    &body_outward_cases,
                    &finally_outward_cases,
                    &outward_emission_cases,
                    &pending_outward_cases,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        routes.insert(
            boundary.boundary_id(),
            crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting::new(
                boundary.boundary_id(),
                boundary.owner_state(),
                owner_region,
                boundary.resume_state(),
                case_routings,
            ),
        );
    }
    Ok(routes)
}

fn collect_expected_handle_boundary_case_tags(
    owner_root_fqn: &str,
    site_id: crate::mir::SiteId,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
) -> Result<Vec<crate::effect_facts::CaseTag>, LlvmEmitError> {
    let lowering = boundary.lowering().ok_or_else(|| {
        frontend_error(format!(
            "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 boundary bd{} 缺少 lowering，无法校验 routing contract",
            site_id.as_u32(),
            boundary.boundary_id().as_u32(),
        ))
    })?;
    let mut tags = BTreeSet::new();
    let raw_tags: Vec<crate::effect_facts::CaseTag> = match lowering {
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::Call(lowering) => lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|forwarding| forwarding.emission().case_tag())
            .collect(),
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::Perform(lowering) => {
            vec![lowering.emitted_step().case_tag()]
        }
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::Resume(lowering) => lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|forwarding| forwarding.emission().case_tag())
            .collect(),
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::RuntimeError(lowering) => {
            vec![lowering.emitted_step().case_tag()]
        }
        crate::effect_lowered::ir::LateLoweredBoundaryLowering::Handle(lowering) => lowering
            .outward_emissions()
            .iter()
            .map(|emission| emission.case_tag())
            .collect(),
    };
    for case_tag in raw_tags {
        if !tags.insert(case_tag) {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 boundary bd{} 重复发布 case c{}，无法校验稳定 routing",
                site_id.as_u32(),
                boundary.boundary_id().as_u32(),
                case_tag.as_u32(),
            )));
        }
    }
    Ok(tags.into_iter().collect())
}

#[allow(clippy::too_many_arguments)]
fn build_expected_handle_boundary_case_routing(
    owner_root_fqn: &str,
    site_id: crate::mir::SiteId,
    boundary: &crate::effect_lowered::ir::LateLoweredBoundary,
    owner_region: crate::effect_lowered::ir::LateLoweredHandleStateRegion,
    case_tag: crate::effect_facts::CaseTag,
    handled_arms: &BTreeMap<
        crate::effect_facts::CaseTag,
        &crate::effect_lowered::ir::LateLoweredHandleArmDispatch,
    >,
    body_outward_cases: &BTreeSet<crate::effect_facts::CaseTag>,
    finally_outward_cases: &BTreeSet<crate::effect_facts::CaseTag>,
    outward_emission_cases: &BTreeSet<crate::effect_facts::CaseTag>,
    pending_outward_cases: &BTreeMap<
        crate::effect_facts::CaseTag,
        crate::effect_lowered::ir::LateLoweredHandlePendingCompletion,
    >,
) -> Result<crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRouting, LlvmEmitError> {
    let action = match owner_region {
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body => {
            if let Some(arm) = handled_arms.get(&case_tag) {
                crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                    arm_state: arm.arm_state(),
                    arm_ordinal: arm.arm_ordinal(),
                    continuation_resume_state: boundary.resume_state(),
                }
            } else if body_outward_cases.contains(&case_tag) {
                pending_outward_cases.get(&case_tag).copied().map_or(
                    crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward,
                    |completion| crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion { completion },
                )
            } else if finally_outward_cases.contains(&case_tag) {
                crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
            } else {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 body boundary bd{} 发布了未声明的 case c{}",
                    site_id.as_u32(),
                    boundary.boundary_id().as_u32(),
                    case_tag.as_u32(),
                )));
            }
        }
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Arm {
            handled_case,
            arm_ordinal,
        } => {
            let arm = handled_arms.get(&handled_case).ok_or_else(|| frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 arm region(c{}, ordinal={}) 缺少 handled-arm contract",
                site_id.as_u32(),
                handled_case.as_u32(),
                arm_ordinal,
            )))?;
            if arm.arm_ordinal() != arm_ordinal {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 arm region(c{}, ordinal={}) 与 handled-arm ordinal {} 不一致",
                    site_id.as_u32(),
                    handled_case.as_u32(),
                    arm_ordinal,
                    arm.arm_ordinal(),
                )));
            }
            if !arm.arm_outward_cases().contains(&case_tag) {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 arm boundary bd{} 发布了未声明的 case c{}",
                    site_id.as_u32(),
                    boundary.boundary_id().as_u32(),
                    case_tag.as_u32(),
                )));
            }
            pending_outward_cases.get(&case_tag).copied().map_or(
                crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward,
                |completion| crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion { completion },
            )
        }
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Finally => {
            if !finally_outward_cases.contains(&case_tag) {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 finally boundary bd{} 发布了未声明的 case c{}",
                    site_id.as_u32(),
                    boundary.boundary_id().as_u32(),
                    case_tag.as_u32(),
                )));
            }
            crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
        }
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Dispatch => {
            if !outward_emission_cases.contains(&case_tag) {
                return Err(frontend_error(format!(
                    "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 dispatch boundary bd{} 发布了未声明的 outward emission case c{}",
                    site_id.as_u32(),
                    boundary.boundary_id().as_u32(),
                    case_tag.as_u32(),
                )));
            }
            crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward
        }
        crate::effect_lowered::ir::LateLoweredHandleStateRegion::Exit
        | crate::effect_lowered::ir::LateLoweredHandleStateRegion::OutsideHandle => {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 boundary bd{} owner state st{} 不属于当前 handle region",
                site_id.as_u32(),
                boundary.boundary_id().as_u32(),
                boundary.owner_state().as_u32(),
            )));
        }
    };
    Ok(crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRouting::new(case_tag, action))
}

fn validate_published_handle_boundary_routings(
    owner_root_fqn: &str,
    site_id: crate::mir::SiteId,
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
    expected_routes: &BTreeMap<
        crate::effect_lowered::ir::BoundaryId,
        crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting,
    >,
) -> Result<(), LlvmEmitError> {
    let mut published = BTreeMap::new();
    for routing in contract.boundary_routings() {
        if published
            .insert(routing.boundary_id(), routing.clone())
            .is_some()
        {
            return Err(frontend_error(format!(
                "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 published boundary routing 重复声明 bd{}",
                site_id.as_u32(),
                routing.boundary_id().as_u32(),
            )));
        }
    }
    if &published != expected_routes {
        return Err(frontend_error(format!(
            "refactor LLVM ABI materialization 发现 callable `{owner_root_fqn}` handle site {} 的 boundary-routing contract 漂移：published={published:?}，expected={expected_routes:?}",
            site_id.as_u32(),
        )));
    }
    Ok(())
}

fn frontend_error(message: String) -> LlvmEmitError {
    LlvmEmitError::Frontend { message }
}

fn expected_source_types_for_carrier(
    types: &TypeStore,
    carrier_ty: TypeId,
    source_count: usize,
) -> Result<Vec<TypeId>, String> {
    match source_count {
        0 => match types.kind(carrier_ty) {
            TypeKind::Value(ValueTypeKind::Unit) => Ok(Vec::new()),
            _ => Err(format!(
                "只有 Unit carrier 才允许 0 个 source，但 published carrier 为 t{}",
                carrier_ty.as_u32(),
            )),
        },
        1 => Ok(vec![carrier_ty]),
        _ => match types.kind(carrier_ty) {
            TypeKind::Value(ValueTypeKind::Tuple(elements)) if elements.len() == source_count => {
                Ok(elements.clone())
            }
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => Err(format!(
                "published tuple carrier t{} 期望 {} 个 source，实际为 {source_count}",
                carrier_ty.as_u32(),
                elements.len(),
            )),
            _ => Err(format!(
                "published carrier t{} 期望单一 source，实际数量为 {source_count}",
                carrier_ty.as_u32(),
            )),
        },
    }
}

fn legacy_hir_closure_carrier_alias(root_fqn: &str) -> Option<String> {
    let (_, suffix) = root_fqn.rsplit_once(".$lambda")?;
    suffix
        .chars()
        .all(|ch| ch.is_ascii_digit())
        .then(|| format!("scoop.lambda${suffix}"))
}

fn render_resume_packing_ids(interface_ids: &[ResumeInterfaceId]) -> String {
    format!(
        "[{}]",
        interface_ids
            .iter()
            .map(|interface_id| interface_id.as_u32().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn same_surface_resume_wrapper_projection_shape(
    left: &LateLoweredSurfaceResumeWrapperProjection,
    right: &LateLoweredSurfaceResumeWrapperProjection,
) -> bool {
    left == right
        || (left.underlying_route().continuation_schema()
            == right.underlying_route().continuation_schema()
            && matches!(
                (
                    left.underlying_route().publication(),
                    right.underlying_route().publication()
                ),
                (
                    LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary { .. },
                    LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary { .. }
                )
            )
            && left.owner_step_schema() == right.owner_step_schema()
            && left.wrapper_step_schema() == right.wrapper_step_schema()
            && left.complete() == right.complete()
            && left.outward_cases() == right.outward_cases())
}

fn render_case_tags(tags: &BTreeSet<crate::effect_facts::CaseTag>) -> String {
    if tags.is_empty() {
        return "[]".to_string();
    }
    format!(
        "[{}]",
        tags.iter()
            .map(|case_tag| format!("c{}", case_tag.as_u32()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::{BTreeSet, HashMap};
    use std::rc::Rc;

    use super::*;
    use crate::effect_facts::{
        CallSiteEffectFacts, CallSiteKind, CallSiteTarget, CallTargetMode, CaseTag,
        EffectPrecision, ImplPlan, SiteEffectFacts, StepSchemaId,
    };
    use crate::effect_lowered::ir::{
        BoundarySiteKind, ContinuationObjectId, LateLoweredBodyVersionKey, LateLoweredBoundary,
        LateLoweredBoundaryLowering, LateLoweredBoundaryMap, LateLoweredBoundarySource,
        LateLoweredBoundarySourceConsumption, LateLoweredCallBoundaryLowering,
        LateLoweredCallBoundaryOperandContract, LateLoweredCallable,
        LateLoweredCompletionPayloadBinding, LateLoweredCompletionPayloadSource,
        LateLoweredConsumedRuntimeErrorCase, LateLoweredContinuationObject,
        LateLoweredContinuationSurfaceResume, LateLoweredDynamicInvokeEntry,
        LateLoweredFrameSchema, LateLoweredFrameSlotKind, LateLoweredHandleDispatchContract,
        LateLoweredHandlePendingCompletion, LateLoweredOperandValueSource, LateLoweredProgram,
        LateLoweredResumeInterface, LateLoweredResumeMethod, LateLoweredResumePayloadBinding,
        LateLoweredSourceStatementClassification, LateLoweredStateTerminator, LateLoweredStepType,
        LateLoweredSurfaceResumeDispatchPublication, StateId, SystemSlotKind,
    };
    use crate::effect_lowered::{
        LateLoweredOptOptions, LateLoweredProgramBuilder, optimize_program_with_options,
    };
    use crate::effect_refactor_pipeline::{
        RefactorMirStageOutput, build_effect_facts_stage_output, build_effect_lowered_stage_output,
        load_typed_hir_stage_output_for_dump,
    };
    use crate::llvm::build_single_file_source_map;
    use crate::llvm::codegen::effect_refactor::types::RefactorCallTargetQuery;
    use crate::llvm::codegen::{
        CompilationUnitCodegenCx, CompilationUnitCodegenInputs, EffectOpTagState, MainCodegen,
    };
    use crate::llvm::target;
    use crate::mir::{LoweredMir, MirLoweringFacts, SiteId, lower_hir_file_for_dump_with_facts};
    use crate::program_facts::ProgramFacts;
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::{SourceFile, SourceMap};
    use crate::ty::{TypeParamType, TypeStore};
    use inkwell::context::Context;

    struct FixtureAbiInputs {
        source_map: SourceMap,
        entry_source_id: crate::source::SourceId,
        hir_compat_scaffold: crate::hir::LoweredHir,
        effect_lowered_stage_output:
            crate::effect_refactor_pipeline::RefactorEffectLoweredStageOutput,
        abi_visibility_program: LateLoweredProgram,
    }

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap()
    }

    fn load_fixture(phase: &str, name: &str) -> SourceFile {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(phase)
            .join(name);
        SourceFile::load(&path).expect("fixture 应可加载")
    }

    fn load_build_fixture(name: &str) -> SourceFile {
        load_fixture("build", name)
    }

    fn build_fixture_inputs_from_source(source: SourceFile) -> FixtureAbiInputs {
        let session = refactor_session();
        let typed_hir_output = load_typed_hir_stage_output_for_dump(&session, &source)
            .expect("typed HIR stage 应成功");
        let hir_compat_scaffold = typed_hir_output
            .lowered_hir()
            .clone_hir_compat_scaffold_without_materialized_mir();
        let facts = MirLoweringFacts::from_refactor_typed_handoff(
            typed_hir_output.lowered_hir(),
            typed_hir_output.effect_contracts(),
        );
        let effect_contracts = typed_hir_output.effect_contracts().clone();
        let mut lowered_hir = typed_hir_output.into_lowered_hir();
        let builtins = lowered_hir.types.intern_builtins();
        let file = lower_hir_file_for_dump_with_facts(
            builtins,
            &mut lowered_hir.types,
            &lowered_hir.file,
            &lowered_hir.member_funs,
            &facts,
        );
        let types = std::mem::replace(&mut lowered_hir.types, TypeStore::new());
        let materialized_mir = lowered_hir.into_materialized_mir();
        let mir_stage_output = RefactorMirStageOutput::new(
            LoweredMir { file, types },
            effect_contracts,
            materialized_mir,
        );
        let effect_facts_stage_output =
            build_effect_facts_stage_output(&session, &source, mir_stage_output)
                .expect("effect facts stage 应成功");
        let effect_lowered_stage_output =
            build_effect_lowered_stage_output(&session, effect_facts_stage_output)
                .expect("effect lowered stage 应成功");
        // ABI materializer 必须消费与真实 refactor LLVM stage 相同的 shell-preserving handoff，
        // 不能误用会裁剪 published resume methods 的 authoritative reachable-body program。
        let abi_visibility_program = optimize_program_with_options(
            LateLoweredProgramBuilder::from_canonical_inputs(
                effect_lowered_stage_output.materialized_pass_view(),
                effect_lowered_stage_output.effect_facts(),
                effect_lowered_stage_output.types(),
            )
            .build()
            .expect("ABI visibility late-lowered program 应成功"),
            LateLoweredOptOptions::preserve_published_resume_shells(),
        );
        let (source_map, entry_source_id) = build_single_file_source_map(&session, &source);
        FixtureAbiInputs {
            source_map,
            entry_source_id,
            hir_compat_scaffold,
            effect_lowered_stage_output,
            abi_visibility_program,
        }
    }

    fn build_fixture_inputs(name: &str) -> FixtureAbiInputs {
        build_fixture_inputs_from_source(load_build_fixture(name))
    }

    fn with_inputs_query_result(
        inputs: FixtureAbiInputs,
        rewrite_program: impl FnOnce(&FixtureAbiInputs) -> LateLoweredProgram,
        check: impl for<'ctx> FnOnce(
            &FixtureAbiInputs,
            Result<RefactorAbiQuery<'ctx>, LlvmEmitError>,
            &inkwell::module::Module<'ctx>,
        ),
    ) {
        let program = rewrite_program(&inputs);
        let context = Context::create();
        let module = context.create_module("refactor_abi_test");
        let builder = context.create_builder();
        let target_info = target::configure_module_for_host(&module).expect("host target 应可配置");
        let target_data = inkwell::targets::TargetData::create(&target_info.data_layout);
        let lowered = &inputs.hir_compat_scaffold;
        let fun_index: HashMap<String, &crate::hir::FunDecl> = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                crate::hir::Item::Fun(fun) => Some(fun),
                _ => None,
            })
            .chain(lowered.member_funs.iter())
            .map(|fun| (fun.fqn.clone(), fun))
            .collect();
        let program_facts = Rc::new(ProgramFacts::from_lowered(lowered));
        let effect_op_tags = Rc::new(RefCell::new(EffectOpTagState::new()));
        let unit_codegen = CompilationUnitCodegenCx::new(CompilationUnitCodegenInputs {
            context: &context,
            module: &module,
            builder: &builder,
            target_data: &target_data,
            host: &target_info,
            source_map: &inputs.source_map,
            entry_source_id: inputs.entry_source_id,
            types: &lowered.types,
            struct_layouts: &lowered.struct_layouts,
            enum_layouts: &lowered.enum_layouts,
            top_level_vars: &lowered.top_level_vars,
            top_level_consts: &lowered.top_level_consts,
            top_level_immutable_values: &lowered.top_level_immutable_values,
            object_inits: &lowered.object_inits,
            class_inits: &lowered.class_inits,
            class_vtables: &lowered.class_vtables,
            interfaces: &lowered.interfaces,
            class_itables: &lowered.class_itables,
            ctor_call_sites: &lowered.ctor_call_sites,
            dispatch_call_sites: &lowered.dispatch_call_sites,
            effect_op_call_sites: &lowered.effect_op_call_sites,
            handle_payload_tuple_tys: &lowered.handle_payload_tuple_tys,
            continuation_resume_call_sites: &lowered.continuation_resume_call_sites,
            when_pat_binding_tys: &lowered.when_pat_binding_tys,
            nominal_kinds: &lowered.nominal_kinds,
            nominal_variances: &lowered.nominal_variances,
            direct_supertypes: &lowered.direct_supertypes,
            builtins: lowered.builtins,
            extern_funs: &lowered.extern_funs,
            fun_index: &fun_index,
            materialized_pass_view: Some(
                inputs.effect_lowered_stage_output.materialized_pass_view(),
            ),
            program_facts,
            effect_op_tags,
        });
        let mut codegen = unit_codegen.fresh_main_codegen();
        let result = codegen.materialize_refactor_program_abi(
            &program,
            inputs.effect_lowered_stage_output.types(),
            &inputs.effect_lowered_stage_output.materialized_pass_view(),
            inputs.effect_lowered_stage_output.effect_facts(),
        );
        check(&inputs, result, &module);
    }

    fn with_inputs_query_result_for_source_types(
        inputs: FixtureAbiInputs,
        rewrite_program: impl FnOnce(&FixtureAbiInputs) -> LateLoweredProgram,
        rewrite_source_types: impl FnOnce(&FixtureAbiInputs) -> TypeStore,
        check: impl for<'ctx> FnOnce(
            &FixtureAbiInputs,
            Result<RefactorAbiQuery<'ctx>, LlvmEmitError>,
            &inkwell::module::Module<'ctx>,
        ),
    ) {
        let program = rewrite_program(&inputs);
        let source_types = rewrite_source_types(&inputs);
        let context = Context::create();
        let module = context.create_module("refactor_abi_test");
        let builder = context.create_builder();
        let target_info = target::configure_module_for_host(&module).expect("host target 应可配置");
        let target_data = inkwell::targets::TargetData::create(&target_info.data_layout);
        let lowered = &inputs.hir_compat_scaffold;
        let fun_index: HashMap<String, &crate::hir::FunDecl> = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                crate::hir::Item::Fun(fun) => Some(fun),
                _ => None,
            })
            .chain(lowered.member_funs.iter())
            .map(|fun| (fun.fqn.clone(), fun))
            .collect();
        let program_facts = Rc::new(ProgramFacts::from_lowered(lowered));
        let effect_op_tags = Rc::new(RefCell::new(EffectOpTagState::new()));
        let unit_codegen = CompilationUnitCodegenCx::new(CompilationUnitCodegenInputs {
            context: &context,
            module: &module,
            builder: &builder,
            target_data: &target_data,
            host: &target_info,
            source_map: &inputs.source_map,
            entry_source_id: inputs.entry_source_id,
            types: &lowered.types,
            struct_layouts: &lowered.struct_layouts,
            enum_layouts: &lowered.enum_layouts,
            top_level_vars: &lowered.top_level_vars,
            top_level_consts: &lowered.top_level_consts,
            top_level_immutable_values: &lowered.top_level_immutable_values,
            object_inits: &lowered.object_inits,
            class_inits: &lowered.class_inits,
            class_vtables: &lowered.class_vtables,
            interfaces: &lowered.interfaces,
            class_itables: &lowered.class_itables,
            ctor_call_sites: &lowered.ctor_call_sites,
            dispatch_call_sites: &lowered.dispatch_call_sites,
            effect_op_call_sites: &lowered.effect_op_call_sites,
            handle_payload_tuple_tys: &lowered.handle_payload_tuple_tys,
            continuation_resume_call_sites: &lowered.continuation_resume_call_sites,
            when_pat_binding_tys: &lowered.when_pat_binding_tys,
            nominal_kinds: &lowered.nominal_kinds,
            nominal_variances: &lowered.nominal_variances,
            direct_supertypes: &lowered.direct_supertypes,
            builtins: lowered.builtins,
            extern_funs: &lowered.extern_funs,
            fun_index: &fun_index,
            materialized_pass_view: Some(
                inputs.effect_lowered_stage_output.materialized_pass_view(),
            ),
            program_facts,
            effect_op_tags,
        });
        let mut codegen = unit_codegen.fresh_main_codegen();
        let result = codegen.materialize_refactor_program_abi(
            &program,
            &source_types,
            &inputs.effect_lowered_stage_output.materialized_pass_view(),
            inputs.effect_lowered_stage_output.effect_facts(),
        );
        check(&inputs, result, &module);
    }

    fn with_inputs_query_result_and_codegen(
        inputs: FixtureAbiInputs,
        rewrite_program: impl FnOnce(&FixtureAbiInputs) -> LateLoweredProgram,
        check: impl for<'ctx> FnOnce(
            &FixtureAbiInputs,
            &mut MainCodegen<'_, 'ctx>,
            Result<RefactorAbiQuery<'ctx>, LlvmEmitError>,
            &inkwell::module::Module<'ctx>,
        ),
    ) {
        let program = rewrite_program(&inputs);
        let context = Context::create();
        let module = context.create_module("refactor_abi_test");
        let builder = context.create_builder();
        let target_info = target::configure_module_for_host(&module).expect("host target 应可配置");
        let target_data = inkwell::targets::TargetData::create(&target_info.data_layout);
        let lowered = &inputs.hir_compat_scaffold;
        let fun_index: HashMap<String, &crate::hir::FunDecl> = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                crate::hir::Item::Fun(fun) => Some(fun),
                _ => None,
            })
            .chain(lowered.member_funs.iter())
            .map(|fun| (fun.fqn.clone(), fun))
            .collect();
        let program_facts = Rc::new(ProgramFacts::from_lowered(lowered));
        let effect_op_tags = Rc::new(RefCell::new(EffectOpTagState::new()));
        let unit_codegen = CompilationUnitCodegenCx::new(CompilationUnitCodegenInputs {
            context: &context,
            module: &module,
            builder: &builder,
            target_data: &target_data,
            host: &target_info,
            source_map: &inputs.source_map,
            entry_source_id: inputs.entry_source_id,
            types: &lowered.types,
            struct_layouts: &lowered.struct_layouts,
            enum_layouts: &lowered.enum_layouts,
            top_level_vars: &lowered.top_level_vars,
            top_level_consts: &lowered.top_level_consts,
            top_level_immutable_values: &lowered.top_level_immutable_values,
            object_inits: &lowered.object_inits,
            class_inits: &lowered.class_inits,
            class_vtables: &lowered.class_vtables,
            interfaces: &lowered.interfaces,
            class_itables: &lowered.class_itables,
            ctor_call_sites: &lowered.ctor_call_sites,
            dispatch_call_sites: &lowered.dispatch_call_sites,
            effect_op_call_sites: &lowered.effect_op_call_sites,
            handle_payload_tuple_tys: &lowered.handle_payload_tuple_tys,
            continuation_resume_call_sites: &lowered.continuation_resume_call_sites,
            when_pat_binding_tys: &lowered.when_pat_binding_tys,
            nominal_kinds: &lowered.nominal_kinds,
            nominal_variances: &lowered.nominal_variances,
            direct_supertypes: &lowered.direct_supertypes,
            builtins: lowered.builtins,
            extern_funs: &lowered.extern_funs,
            fun_index: &fun_index,
            materialized_pass_view: Some(
                inputs.effect_lowered_stage_output.materialized_pass_view(),
            ),
            program_facts,
            effect_op_tags,
        });
        let mut codegen = unit_codegen.fresh_main_codegen();
        let pass_view = inputs.effect_lowered_stage_output.materialized_pass_view();
        let result = codegen.materialize_refactor_program_abi(
            &program,
            inputs.effect_lowered_stage_output.types(),
            &pass_view,
            inputs.effect_lowered_stage_output.effect_facts(),
        );
        check(&inputs, &mut codegen, result, &module);
    }

    fn with_fixture_query_result(
        name: &str,
        rewrite_program: impl FnOnce(&FixtureAbiInputs) -> LateLoweredProgram,
        check: impl for<'ctx> FnOnce(
            &FixtureAbiInputs,
            Result<RefactorAbiQuery<'ctx>, LlvmEmitError>,
            &inkwell::module::Module<'ctx>,
        ),
    ) {
        with_inputs_query_result(build_fixture_inputs(name), rewrite_program, check);
    }

    fn with_phase_fixture_query_result(
        phase: &str,
        name: &str,
        rewrite_program: impl FnOnce(&FixtureAbiInputs) -> LateLoweredProgram,
        check: impl for<'ctx> FnOnce(
            &FixtureAbiInputs,
            Result<RefactorAbiQuery<'ctx>, LlvmEmitError>,
            &inkwell::module::Module<'ctx>,
        ),
    ) {
        with_inputs_query_result(
            build_fixture_inputs_from_source(load_fixture(phase, name)),
            rewrite_program,
            check,
        );
    }

    fn with_fixture_query(
        name: &str,
        check: impl for<'ctx> FnOnce(
            &FixtureAbiInputs,
            &RefactorAbiQuery<'ctx>,
            &inkwell::module::Module<'ctx>,
        ),
    ) {
        with_fixture_query_result(
            name,
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, module| {
                let query = result.expect("refactor ABI materialization 应成功");
                check(inputs, &query, module);
            },
        );
    }

    fn clone_callable_with_interfaces(
        callable: &LateLoweredCallable,
        resume_interfaces: Vec<ResumeInterfaceId>,
    ) -> LateLoweredCallable {
        LateLoweredCallable::new(
            callable.root_fqn().to_string(),
            callable.body_version_key().clone(),
            callable.step_schema(),
            callable.resolved_outward_cases().to_vec(),
            callable.dynamic_invoke_entry().clone(),
            callable.state_graph().clone(),
            callable.frame_schema().clone(),
            callable.boundary_map().clone(),
            callable.resume_state_map().clone(),
            callable.continuation_object(),
            resume_interfaces,
        )
        .with_source_statement_classifications(callable.source_statement_classifications().to_vec())
    }

    fn clone_continuation_object_with_interfaces(
        object: &LateLoweredContinuationObject,
        implemented_interfaces: Vec<ResumeInterfaceId>,
    ) -> LateLoweredContinuationObject {
        LateLoweredContinuationObject::new(
            object.object_id(),
            object.owner_version_key().clone(),
            object.continuation_obj_ty(),
            implemented_interfaces,
            object.captures().to_vec(),
            object.surface_resumes().to_vec(),
            object.methods().to_vec(),
        )
    }

    fn clone_continuation_object_with_surface_resumes(
        object: &LateLoweredContinuationObject,
        surface_resumes: Vec<LateLoweredContinuationSurfaceResume>,
    ) -> LateLoweredContinuationObject {
        LateLoweredContinuationObject::new(
            object.object_id(),
            object.owner_version_key().clone(),
            object.continuation_obj_ty(),
            object.implemented_packings().to_vec(),
            object.captures().to_vec(),
            surface_resumes,
            object.methods().to_vec(),
        )
    }

    fn clone_continuation_object_with_methods(
        object: &LateLoweredContinuationObject,
        methods: Vec<crate::effect_lowered::ir::LateLoweredContinuationMethod>,
    ) -> LateLoweredContinuationObject {
        LateLoweredContinuationObject::new(
            object.object_id(),
            object.owner_version_key().clone(),
            object.continuation_obj_ty(),
            object.implemented_packings().to_vec(),
            object.captures().to_vec(),
            object.surface_resumes().to_vec(),
            methods,
        )
    }

    fn clone_continuation_object_with_id(
        object: &LateLoweredContinuationObject,
        object_id: ContinuationObjectId,
    ) -> LateLoweredContinuationObject {
        LateLoweredContinuationObject::new(
            object_id,
            object.owner_version_key().clone(),
            object.continuation_obj_ty(),
            object.implemented_packings().to_vec(),
            object.captures().to_vec(),
            object.surface_resumes().to_vec(),
            object.methods().to_vec(),
        )
    }

    fn clone_callable_with_boundary_map(
        callable: &LateLoweredCallable,
        boundary_map: LateLoweredBoundaryMap,
    ) -> LateLoweredCallable {
        LateLoweredCallable::new(
            callable.root_fqn().to_string(),
            callable.body_version_key().clone(),
            callable.step_schema(),
            callable.resolved_outward_cases().to_vec(),
            callable.dynamic_invoke_entry().clone(),
            callable.state_graph().clone(),
            callable.frame_schema().clone(),
            boundary_map,
            callable.resume_state_map().clone(),
            callable.continuation_object(),
            callable.resume_packings().to_vec(),
        )
        .with_source_statement_classifications(callable.source_statement_classifications().to_vec())
    }

    fn clone_callable_with_state_graph(
        callable: &LateLoweredCallable,
        state_graph: crate::effect_lowered::ir::LateLoweredStateGraph,
    ) -> LateLoweredCallable {
        LateLoweredCallable::new(
            callable.root_fqn().to_string(),
            callable.body_version_key().clone(),
            callable.step_schema(),
            callable.resolved_outward_cases().to_vec(),
            callable.dynamic_invoke_entry().clone(),
            state_graph,
            callable.frame_schema().clone(),
            callable.boundary_map().clone(),
            callable.resume_state_map().clone(),
            callable.continuation_object(),
            callable.resume_packings().to_vec(),
        )
        .with_source_statement_classifications(callable.source_statement_classifications().to_vec())
    }

    fn clone_callable_with_frame_schema(
        callable: &LateLoweredCallable,
        frame_schema: LateLoweredFrameSchema,
    ) -> LateLoweredCallable {
        LateLoweredCallable::new(
            callable.root_fqn().to_string(),
            callable.body_version_key().clone(),
            callable.step_schema(),
            callable.resolved_outward_cases().to_vec(),
            callable.dynamic_invoke_entry().clone(),
            callable.state_graph().clone(),
            frame_schema,
            callable.boundary_map().clone(),
            callable.resume_state_map().clone(),
            callable.continuation_object(),
            callable.resume_packings().to_vec(),
        )
        .with_source_statement_classifications(callable.source_statement_classifications().to_vec())
    }

    fn clone_callable_with_source_statement_classifications(
        callable: &LateLoweredCallable,
        classifications: Vec<LateLoweredSourceStatementClassification>,
    ) -> LateLoweredCallable {
        LateLoweredCallable::new(
            callable.root_fqn().to_string(),
            callable.body_version_key().clone(),
            callable.step_schema(),
            callable.resolved_outward_cases().to_vec(),
            callable.dynamic_invoke_entry().clone(),
            callable.state_graph().clone(),
            callable.frame_schema().clone(),
            callable.boundary_map().clone(),
            callable.resume_state_map().clone(),
            callable.continuation_object(),
            callable.resume_packings().to_vec(),
        )
        .with_source_statement_classifications(classifications)
    }

    fn clone_state_graph_with_handle_contract(
        state_graph: &crate::effect_lowered::ir::LateLoweredStateGraph,
        site_id: SiteId,
        new_contract: LateLoweredHandleDispatchContract,
    ) -> crate::effect_lowered::ir::LateLoweredStateGraph {
        let states = state_graph
            .states()
            .iter()
            .map(|state| match state.terminator() {
                crate::effect_lowered::ir::LateLoweredStateTerminator::HandleDispatch {
                    site_id: state_site,
                    body_state,
                    arm_states,
                    finally_state,
                    exit_state,
                    boundary_ids,
                    drop_state,
                    ..
                } if *state_site == site_id => crate::effect_lowered::ir::LateLoweredState::new(
                    state.state_id(),
                    state.role(),
                    state.source_slices().to_vec(),
                    crate::effect_lowered::ir::LateLoweredStateTerminator::HandleDispatch {
                        site_id: *state_site,
                        body_state: *body_state,
                        arm_states: arm_states.clone(),
                        finally_state: *finally_state,
                        exit_state: *exit_state,
                        contract: new_contract.clone(),
                        boundary_ids: boundary_ids.clone(),
                        drop_state: *drop_state,
                    },
                ),
                _ => state.clone(),
            })
            .collect();
        crate::effect_lowered::ir::LateLoweredStateGraph::new(
            state_graph.entry_state(),
            state_graph.complete_state(),
            state_graph.cleanup_state(),
            state_graph.drop_state(),
            states,
        )
    }

    fn handle_dispatch_contract(
        callable: &LateLoweredCallable,
        site_id: SiteId,
    ) -> &LateLoweredHandleDispatchContract {
        callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                crate::effect_lowered::ir::LateLoweredStateTerminator::HandleDispatch {
                    site_id: state_site,
                    contract,
                    ..
                } if *state_site == site_id => Some(contract),
                _ => None,
            })
            .expect("应找到指定 site 的 HandleDispatch contract")
    }

    fn first_handle_dispatch(
        callable: &LateLoweredCallable,
    ) -> (SiteId, &LateLoweredHandleDispatchContract) {
        callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                crate::effect_lowered::ir::LateLoweredStateTerminator::HandleDispatch {
                    site_id,
                    contract,
                    ..
                } => Some((*site_id, contract)),
                _ => None,
            })
            .expect("应找到至少一个 HandleDispatch contract")
    }

    fn handle_dispatch_with_pending_outward(
        callable: &LateLoweredCallable,
    ) -> (SiteId, &LateLoweredHandleDispatchContract) {
        callable
            .state_graph()
            .states()
            .iter()
            .find_map(|state| match state.terminator() {
                crate::effect_lowered::ir::LateLoweredStateTerminator::HandleDispatch {
                    site_id,
                    contract,
                    ..
                } if contract.pending_completions().iter().any(|completion| {
                    matches!(
                        completion,
                        LateLoweredHandlePendingCompletion::PropagateOutward(_)
                    )
                }) =>
                {
                    Some((*site_id, contract))
                }
                _ => None,
            })
            .expect("应找到带 pending outward completion 的 HandleDispatch contract")
    }

    fn clone_handle_dispatch_contract_with_handled_arms(
        contract: &LateLoweredHandleDispatchContract,
        handled_arms: Vec<crate::effect_lowered::ir::LateLoweredHandleArmDispatch>,
    ) -> LateLoweredHandleDispatchContract {
        LateLoweredHandleDispatchContract::new(
            contract.carrier(),
            contract.body_complete_target(),
            contract.arm_complete_target(),
            contract.finally_complete_target(),
            contract.body_completion_payload_source().cloned(),
            handled_arms,
            contract.body_outward_cases().to_vec(),
            contract.finally_outward_cases().to_vec(),
            contract.outward_emissions().to_vec(),
            contract.pending_completions().to_vec(),
            contract.pending_payload_transports().to_vec(),
            contract.state_regions().to_vec(),
            contract.boundary_routings().to_vec(),
            contract.abandon_target(),
        )
    }

    fn clone_handle_dispatch_contract_with_regions_and_routes(
        contract: &LateLoweredHandleDispatchContract,
        state_regions: Vec<crate::effect_lowered::ir::LateLoweredHandleStateRegionEntry>,
        boundary_routings: Vec<crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting>,
    ) -> LateLoweredHandleDispatchContract {
        LateLoweredHandleDispatchContract::new(
            contract.carrier(),
            contract.body_complete_target(),
            contract.arm_complete_target(),
            contract.finally_complete_target(),
            contract.body_completion_payload_source().cloned(),
            contract.handled_arms().to_vec(),
            contract.body_outward_cases().to_vec(),
            contract.finally_outward_cases().to_vec(),
            contract.outward_emissions().to_vec(),
            contract.pending_completions().to_vec(),
            contract.pending_payload_transports().to_vec(),
            state_regions,
            boundary_routings,
            contract.abandon_target(),
        )
    }

    fn clone_callable_with_dynamic_invoke_entry(
        callable: &LateLoweredCallable,
        dynamic_invoke_entry: LateLoweredDynamicInvokeEntry,
    ) -> LateLoweredCallable {
        LateLoweredCallable::new(
            callable.root_fqn().to_string(),
            callable.body_version_key().clone(),
            callable.step_schema(),
            callable.resolved_outward_cases().to_vec(),
            dynamic_invoke_entry,
            callable.state_graph().clone(),
            callable.frame_schema().clone(),
            callable.boundary_map().clone(),
            callable.resume_state_map().clone(),
            callable.continuation_object(),
            callable.resume_packings().to_vec(),
        )
        .with_source_statement_classifications(callable.source_statement_classifications().to_vec())
    }

    fn next_step_schema_id(program: &LateLoweredProgram) -> StepSchemaId {
        let next = program
            .step_types()
            .iter()
            .map(|step_type| step_type.step_schema().as_u32())
            .max()
            .map(|raw| raw.saturating_add(1))
            .unwrap_or(0);
        StepSchemaId::new(next)
    }

    fn next_continuation_object_id(program: &LateLoweredProgram) -> ContinuationObjectId {
        let next = program
            .continuation_objects()
            .iter()
            .map(|object| object.object_id().as_u32())
            .max()
            .map(|raw| raw.saturating_add(1))
            .unwrap_or(0);
        ContinuationObjectId::new(next)
    }

    fn clone_step_type_with_step_schema(
        step_type: &LateLoweredStepType,
        step_schema: StepSchemaId,
    ) -> LateLoweredStepType {
        assert!(
            step_type.cases().is_empty(),
            "当前 helper 只支持无 outward case 的 callable version 克隆"
        );
        LateLoweredStepType::new(
            step_schema,
            step_type.invoke_args_tuple_ty(),
            step_type.complete_ty(),
            step_type.continuation_obj_ty(),
            Vec::new(),
        )
    }

    fn clone_no_outward_continuation_object_with_version(
        object: &LateLoweredContinuationObject,
        object_id: ContinuationObjectId,
        owner_version_key: LateLoweredBodyVersionKey,
    ) -> LateLoweredContinuationObject {
        assert!(
            object.implemented_packings().is_empty()
                && object.surface_resumes().is_empty()
                && object.methods().is_empty(),
            "当前 helper 只支持无 resume publication 的 continuation object 克隆"
        );
        LateLoweredContinuationObject::new(
            object_id,
            owner_version_key,
            object.continuation_obj_ty(),
            Vec::new(),
            object.captures().to_vec(),
            Vec::new(),
            Vec::new(),
        )
    }

    fn clone_no_outward_callable_with_version(
        callable: &LateLoweredCallable,
        body_version_key: LateLoweredBodyVersionKey,
        step_schema: StepSchemaId,
        continuation_object: ContinuationObjectId,
    ) -> LateLoweredCallable {
        assert!(
            callable.resolved_outward_cases().is_empty() && callable.resume_packings().is_empty(),
            "当前 helper 只支持无 outward case / 无 resume packing 的 callable version 克隆"
        );
        LateLoweredCallable::new(
            callable.root_fqn().to_string(),
            body_version_key,
            step_schema,
            Vec::new(),
            LateLoweredDynamicInvokeEntry::new(
                callable.dynamic_invoke_entry().invoke_args_tuple_ty(),
                step_schema,
                callable.dynamic_invoke_entry().entry_state(),
                callable.dynamic_invoke_entry().complete_state(),
            ),
            callable.state_graph().clone(),
            callable.frame_schema().clone(),
            callable.boundary_map().clone(),
            callable.resume_state_map().clone(),
            continuation_object,
            Vec::new(),
        )
        .with_source_statement_classifications(callable.source_statement_classifications().to_vec())
    }

    fn duplicate_no_outward_callable_version(
        program: &LateLoweredProgram,
        root_fqn: &str,
    ) -> LateLoweredProgram {
        let callable = program
            .callables()
            .iter()
            .find(|callable| callable.root_fqn() == root_fqn)
            .unwrap_or_else(|| panic!("应存在 callable `{root_fqn}`"));
        assert_eq!(
            callable.impl_plan(),
            ImplPlan::NoOutward,
            "当前 helper 只支持 NoOutward callable version"
        );

        let step_type = program
            .step_type(callable.step_schema())
            .expect("step type 应存在");
        let continuation_object = program
            .continuation_object(callable.continuation_object())
            .expect("continuation object 应存在");
        let next_step_schema = next_step_schema_id(program);
        let next_object_id = next_continuation_object_id(program);
        let cloned_version_key = LateLoweredBodyVersionKey::new(
            callable.instance_key().clone(),
            callable.allowed_row().clone(),
            callable.impl_plan(),
            !callable.needs_reentry(),
        );

        let mut step_types = program.step_types().to_vec();
        step_types.push(clone_step_type_with_step_schema(
            step_type,
            next_step_schema,
        ));

        let mut continuation_objects = program.continuation_objects().to_vec();
        continuation_objects.push(clone_no_outward_continuation_object_with_version(
            continuation_object,
            next_object_id,
            cloned_version_key.clone(),
        ));

        let mut callables = program.callables().to_vec();
        callables.push(clone_no_outward_callable_with_version(
            callable,
            cloned_version_key,
            next_step_schema,
            next_object_id,
        ));

        LateLoweredProgram::new(
            step_types,
            program.resume_packings().to_vec(),
            continuation_objects,
            callables,
        )
    }

    fn site_boundary(
        callable: &LateLoweredCallable,
        kind: BoundarySiteKind,
    ) -> &LateLoweredBoundary {
        callable
            .boundary_map()
            .entries()
            .iter()
            .find(|boundary| {
                matches!(
                    boundary.source(),
                    LateLoweredBoundarySource::Site {
                        kind: boundary_kind,
                        ..
                    } if boundary_kind == kind
                )
            })
            .expect("应找到指定 kind 的 boundary")
    }

    fn call_boundary_lowering(boundary: &LateLoweredBoundary) -> &LateLoweredCallBoundaryLowering {
        let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering() else {
            panic!("boundary 应物化成 Call lowering");
        };
        lowering
    }

    fn perform_boundary_lowering(
        boundary: &LateLoweredBoundary,
    ) -> &crate::effect_lowered::ir::LateLoweredPerformBoundaryLowering {
        let Some(LateLoweredBoundaryLowering::Perform(lowering)) = boundary.lowering() else {
            panic!("boundary 应物化成 Perform lowering");
        };
        lowering
    }

    fn resume_boundary_lowering(
        boundary: &LateLoweredBoundary,
    ) -> &crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering {
        let Some(LateLoweredBoundaryLowering::Resume(lowering)) = boundary.lowering() else {
            panic!("boundary 应物化成 Resume lowering");
        };
        lowering
    }

    fn boundary_site_id(boundary: &LateLoweredBoundary) -> crate::mir::SiteId {
        let LateLoweredBoundarySource::Site { site_id, .. } = boundary.source() else {
            panic!("boundary 应带 site source");
        };
        site_id
    }

    fn handle_dispatch_state(
        callable: &LateLoweredCallable,
        site_id: SiteId,
    ) -> &crate::effect_lowered::ir::LateLoweredState {
        callable
            .state_graph()
            .states()
            .iter()
            .find(|state| {
                matches!(
                    state.terminator(),
                    LateLoweredStateTerminator::HandleDispatch { site_id: state_site, .. }
                        if *state_site == site_id
                )
            })
            .expect("应找到指定 site 的 HandleDispatch state")
    }

    fn source_slice_non_boundary_dynamic_call_site(
        inputs: &FixtureAbiInputs,
        callable: &LateLoweredCallable,
    ) -> (crate::mir::SiteId, CallSiteEffectFacts) {
        let body = inputs
            .effect_lowered_stage_output
            .materialized_pass_view()
            .callable(callable.root_fqn())
            .expect("callable 的 canonical MIR body 应存在")
            .body
            .as_ref()
            .expect("callable 的 canonical MIR body 内容应存在");
        let body_facts = inputs
            .effect_lowered_stage_output
            .effect_facts()
            .body(callable.instance_key())
            .expect("callable 的 BodyEffectFacts 应存在");
        let boundary_call_sites = callable
            .boundary_map()
            .entries()
            .iter()
            .filter_map(|boundary| match boundary.source() {
                LateLoweredBoundarySource::Site {
                    site_id,
                    kind: BoundarySiteKind::Call,
                } => Some(site_id),
                LateLoweredBoundarySource::RuntimeError { .. }
                | LateLoweredBoundarySource::Site { .. } => None,
            })
            .collect::<BTreeSet<_>>();

        for state in callable.state_graph().states() {
            for slice in state.source_slices() {
                let block = &body.blocks[slice.block_id().as_u32() as usize];
                let start = slice.start_statement_index() as usize;
                let end = slice.end_statement_index() as usize;
                for stmt in &block.stmts[start..end] {
                    let MirStatementKind::Assign {
                        value: MirRvalue::Call { site_id, kind, .. },
                        ..
                    } = &stmt.kind
                    else {
                        continue;
                    };
                    if boundary_call_sites.contains(site_id)
                        || !matches!(
                            kind,
                            MirCallKind::FunValue { .. }
                                | MirCallKind::Closure { .. }
                                | MirCallKind::Virtual { .. }
                                | MirCallKind::Interface { .. }
                        )
                    {
                        continue;
                    }
                    let SiteEffectFacts::Call(facts) = body_facts
                        .site(*site_id)
                        .expect("source-slice dynamic call site 应带 published Call facts")
                    else {
                        panic!("source-slice dynamic call site 必须对应 Call facts");
                    };
                    if facts.target_mode() == CallTargetMode::KnownInstance {
                        continue;
                    }
                    return (*site_id, facts.clone());
                }
            }
        }

        panic!("应找到一个 non-boundary source-slice dynamic call site");
    }

    fn clone_resume_interface_with_methods(
        interface: &LateLoweredResumeInterface,
        methods: Vec<LateLoweredResumeMethod>,
    ) -> LateLoweredResumeInterface {
        LateLoweredResumeInterface::new(
            interface.interface_id(),
            interface.effect_family().clone(),
            interface.return_step_schema(),
            methods,
        )
    }

    fn single_case_worker_program_with_ping_method_order(
        inputs: &FixtureAbiInputs,
        method_case_order: &[CaseTag],
    ) -> LateLoweredProgram {
        let program = &inputs.abi_visibility_program;
        let callable = program
            .callable("fixtures.build.singleCaseWorker")
            .expect("callable 应存在");
        let step_type = program
            .step_type(callable.step_schema())
            .expect("step type 应存在");
        let ping_interface = program
            .resume_packings()
            .iter()
            .find(|interface| interface.effect_family().effect_fqn() == "fixtures.build.Ping")
            .expect("应存在 Ping resume packing");
        let methods = method_case_order
            .iter()
            .map(|case_tag| {
                ping_interface
                    .methods()
                    .iter()
                    .find(|method| method.case_tag() == *case_tag)
                    .cloned()
                    .unwrap_or_else(|| {
                        let step_case = step_type
                            .case(*case_tag)
                            .expect("method case 应可回查 step shell");
                        LateLoweredResumeMethod::new(
                            step_case.case_tag(),
                            step_case.concrete_op_key().clone(),
                            step_case.continuation_contract(),
                        )
                    })
            })
            .collect::<Vec<_>>();
        let resume_interfaces = program
            .resume_packings()
            .iter()
            .map(|candidate| {
                if candidate.interface_id() == ping_interface.interface_id() {
                    clone_resume_interface_with_methods(candidate, methods.clone())
                } else {
                    candidate.clone()
                }
            })
            .collect();

        LateLoweredProgram::new(
            program.step_types().to_vec(),
            resume_interfaces,
            program.continuation_objects().to_vec(),
            program.callables().to_vec(),
        )
    }

    fn resume_method_for_case(
        step_type: &LateLoweredStepType,
        case_tag: CaseTag,
    ) -> LateLoweredResumeMethod {
        let step_case = step_type
            .case(case_tag)
            .expect("method case 应可回查 step shell");
        LateLoweredResumeMethod::new(
            step_case.case_tag(),
            step_case.concrete_op_key().clone(),
            step_case.continuation_contract(),
        )
    }

    fn next_resume_interface_id(program: &LateLoweredProgram) -> ResumeInterfaceId {
        let next = program
            .resume_packings()
            .iter()
            .map(|interface| interface.interface_id().as_u32())
            .max()
            .map(|raw| raw.saturating_add(1))
            .unwrap_or(0);
        ResumeInterfaceId::new(next)
    }

    fn unit_worker_program_with_ping_interface(inputs: &FixtureAbiInputs) -> LateLoweredProgram {
        let program = &inputs.abi_visibility_program;
        let callable = program
            .callable("fixtures.build.unitWorker")
            .expect("callable 应存在");
        let step_type = program
            .step_type(callable.step_schema())
            .expect("step type 应存在");
        let ping_method = resume_method_for_case(step_type, CaseTag::new(0));
        let ping_interface_id = program
            .resume_packings()
            .iter()
            .find(|interface| interface.effect_family().effect_fqn() == "fixtures.build.Ping")
            .map(LateLoweredResumeInterface::interface_id)
            .unwrap_or_else(|| next_resume_interface_id(program));
        let ping_interface = LateLoweredResumeInterface::new(
            ping_interface_id,
            ping_method.concrete_op_key().effect_family().clone(),
            callable.step_schema(),
            vec![ping_method],
        );

        let resume_interfaces = program
            .resume_packings()
            .iter()
            .filter(|interface| interface.interface_id() != ping_interface_id)
            .cloned()
            .chain(std::iter::once(ping_interface))
            .collect();
        let callables = program
            .callables()
            .iter()
            .map(|candidate| {
                if candidate.step_schema() == callable.step_schema() {
                    clone_callable_with_interfaces(candidate, vec![ping_interface_id])
                } else {
                    candidate.clone()
                }
            })
            .collect();
        let continuation_objects = program
            .continuation_objects()
            .iter()
            .map(|candidate| {
                if candidate.object_id() == callable.continuation_object() {
                    clone_continuation_object_with_interfaces(candidate, vec![ping_interface_id])
                } else {
                    candidate.clone()
                }
            })
            .collect();

        LateLoweredProgram::new(
            program.step_types().to_vec(),
            resume_interfaces,
            continuation_objects,
            callables,
        )
    }

    #[test]
    fn refactor_llvm_source_slice_classification_rejects_missing_handoff() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let mut removed = false;
                let callables = program
                    .callables()
                    .iter()
                    .map(|callable| {
                        if !removed && !callable.source_statement_classifications().is_empty() {
                            removed = true;
                            clone_callable_with_source_statement_classifications(
                                callable,
                                Vec::new(),
                            )
                        } else {
                            callable.clone()
                        }
                    })
                    .collect();
                assert!(
                    removed,
                    "fixture 应发布至少一个 source statement classification"
                );
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 classification handoff 必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(message.contains("source-slice statement"));
                assert!(message.contains("classification"));
            },
        );
    }

    #[test]
    fn refactor_llvm_step_layout_keeps_canonical_case_set_for_single_case_callable() {
        with_fixture_query(
            "effect_refactor_step_enum_single_case.scoop",
            |inputs, query, module| {
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("fixtures.build.singleCaseWorker")
                    .expect("callable 应存在");
                assert_eq!(callable.impl_plan(), ImplPlan::SingleCase(CaseTag::new(0)));

                let step_layout = query
                    .step_layout(callable.step_schema())
                    .expect("step layout 应可查询");
                assert_eq!(step_layout.complete_variant().tag_value(), 0);
                assert_eq!(step_layout.cases().len(), 3);
                assert_eq!(
                    step_layout
                        .case_layout(CaseTag::new(0))
                        .expect("case0 应存在")
                        .variant()
                        .tag_value(),
                    1
                );
                assert_eq!(
                    step_layout
                        .case_layout(CaseTag::new(1))
                        .expect("case1 应存在")
                        .variant()
                        .tag_value(),
                    2
                );
                assert_eq!(
                    step_layout
                        .case_layout(CaseTag::new(2))
                        .expect("runtime-error case 应存在")
                        .variant()
                        .tag_value(),
                    3
                );
                assert!(
                    module
                        .get_global(step_layout.complete_tag_constant_name())
                        .is_some()
                );
                assert!(
                    module
                        .get_global(
                            step_layout
                                .case_layout(CaseTag::new(1))
                                .expect("case1 应存在")
                                .tag_constant_name(),
                        )
                        .is_some()
                );
                assert!(
                    module
                        .get_global(
                            step_layout
                                .case_layout(CaseTag::new(2))
                                .expect("runtime-error case 应存在")
                                .tag_constant_name(),
                        )
                        .is_some()
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_frame_layout_preserves_slot_indices_and_system_fields() {
        with_fixture_query(
            "effect_refactor_step_enum_single_case.scoop",
            |inputs, query, _module| {
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("fixtures.build.singleCaseWorker")
                    .expect("callable 应存在");
                let frame_layout = query
                    .frame_layout(callable.step_schema())
                    .expect("frame layout 应可查询");

                assert_eq!(
                    frame_layout.fields()[0].kind(),
                    RefactorFrameFieldKind::Header
                );
                for (ordinal, slot) in callable.frame_schema().slots().iter().enumerate() {
                    let expected_field_index = ordinal as u32 + 1;
                    assert_eq!(
                        frame_layout.field_index_for_slot(slot.slot_id()),
                        Some(expected_field_index)
                    );
                    if let LateLoweredFrameSlotKind::System(kind) = slot.kind() {
                        assert_eq!(
                            frame_layout.field_index_for_system(kind),
                            Some(expected_field_index)
                        );
                    }
                }
                for required in [
                    SystemSlotKind::StateTag,
                    SystemSlotKind::ResumePayloadCarrier,
                    SystemSlotKind::CleanupFlag,
                    SystemSlotKind::OneShotFlag,
                    SystemSlotKind::CompletionTag,
                ] {
                    assert!(
                        frame_layout.field_index_for_system(required).is_some(),
                        "frame layout 缺少系统字段 {required:?}"
                    );
                }
            },
        );
    }

    #[test]
    fn refactor_llvm_continuation_layout_keeps_full_method_set() {
        with_fixture_query_result(
            "effect_refactor_step_enum_single_case.scoop",
            |inputs| {
                single_case_worker_program_with_ping_method_order(
                    inputs,
                    &[CaseTag::new(0), CaseTag::new(1)],
                )
            },
            |inputs, result, module| {
                let query = result.expect("published full method set 应可物化 ABI");
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("fixtures.build.singleCaseWorker")
                    .expect("callable 应存在");
                let continuation_layout = query
                    .continuation_layout(callable.continuation_object())
                    .expect("continuation layout 应可查询");
                let callable_layout = query
                    .callable_layout(callable.step_schema())
                    .expect("callable layout 应可查询");
                let interface_id = *callable_layout
                    .resume_packings()
                    .iter()
                    .find(|interface_id| {
                        query
                            .resume_packing_layout(**interface_id)
                            .is_some_and(|interface| {
                                interface.packing_family_fqn() == "fixtures.build.Ping"
                            })
                    })
                    .expect("应存在 Ping resume packing");
                let interface_layout = query
                    .resume_packing_layout(interface_id)
                    .expect("resume packing layout 应可查询");

                assert_eq!(interface_layout.methods().len(), 2);
                assert_eq!(
                    interface_layout
                        .method(CaseTag::new(0))
                        .expect("case0 method 应存在")
                        .vtable_index(),
                    0
                );
                assert_eq!(
                    interface_layout
                        .method(CaseTag::new(1))
                        .expect("case1 method 应存在")
                        .vtable_index(),
                    1
                );
                assert!(
                    continuation_layout
                        .field_index_for_packing(interface_id)
                        .is_some()
                );
                assert!(
                    module
                        .get_function(
                            interface_layout
                                .method(CaseTag::new(1))
                                .expect("case1 method 应存在")
                                .symbol_name(),
                        )
                        .is_some()
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_continuation_layout_preserves_published_packing_order() {
        with_fixture_query_result(
            "effect_refactor_step_enum_single_case.scoop",
            |inputs| {
                let program = single_case_worker_program_with_ping_method_order(
                    inputs,
                    &[CaseTag::new(0), CaseTag::new(1)],
                );
                let callable = program
                    .callable("fixtures.build.singleCaseWorker")
                    .expect("callable 应存在");
                let continuation_object = program
                    .continuation_object(callable.continuation_object())
                    .expect("continuation object 应存在");
                let step_type = program
                    .step_type(callable.step_schema())
                    .expect("step type 应存在");
                let ping_interface = program
                    .resume_packings()
                    .iter()
                    .find(|interface| {
                        interface.effect_family().effect_fqn() == "fixtures.build.Ping"
                    })
                    .expect("应存在 Ping resume packing");
                let raise_interface_id = next_resume_interface_id(&program);
                let raise_method = resume_method_for_case(step_type, CaseTag::new(2));
                let raise_interface = LateLoweredResumeInterface::new(
                    raise_interface_id,
                    raise_method.concrete_op_key().effect_family().clone(),
                    callable.step_schema(),
                    vec![raise_method],
                );
                let reversed_interfaces = vec![raise_interface_id, ping_interface.interface_id()];
                let resume_interfaces = program
                    .resume_packings()
                    .iter()
                    .cloned()
                    .chain(std::iter::once(raise_interface))
                    .collect();

                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == callable.step_schema() {
                            clone_callable_with_interfaces(candidate, reversed_interfaces.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                let continuation_objects = program
                    .continuation_objects()
                    .iter()
                    .map(|candidate| {
                        if candidate.object_id() == continuation_object.object_id() {
                            clone_continuation_object_with_interfaces(
                                candidate,
                                reversed_interfaces.clone(),
                            )
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();

                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    resume_interfaces,
                    continuation_objects,
                    callables,
                )
            },
            |inputs, result, _module| {
                let query = result.expect("reordered published resume packings 应仍可物化 ABI");
                let callable = inputs
                    .abi_visibility_program
                    .callable("fixtures.build.singleCaseWorker")
                    .expect("singleCaseWorker callable 应存在");
                let callable_layout = query
                    .callable_layout_by_version_key(callable.body_version_key())
                    .expect("callable layout 应可查询");
                let ping_interface_id = callable_layout
                    .resume_packings()
                    .iter()
                    .find(|interface_id| {
                        query
                            .resume_packing_layout(**interface_id)
                            .is_some_and(|interface| {
                                interface.packing_family_fqn() == "fixtures.build.Ping"
                            })
                    })
                    .copied()
                    .expect("应存在 Ping resume packing");
                let raise_interface_id = callable_layout
                    .resume_packings()
                    .iter()
                    .find(|interface_id| {
                        query
                            .resume_packing_layout(**interface_id)
                            .is_some_and(|interface| {
                                interface.packing_family_fqn() == "scoop.core.Raise"
                            })
                    })
                    .copied()
                    .expect("应存在 Raise resume packing");
                let expected_order = vec![raise_interface_id, ping_interface_id];

                assert_eq!(callable_layout.resume_packings(), expected_order.as_slice());

                let continuation_layout = query
                    .continuation_layout(callable_layout.continuation_object())
                    .expect("continuation layout 应可查询");
                let first_index = continuation_layout
                    .field_index_for_packing(expected_order[0])
                    .expect("首个 published packing 应有 field");
                let second_index = continuation_layout
                    .field_index_for_packing(expected_order[1])
                    .expect("次个 published packing 应有 field");
                assert!(
                    first_index < second_index,
                    "continuation field 顺序必须跟随 published packing 顺序"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_continuation_layout_preserves_authoritative_method_order() {
        with_fixture_query_result(
            "effect_refactor_step_enum_single_case.scoop",
            |inputs| {
                single_case_worker_program_with_ping_method_order(
                    inputs,
                    &[CaseTag::new(1), CaseTag::new(0)],
                )
            },
            |inputs, result, _module| {
                let query = result.expect("reordered authoritative methods 应仍可物化 ABI");
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("fixtures.build.singleCaseWorker")
                    .expect("callable 应存在");
                let interface_id = query
                    .callable_layout(callable.step_schema())
                    .expect("callable layout 应可查询")
                    .resume_packings()
                    .iter()
                    .find(|interface_id| {
                        query
                            .resume_packing_layout(**interface_id)
                            .is_some_and(|interface| {
                                interface.packing_family_fqn() == "fixtures.build.Ping"
                            })
                    })
                    .copied()
                    .expect("应存在 Ping resume packing");
                let interface_layout = query
                    .resume_packing_layout(interface_id)
                    .expect("resume packing layout 应可查询");

                assert_eq!(
                    interface_layout
                        .method(CaseTag::new(1))
                        .expect("case1 method 应存在")
                        .vtable_index(),
                    0
                );
                assert_eq!(
                    interface_layout
                        .method(CaseTag::new(0))
                        .expect("case0 method 应存在")
                        .vtable_index(),
                    1
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_continuation_layout_rejects_missing_published_packing() {
        with_fixture_query_result(
            "effect_refactor_step_enum_single_case.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let callable = program
                    .callable("fixtures.build.singleCaseWorker")
                    .expect("callable 应存在");
                let dropped_interface = callable
                    .resume_packings()
                    .first()
                    .copied()
                    .expect("fixture 应至少发布一个 packing");
                let resume_interfaces = program
                    .resume_packings()
                    .iter()
                    .filter(|interface| interface.interface_id() != dropped_interface)
                    .cloned()
                    .collect();

                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    resume_interfaces,
                    program.continuation_objects().to_vec(),
                    program.callables().to_vec(),
                )
            },
            |inputs, result, _module| {
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("fixtures.build.singleCaseWorker")
                    .expect("callable 应存在");
                let dropped_interface = callable
                    .resume_packings()
                    .first()
                    .copied()
                    .expect("fixture 应至少发布一个 packing");
                let err = match result {
                    Ok(_) => panic!("缺失 published packing 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains(&format!("resume packing {}", dropped_interface.as_u32())),
                    "错误消息应指出缺失的 published packing: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_continuation_layout_rejects_missing_authoritative_method() {
        with_fixture_query_result(
            "effect_refactor_step_enum_single_case.scoop",
            |inputs| single_case_worker_program_with_ping_method_order(inputs, &[CaseTag::new(0)]),
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 authoritative method 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("authoritative method cases [1]"),
                    "错误消息应指出缺失的 authoritative case tag: {message}"
                );
                assert!(
                    message.contains("effect family `fixtures.build.Ping`"),
                    "错误消息应指出缺失方法所属的 interface family: {message}"
                );
                assert!(
                    message.contains("step schema"),
                    "错误消息应指出缺失方法对应的 step schema: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_call_target_query_preserves_known_instance_direct_entries() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_handle_hidden_suspend_virtual_helper_basic.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("known-instance direct call 应可回查 callable entry");
                let program = inputs.effect_lowered_stage_output.program();
                let main = program.callable("main").expect("main callable 应存在");
                let helper = program.callable("helper").expect("helper callable 应存在");
                let callable = query
                    .callable_layout_by_version_key(main.body_version_key())
                    .expect("main callable layout 应存在");
                let boundary = site_boundary(main, BoundarySiteKind::Call);
                let lowering = call_boundary_lowering(boundary);

                assert_eq!(
                    lowering.facts().target_mode(),
                    CallTargetMode::KnownInstance
                );
                let site_id = boundary_site_id(boundary);
                let RefactorCallTargetQuery::KnownInstance(target) = query
                    .call_target_layout(callable.step_schema(), site_id, lowering.facts())
                    .expect("known-instance call target 应可回查 published direct entry")
                else {
                    panic!("known-instance direct call 不应走 dynamic invoke contract");
                };
                assert_eq!(target.root_fqn(), "helper");
                assert_eq!(target.body_version_key(), helper.body_version_key());
                assert_eq!(
                    target.dynamic_entry().invoke_args_tuple_ty(),
                    lowering.facts().invoke_args_tuple_ty()
                );
                assert_eq!(
                    target.dynamic_entry().return_step_schema(),
                    lowering.facts().callee_schema()
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_callable_version_query_resolves_layout_by_body_version_key() {
        with_fixture_query(
            "effect_refactor_dynamic_entry_publication_emit_llvm.scoop",
            |inputs, query, _module| {
                for callable in inputs.abi_visibility_program.callables() {
                    let layout = query
                        .callable_layout_by_version_key(callable.body_version_key())
                        .expect("published callable version 应可按 body version key 回查");
                    assert_eq!(layout.root_fqn(), callable.root_fqn());
                    assert_eq!(layout.step_schema(), callable.step_schema());
                    assert_eq!(layout.continuation_object(), callable.continuation_object());
                }
            },
        );
    }

    #[test]
    fn refactor_llvm_known_instance_version_selection_resolves_generic_instance_keys() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query =
                    result.expect("generic known-instance callable 应可回查 callable version");
                let println_int = inputs
                    .abi_visibility_program
                    .callables()
                    .iter()
                    .find(|callable| callable.root_fqn() == "scoop.core.println::<Int>")
                    .expect("fixture 应发布 println::<Int> callable shell");
                let facts = CallSiteEffectFacts::new(
                    CallSiteKind::Direct,
                    CallSiteTarget::KnownInstance(println_int.instance_key().clone()),
                    println_int.dynamic_invoke_entry().invoke_args_tuple_ty(),
                    println_int.step_schema(),
                    crate::effect_facts::CaseSet::new(println_int.step_schema(), Vec::new()),
                    EffectPrecision::Precise,
                );

                let RefactorCallTargetQuery::KnownInstance(target) = query
                    .call_target_layout(println_int.step_schema(), SiteId::from_raw(900), &facts)
                    .expect("generic known-instance selector 应按 instance key + callee step schema 解析")
                else {
                    panic!("generic known-instance call 不应走 dynamic invoke contract");
                };

                assert_eq!(target.root_fqn(), println_int.root_fqn());
                assert_eq!(target.body_version_key(), println_int.body_version_key());
                assert_eq!(target.surface_instance(), println_int.instance_key());
            },
        );
    }

    #[test]
    fn refactor_llvm_boundary_operand_contract_resolves_direct_call_anchor_and_args() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("direct call boundary operand contract 应成功发布");
                let main = inputs
                    .abi_visibility_program
                    .callable("main")
                    .expect("main callable 应存在");
                let boundary = site_boundary(main, BoundarySiteKind::Call);
                let lowering = call_boundary_lowering(boundary);
                let site_id = boundary_site_id(boundary);
                let layout = query
                    .call_boundary_operand_layout(
                        main.step_schema(),
                        site_id,
                        lowering.operand_contract(),
                    )
                    .expect("direct call boundary 应可回查 published operand contract");
                let RefactorCallTargetQuery::KnownInstance(_) = query
                    .call_target_layout(main.step_schema(), site_id, lowering.facts())
                    .expect("direct call target contract 应成功")
                else {
                    panic!("known-instance direct call 不应走 dynamic invoke contract");
                };

                assert_eq!(layout.owner_step_schema(), main.step_schema());
                assert_eq!(layout.site_id(), site_id);
                assert!(matches!(
                    layout.contract().source_consumption(),
                    LateLoweredBoundarySourceConsumption::Statement {
                        consumes_last_statement: true,
                        ..
                    }
                ));
                assert!(layout.contract().carrier_source().is_none());
                assert_eq!(layout.contract().arg_sources().len(), 1);
                assert_eq!(
                    inputs
                        .effect_lowered_stage_output
                        .types()
                        .display(layout.contract().arg_sources()[0].source_ty())
                        .to_string(),
                    "Bool"
                );
                assert!(matches!(
                    layout.contract().arg_sources()[0].value(),
                    LateLoweredOperandValueSource::Local(_)
                        | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Bool(_))
                ));
                assert!(layout.contract().arg_sources()[0].span().is_some());
            },
        );
    }

    #[test]
    fn refactor_llvm_boundary_operand_contract_resolves_dynamic_call_carrier() {
        with_phase_fixture_query_result(
            "effect_facts",
            "dynamic_fallback_widening.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("dynamic call boundary operand contract 应成功发布");
                let call_value = inputs
                    .abi_visibility_program
                    .callable("sample.callValue")
                    .expect("sample.callValue callable 应存在");
                let boundary = site_boundary(call_value, BoundarySiteKind::Call);
                let lowering = call_boundary_lowering(boundary);
                let site_id = boundary_site_id(boundary);
                let layout = query
                    .call_boundary_operand_layout(
                        call_value.step_schema(),
                        site_id,
                        lowering.operand_contract(),
                    )
                    .expect("dynamic call boundary 应可回查 published operand contract");
                let RefactorCallTargetQuery::DynamicInvoke(_) = query
                    .call_target_layout(call_value.step_schema(), site_id, lowering.facts())
                    .expect("dynamic call target contract 应成功")
                else {
                    panic!("non-KnownInstance call 应走 dynamic invoke contract");
                };

                assert!(matches!(
                    layout.contract().source_consumption(),
                    LateLoweredBoundarySourceConsumption::Statement { .. }
                ));
                assert_eq!(layout.contract().arg_sources().len(), 0);
                assert!(matches!(
                    layout
                        .contract()
                        .carrier_source()
                        .expect("dynamic call 应发布 carrier source")
                        .value(),
                    LateLoweredOperandValueSource::Local(_)
                ));
            },
        );
    }

    #[test]
    fn refactor_llvm_boundary_operand_contract_resolves_perform_and_resume_sources() {
        with_phase_fixture_query_result(
            "effect_facts",
            "handle_perform.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("perform boundary operand contract 应成功发布");
                let main = inputs
                    .abi_visibility_program
                    .callable("a.main")
                    .expect("a.main callable 应存在");
                let boundary = site_boundary(main, BoundarySiteKind::Perform);
                let lowering = perform_boundary_lowering(boundary);
                let site_id = boundary_site_id(boundary);
                let layout = query
                    .perform_boundary_operand_layout(
                        main.step_schema(),
                        site_id,
                        lowering.operand_contract(),
                    )
                    .expect("perform boundary 应可回查 published operand contract");

                assert!(matches!(
                    layout.contract().source_consumption(),
                    LateLoweredBoundarySourceConsumption::Terminator { .. }
                ));
                assert_eq!(layout.contract().payload_sources().len(), 1);
                assert!(matches!(
                    layout.contract().payload_sources()[0].value(),
                    LateLoweredOperandValueSource::Local(_)
                        | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Int)
                ));
                assert!(layout.contract().payload_sources()[0].span().is_some());
            },
        );

        with_phase_fixture_query_result(
            "effect_facts",
            "dispatch_and_resume_call.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("resume boundary operand contract 应成功发布");
                let callable = inputs
                    .abi_visibility_program
                    .callable("fixtures.mir.resumeBoom")
                    .expect("fixtures.mir.resumeBoom callable 应存在");
                let boundary = site_boundary(callable, BoundarySiteKind::Resume);
                let lowering = resume_boundary_lowering(boundary);
                let site_id = boundary_site_id(boundary);
                let layout = query
                    .resume_boundary_operand_layout(
                        callable.step_schema(),
                        site_id,
                        lowering.operand_contract(),
                    )
                    .expect("resume boundary 应可回查 published operand contract");

                assert!(matches!(
                    layout.contract().source_consumption(),
                    LateLoweredBoundarySourceConsumption::Statement {
                        consumes_last_statement: true,
                        ..
                    }
                ));
                assert!(matches!(
                    layout.contract().continuation_source().value(),
                    LateLoweredOperandValueSource::Local(_)
                ));
                assert_eq!(layout.contract().arg_sources().len(), 1);
                assert!(matches!(
                    layout.contract().arg_sources()[0].value(),
                    LateLoweredOperandValueSource::Local(_)
                        | LateLoweredOperandValueSource::Const(crate::mir::ConstValue::Int)
                ));
                assert!(layout.contract().arg_sources()[0].span().is_some());
                let route = layout.contract().underlying_continuation_route();
                assert_eq!(
                    route.continuation_schema(),
                    lowering.facts().continuation_schema()
                );
                assert!(matches!(
                    route.publication(),
                    LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                        owner_version_key,
                        owner_continuation_object,
                        site_id: route_site_id,
                    } if owner_version_key == callable.body_version_key()
                        && *owner_continuation_object == callable.continuation_object()
                        && *route_site_id == site_id
                ));
            },
        );

        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query =
                    result.expect("readback resume boundary provenance 应成功发布到 LLVM query");
                let callable = inputs
                    .abi_visibility_program
                    .callable("main")
                    .expect("main callable 应存在");
                let handle_state = handle_dispatch_state(callable, SiteId::from_raw(0));
                let LateLoweredStateTerminator::HandleDispatch { contract, .. } =
                    handle_state.terminator()
                else {
                    panic!("main site0 应保持 HandleDispatch terminator");
                };
                let binder = contract.handled_arms()[0]
                    .continuation_binder()
                    .expect("Ask handle arm 应发布 continuation binder");

                let routes = callable
                    .boundary_map()
                    .entries()
                    .iter()
                    .filter_map(|boundary| match boundary.lowering() {
                        Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                            Some((boundary_site_id(boundary), lowering))
                        }
                        _ => None,
                    })
                    .map(|(site_id, lowering)| {
                        let layout = query
                            .resume_boundary_operand_layout(
                                callable.step_schema(),
                                site_id,
                                lowering.operand_contract(),
                            )
                            .unwrap_or_else(|err| {
                                panic!(
                                    "resume site{} 应可回查 boundary operand contract: {err}",
                                    site_id.as_u32()
                                )
                            });
                        let route = layout.contract().underlying_continuation_route();
                        (site_id, route)
                    })
                    .collect::<Vec<_>>();

                assert_eq!(
                    routes
                        .iter()
                        .map(|(site_id, _)| site_id.as_u32())
                        .collect::<Vec<_>>(),
                    vec![25, 30, 35, 40]
                );
                for (_site_id, route) in routes {
                    assert_eq!(route.continuation_schema(), binder.continuation_schema());
                    assert!(matches!(
                        route.publication(),
                        LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                            owner_continuation_object,
                            site_id,
                            arm_ordinal,
                            handled_case,
                            ..
                        } if *owner_continuation_object == callable.continuation_object()
                            && site_id.as_u32() == 0
                            && *arm_ordinal == 0
                            && *handled_case == contract.handled_arms()[0].handled_case()
                    ));
                }
            },
        );
    }

    #[test]
    fn refactor_llvm_resume_payload_binding_resolves_boundary_and_state_queries() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query =
                    result.expect("call/resume boundary 的 resumed local/home contract 应成功发布");

                let main = inputs
                    .abi_visibility_program
                    .callable("main")
                    .expect("main callable 应存在");
                let call_boundary = site_boundary(main, BoundarySiteKind::Call);
                let call_binding = main
                    .frame_schema()
                    .resume_payload_binding(call_boundary.boundary_id())
                    .expect("call boundary 应发布 resumed local/home binding");
                let call_layout = query
                    .resume_payload_binding_layout(main.step_schema(), call_binding)
                    .expect("call boundary 应可回查 resumed local/home contract");
                let call_frame_layout = query
                    .frame_layout(main.step_schema())
                    .expect("callable frame layout 应可查询");
                let call_home_slot = call_binding
                    .consumer_frame_slot()
                    .expect("call boundary 应发布 frame home slot");

                assert_eq!(call_layout.boundary_id(), call_boundary.boundary_id());
                assert_eq!(call_layout.resume_state(), call_boundary.resume_state());
                assert_eq!(call_layout.consumer_local(), call_binding.consumer_local());
                assert_eq!(
                    call_layout.frame_field_index(),
                    call_frame_layout.field_index_for_slot(call_home_slot),
                );

                let run = inputs
                    .abi_visibility_program
                    .callable("run")
                    .expect("run callable 应存在");
                let resume_boundary = site_boundary(run, BoundarySiteKind::Resume);
                let resume_binding = run
                    .frame_schema()
                    .resume_payload_binding(resume_boundary.boundary_id())
                    .expect("resume boundary 应发布 resumed local/home binding");
                let resume_layout = query
                    .resume_payload_binding_layout(run.step_schema(), resume_binding)
                    .expect("resume boundary 应可回查 resumed local/home contract");
                let state_layout = query
                    .resume_payload_binding_for_state(
                        run.step_schema(),
                        resume_boundary.resume_state(),
                    )
                    .expect("resume state 应可直接回查 resumed local/home contract");

                assert_eq!(
                    resume_layout.consumer_local(),
                    resume_binding.consumer_local()
                );
                assert_eq!(
                    state_layout.consumer_frame_slot(),
                    resume_binding.consumer_frame_slot(),
                );
            },
        );

        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result
                    .expect("perform/runtime-error 的 resumed local/home contract 应成功发布");

                let fetch = inputs
                    .abi_visibility_program
                    .callable("fetch")
                    .expect("fetch callable 应存在");
                let perform_boundary = site_boundary(fetch, BoundarySiteKind::Perform);
                let perform_binding = fetch
                    .frame_schema()
                    .resume_payload_binding(perform_boundary.boundary_id())
                    .expect("perform boundary 应发布 resumed local/home binding");
                let perform_layout = query
                    .resume_payload_binding_layout(fetch.step_schema(), perform_binding)
                    .expect("perform boundary 应可回查 resumed local/home contract");
                let fetch_frame_layout = query
                    .frame_layout(fetch.step_schema())
                    .expect("fetch frame layout 应可查询");
                let perform_home_slot = perform_binding
                    .consumer_frame_slot()
                    .expect("perform boundary 应发布 frame home slot");

                assert_eq!(perform_layout.boundary_id(), perform_boundary.boundary_id());
                assert_eq!(
                    perform_layout.resume_state(),
                    perform_boundary.resume_state()
                );
                assert_eq!(
                    perform_layout.frame_field_index(),
                    fetch_frame_layout.field_index_for_slot(perform_home_slot),
                );

                let main = inputs
                    .abi_visibility_program
                    .callable("main")
                    .expect("main callable 应存在");
                let runtime_error_boundary = main
                    .boundary_map()
                    .entries()
                    .iter()
                    .find(|boundary| {
                        matches!(
                            boundary.source(),
                            LateLoweredBoundarySource::RuntimeError { origin_site }
                                if origin_site == SiteId::from_raw(25)
                        )
                    })
                    .expect("site25 的 runtime-error boundary 应存在");
                let runtime_error_binding = main
                    .frame_schema()
                    .resume_payload_binding(runtime_error_boundary.boundary_id())
                    .expect("runtime-error boundary 应发布 resumed local/home binding");
                let runtime_error_layout = query
                    .resume_payload_binding_layout(main.step_schema(), runtime_error_binding)
                    .expect("runtime-error boundary 应可回查 resumed local/home contract");
                let state_layout = query
                    .resume_payload_binding_for_state(
                        main.step_schema(),
                        runtime_error_boundary.resume_state(),
                    )
                    .expect("runtime-error resume state 应可直接回查 resumed local/home contract");

                assert_eq!(
                    runtime_error_layout.consumer_local(),
                    runtime_error_binding.consumer_local(),
                );
                assert_eq!(
                    state_layout.consumer_frame_slot(),
                    runtime_error_binding.consumer_frame_slot(),
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_resume_payload_binding_rejects_missing_contract() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let fetch = program.callable("fetch").expect("fetch callable 应存在");
                let frame_schema =
                    LateLoweredFrameSchema::new(fetch.frame_schema().slots().to_vec())
                        .with_completion_payload_bindings(
                            fetch.frame_schema().completion_payload_bindings().to_vec(),
                        );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == fetch.step_schema() {
                            clone_callable_with_frame_schema(candidate, frame_schema.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 resumed local/home contract 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("resumed local/home contract"),
                    "错误消息应指出缺失的是 resumed local/home contract: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_resume_payload_binding_rejects_runtime_error_binding_drift() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let main = program.callable("main").expect("main callable 应存在");
                let runtime_error_boundary = main
                    .boundary_map()
                    .entries()
                    .iter()
                    .find(|boundary| {
                        matches!(
                            boundary.source(),
                            LateLoweredBoundarySource::RuntimeError { origin_site }
                                if origin_site == SiteId::from_raw(25)
                        )
                    })
                    .expect("site25 的 runtime-error boundary 应存在");
                let replacement = main
                    .frame_schema()
                    .resume_payload_bindings()
                    .iter()
                    .find(|binding| {
                        binding.boundary_id() != runtime_error_boundary.boundary_id()
                            && binding.resume_state() != runtime_error_boundary.resume_state()
                    })
                    .copied()
                    .expect("应存在可用于构造 drift 的其它 resumed local/home binding");
                let bindings = main
                    .frame_schema()
                    .resume_payload_bindings()
                    .iter()
                    .copied()
                    .map(|binding| {
                        if binding.boundary_id() == runtime_error_boundary.boundary_id() {
                            LateLoweredResumePayloadBinding::new(
                                binding.boundary_id(),
                                binding.resume_state(),
                                replacement.consumer_local(),
                                replacement.consumer_frame_slot(),
                            )
                        } else {
                            binding
                        }
                    })
                    .collect();
                let frame_schema =
                    LateLoweredFrameSchema::new(main.frame_schema().slots().to_vec())
                        .with_resume_payload_bindings(bindings)
                        .with_completion_payload_bindings(
                            main.frame_schema().completion_payload_bindings().to_vec(),
                        );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == main.step_schema() {
                            clone_callable_with_frame_schema(candidate, frame_schema.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("runtime-error binding 漂移时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("resumed local/home contract")
                        && (message.contains("runtime-error boundary")
                            || message.contains("漂移")
                            || message.contains("冲突")),
                    "错误消息应指出 runtime-error resumed local/home contract 漂移: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_completion_payload_contract_resolves_return_state_query() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("completion payload contract 应成功发布到 LLVM query");
                let run = inputs
                    .abi_visibility_program
                    .callable("run")
                    .expect("run callable 应存在");
                let binding = run
                    .frame_schema()
                    .completion_payload_bindings()
                    .iter()
                    .find(|binding| !binding.payload_source().is_unit())
                    .expect("run(): Int 应发布 non-Unit completion payload binding");
                let layout = query
                    .completion_payload_binding_layout(run.step_schema(), binding)
                    .expect("return state 应可回查 completion payload contract");
                let state_layout = query
                    .completion_payload_binding_for_state(run.step_schema(), binding.return_state())
                    .expect("return state 应可直接回查 completion payload contract");
                let frame_layout = query
                    .frame_layout(run.step_schema())
                    .expect("run frame layout 应可查询");

                assert_eq!(layout.owner_step_schema(), run.step_schema());
                assert_eq!(layout.return_state(), binding.return_state());
                assert_eq!(layout.complete_state(), run.state_graph().complete_state());
                assert_eq!(state_layout.binding(), binding);
                assert_eq!(layout.payload_source(), binding.payload_source());
                assert_eq!(
                    inputs
                        .effect_lowered_stage_output
                        .types()
                        .display(layout.payload_source().source_ty())
                        .to_string(),
                    "Int"
                );
                assert!(
                    !layout.payload_abi().is_elided(),
                    "Int completion payload 不应在 ABI 中被 elide"
                );
                if let Some(slot_id) = binding.payload_frame_slot() {
                    assert_eq!(
                        layout.frame_field_index(),
                        frame_layout.field_index_for_slot(slot_id),
                    );
                }
            },
        );
    }

    #[test]
    fn refactor_llvm_completion_payload_contract_rejects_missing_contract() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let run = program.callable("run").expect("run callable 应存在");
                let frame_schema = LateLoweredFrameSchema::new(run.frame_schema().slots().to_vec())
                    .with_resume_payload_bindings(
                        run.frame_schema().resume_payload_bindings().to_vec(),
                    );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == run.step_schema() {
                            clone_callable_with_frame_schema(candidate, frame_schema.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 completion payload contract 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("completion payload contract"),
                    "错误消息应指出缺失的是 completion payload contract: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_completion_payload_contract_rejects_source_drift() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let run = program.callable("run").expect("run callable 应存在");
                let drifted_bindings = run
                    .frame_schema()
                    .completion_payload_bindings()
                    .iter()
                    .map(|binding| {
                        if binding.payload_source().is_unit() {
                            binding.clone()
                        } else {
                            LateLoweredCompletionPayloadBinding::new(
                                binding.return_state(),
                                binding.complete_state(),
                                LateLoweredCompletionPayloadSource::unit(
                                    binding.payload_source().source_ty(),
                                ),
                                binding.payload_frame_slot(),
                            )
                        }
                    })
                    .collect();
                let frame_schema = LateLoweredFrameSchema::new(run.frame_schema().slots().to_vec())
                    .with_resume_payload_bindings(
                        run.frame_schema().resume_payload_bindings().to_vec(),
                    )
                    .with_completion_payload_bindings(drifted_bindings);
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == run.step_schema() {
                            clone_callable_with_frame_schema(candidate, frame_schema.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("completion payload source 漂移时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("completion payload source")
                        || message.contains("completion payload frame home"),
                    "错误消息应指出 completion payload contract 漂移: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_boundary_operand_contract_rejects_ordered_arg_drift() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let main = program.callable("main").expect("main callable 应存在");
                let boundary_map = LateLoweredBoundaryMap::new(
                    main.boundary_map()
                        .entries()
                        .iter()
                        .map(|boundary| {
                            let lowering = match boundary
                                .lowering()
                                .cloned()
                                .expect("boundary 应带 lowering")
                            {
                                LateLoweredBoundaryLowering::Call(lowering) => {
                                    LateLoweredBoundaryLowering::Call(
                                        LateLoweredCallBoundaryLowering::new(
                                            lowering.facts().clone(),
                                            lowering.result_local(),
                                            LateLoweredCallBoundaryOperandContract::new(
                                                lowering.operand_contract().source_consumption(),
                                                None,
                                                Vec::new(),
                                            ),
                                            lowering.dispatch().clone(),
                                            lowering.continuation_compositions().to_vec(),
                                            lowering.consumed_runtime_error_case().cloned(),
                                        ),
                                    )
                                }
                                other => other,
                            };
                            LateLoweredBoundary::new(
                                boundary.boundary_id(),
                                boundary.source(),
                                boundary.owner_state(),
                                boundary.resume_state(),
                            )
                            .with_lowering(lowering)
                        })
                        .collect(),
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == main.step_schema() {
                            clone_callable_with_boundary_map(candidate, boundary_map.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("ordered arg drift 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("ordered args")
                        && (message.contains("contract 非法")
                            || message.contains("单一 source")
                            || message.contains("component")),
                    "错误消息应指出 ordered args contract 漂移: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_boundary_operand_contract_rejects_missing_dynamic_carrier_source() {
        with_phase_fixture_query_result(
            "effect_facts",
            "dynamic_fallback_widening.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let call_value = program
                    .callable("sample.callValue")
                    .expect("sample.callValue callable 应存在");
                let boundary_map = LateLoweredBoundaryMap::new(
                    call_value
                        .boundary_map()
                        .entries()
                        .iter()
                        .map(|boundary| {
                            let lowering = match boundary
                                .lowering()
                                .cloned()
                                .expect("boundary 应带 lowering")
                            {
                                LateLoweredBoundaryLowering::Call(lowering) => {
                                    LateLoweredBoundaryLowering::Call(
                                        LateLoweredCallBoundaryLowering::new(
                                            lowering.facts().clone(),
                                            lowering.result_local(),
                                            LateLoweredCallBoundaryOperandContract::new(
                                                lowering.operand_contract().source_consumption(),
                                                None,
                                                lowering.operand_contract().arg_sources().to_vec(),
                                            ),
                                            lowering.dispatch().clone(),
                                            lowering.continuation_compositions().to_vec(),
                                            lowering.consumed_runtime_error_case().cloned(),
                                        ),
                                    )
                                }
                                other => other,
                            };
                            LateLoweredBoundary::new(
                                boundary.boundary_id(),
                                boundary.source(),
                                boundary.owner_state(),
                                boundary.resume_state(),
                            )
                            .with_lowering(lowering)
                        })
                        .collect(),
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == call_value.step_schema() {
                            clone_callable_with_boundary_map(candidate, boundary_map.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 dynamic carrier source 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("carrier source contract"),
                    "错误消息应指出缺失的是 dynamic carrier source contract: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_boundary_operand_contract_rejects_missing_underlying_continuation_route_publication()
     {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let main = program.callable("main").expect("main callable 应存在");
                let boundary_map = LateLoweredBoundaryMap::new(
                    main.boundary_map()
                        .entries()
                        .iter()
                        .map(|boundary| {
                            let lowering = match boundary
                                .lowering()
                                .cloned()
                                .expect("boundary 应带 lowering")
                            {
                                LateLoweredBoundaryLowering::Resume(lowering) => {
                                    let route = lowering
                                        .operand_contract()
                                        .underlying_continuation_route();
                                    let broken_contract =
                                        crate::effect_lowered::ir::LateLoweredResumeBoundaryOperandContract::new(
                                            lowering.operand_contract().source_consumption(),
                                            lowering.operand_contract().continuation_source().clone(),
                                            lowering.operand_contract().arg_sources().to_vec(),
                                            crate::effect_lowered::ir::LateLoweredContinuationRoute::new(
                                                route.continuation_schema(),
                                                LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                                                    owner_version_key: main.body_version_key().clone(),
                                                    owner_continuation_object: main.continuation_object(),
                                                    site_id: SiteId::from_raw(999),
                                                    arm_ordinal: 0,
                                                    handled_case: CaseTag::new(1),
                                                },
                                            ),
                                        );
                                    LateLoweredBoundaryLowering::Resume(
                                        crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering::new(
                                            lowering.facts().clone(),
                                            lowering.result_local(),
                                            lowering.runtime_error_boundary(),
                                            broken_contract,
                                            lowering.dispatch().clone(),
                                        ),
                                    )
                                }
                                other => other,
                            };
                            LateLoweredBoundary::new(
                                boundary.boundary_id(),
                                boundary.source(),
                                boundary.owner_state(),
                                boundary.resume_state(),
                            )
                            .with_lowering(lowering)
                        })
                        .collect(),
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == main.step_schema() {
                            clone_callable_with_boundary_map(candidate, boundary_map.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => {
                        panic!("缺失 underlying continuation route publication 时必须 fail fast")
                    }
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("underlying continuation route")
                        || message.contains("缺少 publication"),
                    "错误消息应指出 underlying continuation route publication 漂移: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_dynamic_invoke_query_resolves_fun_value_unit_contract() {
        with_phase_fixture_query_result(
            "effect_facts",
            "dynamic_fallback_widening.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query =
                    result.expect("fun-value DynamicFallback 应可物化 dynamic invoke contract");
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("sample.callValue")
                    .expect("sample.callValue callable 应存在");
                let boundary = site_boundary(callable, BoundarySiteKind::Call);
                let lowering = call_boundary_lowering(boundary);

                assert_eq!(
                    lowering.facts().target_mode(),
                    CallTargetMode::DynamicFallback
                );
                let site_id = boundary_site_id(boundary);
                let RefactorCallTargetQuery::DynamicInvoke(layout) = query
                    .call_target_layout(callable.step_schema(), site_id, lowering.facts())
                    .expect("fun-value boundary 应可回查 dynamic invoke contract")
                else {
                    panic!("DynamicFallback fun-value call 应走 dynamic invoke contract");
                };
                assert_eq!(layout.owner_step_schema(), callable.step_schema());
                assert_eq!(layout.site_id(), site_id);
                assert_eq!(layout.target_mode(), CallTargetMode::DynamicFallback);
                assert_eq!(
                    layout.return_step_schema(),
                    lowering.facts().callee_schema()
                );
                assert_eq!(
                    layout.invoke_args_tuple_ty(),
                    lowering.facts().invoke_args_tuple_ty()
                );
                assert!(layout.args_abi().is_elided());
                assert_eq!(layout.param_count(), 1);
                match layout.carrier() {
                    RefactorDynamicInvokeCarrierLayout::ClosureObject(carrier) => {
                        assert_eq!(carrier.object_ty().count_fields(), 3);
                        assert_eq!(carrier.env_field_index(), 1);
                        assert_eq!(carrier.fn_field_index(), 2);
                        assert!(!carrier.receiver_abi().is_elided());
                    }
                    other => {
                        panic!("fun-value dynamic invoke 应发布 closure carrier，而不是 {other:?}")
                    }
                }
            },
        );
    }

    #[test]
    fn refactor_llvm_callable_carrier_layout_resolves_virtual_candidate_set_contracts() {
        with_fixture_query_result(
            "effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query =
                    result.expect("candidate-set virtual helper 应可物化 dynamic invoke contract");
                let callable = inputs
                    .abi_visibility_program
                    .callable("fixtures.build.helper")
                    .expect("fixtures.build.helper callable 应存在");
                let boundary = site_boundary(callable, BoundarySiteKind::Call);
                let lowering = call_boundary_lowering(boundary);

                assert_eq!(lowering.facts().target_mode(), CallTargetMode::CandidateSet);
                let site_id = boundary_site_id(boundary);
                let RefactorCallTargetQuery::DynamicInvoke(layout) = query
                    .call_target_layout(callable.step_schema(), site_id, lowering.facts())
                    .expect("candidate-set virtual boundary 应可回查 dynamic invoke contract")
                else {
                    panic!("CandidateSet virtual call 应走 dynamic invoke contract");
                };
                assert_eq!(layout.target_mode(), CallTargetMode::CandidateSet);
                assert_eq!(layout.param_count(), 1);
                assert!(layout.args_abi().is_elided());
                assert_eq!(
                    layout.return_step_schema(),
                    lowering.facts().callee_schema()
                );
                match layout.carrier() {
                    RefactorDynamicInvokeCarrierLayout::VirtualReceiver(dispatch) => {
                        assert_eq!(
                            inputs
                                .effect_lowered_stage_output
                                .types()
                                .display(dispatch.receiver_ty())
                                .to_string(),
                            "fixtures.build.Base"
                        );
                        assert_eq!(dispatch.owner_fqn(), "fixtures.build.Base");
                        assert_eq!(dispatch.member_name(), "ping");
                        assert!(!dispatch.receiver_abi().is_elided());
                    }
                    other => panic!(
                        "virtual CandidateSet 应发布 receiver-dispatch carrier，而不是 {other:?}"
                    ),
                }
            },
        );
    }

    #[test]
    fn refactor_llvm_dynamic_invoke_query_resolves_non_boundary_virtual_contract() {
        with_fixture_query(
            "effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop",
            |inputs, query, _module| {
                let helper = inputs
                    .abi_visibility_program
                    .callable("fixtures.build.helper")
                    .expect("fixtures.build.helper callable 应存在");
                assert!(
                    helper
                        .boundary_map()
                        .entries()
                        .iter()
                        .all(|boundary| !matches!(
                            boundary.source(),
                            LateLoweredBoundarySource::Site {
                                kind: BoundarySiteKind::Call,
                                ..
                            }
                        )),
                    "pure helper 的 dynamic call 不应被发布成 boundary"
                );

                let (site_id, facts) = source_slice_non_boundary_dynamic_call_site(inputs, helper);
                assert!(
                    facts.resolved_cases().is_empty(),
                    "non-boundary dynamic call 的 resolved cases 应为空"
                );
                let RefactorCallTargetQuery::DynamicInvoke(layout) = query
                    .call_target_layout(helper.step_schema(), site_id, &facts)
                    .expect("non-boundary source-slice dynamic call 应可回查 published dynamic invoke contract")
                else {
                    panic!("non-boundary virtual call 应走 dynamic invoke contract");
                };

                assert_eq!(layout.owner_step_schema(), helper.step_schema());
                assert_eq!(layout.site_id(), site_id);
                assert_eq!(layout.target_mode(), CallTargetMode::CandidateSet);
                assert_eq!(layout.invoke_args_tuple_ty(), facts.invoke_args_tuple_ty());
                assert_eq!(layout.return_step_schema(), facts.callee_schema());
            },
        );
    }

    #[test]
    fn refactor_llvm_callable_carrier_layout_resolves_non_boundary_virtual_contracts() {
        with_fixture_query(
            "effect_refactor_non_boundary_dynamic_call_emit_llvm.scoop",
            |inputs, query, _module| {
                let helper = inputs
                    .abi_visibility_program
                    .callable("fixtures.build.helper")
                    .expect("fixtures.build.helper callable 应存在");
                let (site_id, facts) = source_slice_non_boundary_dynamic_call_site(inputs, helper);
                let RefactorCallTargetQuery::DynamicInvoke(layout) = query
                    .call_target_layout(helper.step_schema(), site_id, &facts)
                    .expect("non-boundary source-slice dynamic call 应可回查 published dynamic invoke contract")
                else {
                    panic!("non-boundary virtual call 应走 dynamic invoke contract");
                };

                assert_eq!(layout.target_mode(), CallTargetMode::CandidateSet);
                assert_eq!(layout.param_count(), 1);
                assert!(layout.args_abi().is_elided());
                match layout.carrier() {
                    RefactorDynamicInvokeCarrierLayout::VirtualReceiver(dispatch) => {
                        assert_eq!(
                            inputs
                                .effect_lowered_stage_output
                                .types()
                                .display(dispatch.receiver_ty())
                                .to_string(),
                            "fixtures.build.Base"
                        );
                        assert_eq!(dispatch.owner_fqn(), "fixtures.build.Base");
                        assert_eq!(dispatch.member_name(), "ping");
                        assert!(!dispatch.receiver_abi().is_elided());
                    }
                    other => panic!(
                        "non-boundary virtual call 应发布 receiver-dispatch carrier，而不是 {other:?}"
                    ),
                }
            },
        );
    }

    #[test]
    fn refactor_llvm_dynamic_invoke_query_rejects_missing_published_contract() {
        with_fixture_query_result(
            "effect_refactor_dynamic_invoke_candidate_set_emit_llvm.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let helper = program
                    .callable("fixtures.build.helper")
                    .expect("fixtures.build.helper callable 应存在");
                let bogus_site = crate::mir::SiteId::from_raw(999);
                let rewritten_boundary_map = LateLoweredBoundaryMap::new(
                    helper
                        .boundary_map()
                        .entries()
                        .iter()
                        .map(|boundary| {
                            let source = match boundary.source() {
                                LateLoweredBoundarySource::Site {
                                    kind: BoundarySiteKind::Call,
                                    ..
                                } => LateLoweredBoundarySource::Site {
                                    site_id: bogus_site,
                                    kind: BoundarySiteKind::Call,
                                },
                                other => other,
                            };
                            let lowered = boundary
                                .lowering()
                                .cloned()
                                .expect("candidate-set helper 的 boundary 应带 lowering");
                            LateLoweredBoundary::new(
                                boundary.boundary_id(),
                                source,
                                boundary.owner_state(),
                                boundary.resume_state(),
                            )
                            .with_lowering(lowered)
                        })
                        .collect(),
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == helper.step_schema() {
                            clone_callable_with_boundary_map(
                                candidate,
                                rewritten_boundary_map.clone(),
                            )
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 dynamic-invoke contract 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("canonical MIR call metadata"),
                    "错误消息应指出缺失的是 call-site authoritative metadata: {message}"
                );
                assert!(
                    message.contains("dynamic-invoke contract"),
                    "错误消息应指出缺失的是 dynamic-invoke contract: {message}"
                );
                assert!(
                    message.contains("fixtures.build.helper") && message.contains("999"),
                    "错误消息应指出缺失 contract 所属的 callable 和 site id: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_call_boundary_continuation_composition() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("refactor ABI materialization 应成功");
                let main = inputs
                    .abi_visibility_program
                    .callable("main")
                    .expect("main callable 应存在");
                let composition = main
                    .boundary_map()
                    .entries()
                    .iter()
                    .find_map(|boundary| {
                        let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering()
                        else {
                            return None;
                        };
                        lowering.continuation_compositions().first()
                    })
                    .expect("main 的 fetch call boundary 应发布 composition contract");
                let continuation_layout = query
                    .continuation_layout(main.continuation_object())
                    .expect("main continuation object layout 应存在");
                assert!(continuation_layout.fields().iter().any(|field| {
                    field.kind() == RefactorContinuationFieldKind::ComposedCalleeContinuation
                }));
                let callee_surface = query
                    .surface_resume_layout(composition.callee_continuation_schema())
                    .expect("callee continuation surface resume ABI 应发布");
                assert_eq!(
                    callee_surface.return_step_schema(),
                    composition.input_step_schema()
                );
                assert_eq!(
                    callee_surface.resume_tuple_ty(),
                    composition.callee_continuation_contract().resume_tuple_ty()
                );
            },
        );

        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let main = program.callable("main").expect("main callable 应存在");
                let boundary_map = LateLoweredBoundaryMap::new(
                    main.boundary_map()
                        .entries()
                        .iter()
                        .map(|boundary| {
                            let lowering = match boundary
                                .lowering()
                                .cloned()
                                .expect("main boundary 应带 lowering")
                            {
                                LateLoweredBoundaryLowering::Call(lowering) => {
                                    LateLoweredBoundaryLowering::Call(
                                        LateLoweredCallBoundaryLowering::new(
                                            lowering.facts().clone(),
                                            lowering.result_local(),
                                            lowering.operand_contract().clone(),
                                            lowering.dispatch().clone(),
                                            Vec::new(),
                                            lowering.consumed_runtime_error_case().cloned(),
                                        ),
                                    )
                                }
                                other => other,
                            };
                            LateLoweredBoundary::new(
                                boundary.boundary_id(),
                                boundary.source(),
                                boundary.owner_state(),
                                boundary.resume_state(),
                            )
                            .with_lowering(lowering)
                        })
                        .collect(),
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == main.step_schema() {
                            clone_callable_with_boundary_map(candidate, boundary_map.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 continuation composition 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("continuation composition"),
                    "错误消息应指出缺失 call-boundary continuation composition: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_dynamic_entry_publication_declares_closure_vtable_and_itable_targets() {
        with_inputs_query_result_and_codegen(
            build_fixture_inputs("effect_refactor_dynamic_entry_publication_emit_llvm.scoop"),
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, codegen, result, module| {
                let query = result.expect("refactor ABI materialization 应成功");
                let make_closure_callable = inputs
                    .abi_visibility_program
                    .callable("fixtures.build.makeClosure")
                    .expect("makeClosure callable 应存在");
                let base_ping_callable = inputs
                    .abi_visibility_program
                    .callable("fixtures.build.Base.ping")
                    .expect("Base.ping callable 应存在");
                let derived_ping_callable = inputs
                    .abi_visibility_program
                    .callable("fixtures.build.Derived.ping")
                    .expect("Derived.ping callable 应存在");

                let make_closure = query
                    .callable_carrier_target_layout(
                        RefactorCallableCarrierKind::ClosureObject,
                        "fixtures.build.makeClosure",
                    )
                    .expect("makeClosure closure carrier target 应存在");
                let base_vtable = query
                    .callable_carrier_target_layout(
                        RefactorCallableCarrierKind::ClassVtable,
                        "fixtures.build.Base.ping",
                    )
                    .expect("Base.ping vtable carrier target 应存在");
                let base_itable = query
                    .callable_carrier_target_layout(
                        RefactorCallableCarrierKind::InterfaceItable,
                        "fixtures.build.Base.ping",
                    )
                    .expect("Base.ping itable carrier target 应存在");
                let derived_vtable = query
                    .callable_carrier_target_layout(
                        RefactorCallableCarrierKind::ClassVtable,
                        "fixtures.build.Derived.ping",
                    )
                    .expect("Derived.ping vtable carrier target 应存在");
                let derived_itable = query
                    .callable_carrier_target_layout(
                        RefactorCallableCarrierKind::InterfaceItable,
                        "fixtures.build.Derived.ping",
                    )
                    .expect("Derived.ping itable carrier target 应存在");

                assert_eq!(
                    make_closure.body_version_key(),
                    make_closure_callable.body_version_key()
                );
                assert_eq!(
                    base_vtable.body_version_key(),
                    base_ping_callable.body_version_key()
                );
                assert_eq!(
                    base_itable.body_version_key(),
                    base_ping_callable.body_version_key()
                );
                assert_eq!(
                    derived_vtable.body_version_key(),
                    derived_ping_callable.body_version_key()
                );
                assert_eq!(
                    derived_itable.body_version_key(),
                    derived_ping_callable.body_version_key()
                );

                let _ = codegen
                    .get_or_create_class_vtable_global(dummy_span(), "fixtures.build.Base")
                    .expect("Base vtable 应可物化");
                let _ = codegen
                    .get_or_create_class_vtable_global(dummy_span(), "fixtures.build.Derived")
                    .expect("Derived vtable 应可物化");
                let _ = codegen
                    .get_or_create_class_itable_global(dummy_span(), "fixtures.build.Base")
                    .expect("Base itable 应可物化");
                let _ = codegen
                    .get_or_create_class_itable_global(dummy_span(), "fixtures.build.Derived")
                    .expect("Derived itable 应可物化");

                assert!(module.get_function(make_closure.symbol_name()).is_some());
                assert!(module.get_function(base_vtable.symbol_name()).is_some());
                assert!(module.get_function(base_itable.symbol_name()).is_some());
                assert!(module.get_function(derived_vtable.symbol_name()).is_some());
                assert!(module.get_function(derived_itable.symbol_name()).is_some());
            },
        );
    }

    #[test]
    fn refactor_llvm_callable_carrier_version_selection_rejects_ambiguous_root_targets() {
        with_fixture_query_result(
            "effect_refactor_dynamic_entry_publication_emit_llvm.scoop",
            |inputs| {
                duplicate_no_outward_callable_version(
                    &inputs.abi_visibility_program,
                    "fixtures.build.makeClosure",
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!(
                        "缺少 callable version selector 的 duplicate carrier target 必须 fail fast"
                    ),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("closure callable object"),
                    "错误消息应指出歧义 carrier kind: {message}"
                );
                assert!(
                    message.contains("fixtures.build.makeClosure"),
                    "错误消息应指出歧义 callable: {message}"
                );
                assert!(
                    message.contains("多个 published callable version")
                        || message.contains("多个 published callable version"),
                    "错误消息应指出存在多个 callable version: {message}"
                );
                assert!(
                    message.contains("authoritative version selector"),
                    "错误消息应指出缺少 authoritative selector: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_dynamic_entry_publication_rejects_missing_dispatch_callable_shell() {
        with_inputs_query_result_and_codegen(
            build_fixture_inputs("effect_refactor_dynamic_entry_publication_emit_llvm.scoop"),
            |inputs| inputs.abi_visibility_program.clone(),
            |_inputs, codegen, result, _module| {
                let _ = result.expect("ABI materialization 应成功");
                let dummy_fn = codegen.module.add_function(
                    "__scoop_refactor_missing_carrier_target_dummy",
                    codegen.context.void_type().fn_type(&[], false),
                    None,
                );
                let err = match codegen.callable_carrier_target_fn_ptr(
                    RefactorCallableCarrierKind::ClassVtable,
                    "fixtures.build.Missing.ping",
                    dummy_fn.as_global_value().as_pointer_value(),
                ) {
                    Ok(_) => panic!("缺失 dispatch callable shell 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("fixtures.build.Missing.ping"),
                    "错误消息应指出缺失 shell 的 target callable: {message}"
                );
                assert!(
                    message.contains("published target entry")
                        || message.contains("class vtable slot"),
                    "错误消息应指出问题出在 carrier target 发布: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_local_runtime_error_contract_resolves_pure_call_boundary_targets() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, module| {
                let query =
                    result.expect("pure caller local runtime-error contract 应可发布到 ABI query");
                let main = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("main")
                    .expect("main callable 应存在");
                let mut checked = 0usize;

                for boundary in main.boundary_map().entries() {
                    let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering()
                    else {
                        continue;
                    };
                    let Some(contract) = lowering.consumed_runtime_error_case() else {
                        continue;
                    };
                    let site_id = boundary_site_id(boundary);
                    let published = query
                        .call_local_runtime_error_contract(main.step_schema(), site_id, contract)
                        .expect("call boundary 应可回查 published local runtime-error contract");

                    assert_eq!(published.owner_step_schema(), main.step_schema());
                    assert_eq!(published.site_id(), site_id);
                    assert_eq!(published.input_case_tag(), contract.input_case_tag());
                    assert_eq!(published.payload_tuple_ty(), contract.payload_tuple_ty());
                    assert_eq!(
                        published.terminal_action().lowered_action(),
                        contract.terminal_action()
                    );
                    assert_eq!(published.target_state(), contract.target_state());
                    assert!(
                        !published.payload_abi().is_elided(),
                        "RuntimeError payload 不应被零载荷退化"
                    );
                    let runtime_entry = published.terminal_action().runtime_entry();
                    assert_eq!(
                        runtime_entry.kind(),
                        LateLoweredPublishedRuntimeEntry::RuntimeErrorFatal
                    );
                    assert_eq!(runtime_entry.symbol_name(), "scoop_runtime_error_fatal");
                    assert_eq!(runtime_entry.param_count(), 1);
                    assert!(
                        module.get_function(runtime_entry.symbol_name()).is_some(),
                        "published runtime fatal entry 应声明到 LLVM module 中"
                    );
                    checked += 1;
                }

                assert_eq!(
                    checked, 2,
                    "fixture 应包含两个 pure caller call boundary contract"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_local_runtime_error_contract_rejects_missing_target_state() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let main = program.callable("main").expect("main callable 应存在");
                let boundary_map = LateLoweredBoundaryMap::new(
                    main.boundary_map()
                        .entries()
                        .iter()
                        .map(|boundary| {
                            let lowering = match boundary
                                .lowering()
                                .cloned()
                                .expect("main boundary 应带 lowering")
                            {
                                LateLoweredBoundaryLowering::Call(lowering) => {
                                    let consumed_runtime_error_case = lowering
                                        .consumed_runtime_error_case()
                                        .cloned()
                                        .map(|contract| {
                                            LateLoweredConsumedRuntimeErrorCase::new(
                                                contract.input_case_tag(),
                                                contract.input_concrete_op_key().clone(),
                                                contract.payload_tuple_ty(),
                                                contract.terminal_action(),
                                                StateId::new(999),
                                            )
                                        });
                                    LateLoweredBoundaryLowering::Call(
                                        LateLoweredCallBoundaryLowering::new(
                                            lowering.facts().clone(),
                                            lowering.result_local(),
                                            lowering.operand_contract().clone(),
                                            lowering.dispatch().clone(),
                                            lowering.continuation_compositions().to_vec(),
                                            consumed_runtime_error_case,
                                        ),
                                    )
                                }
                                other => other,
                            };
                            LateLoweredBoundary::new(
                                boundary.boundary_id(),
                                boundary.source(),
                                boundary.owner_state(),
                                boundary.resume_state(),
                            )
                            .with_lowering(lowering)
                        })
                        .collect(),
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == main.step_schema() {
                            clone_callable_with_boundary_map(candidate, boundary_map.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 local runtime-error target state 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("local runtime-error target state"),
                    "错误消息应指出缺失的是 local runtime-error target state: {message}"
                );
                assert!(
                    message.contains("main") && message.contains("call site 1"),
                    "错误消息应指出缺失 contract 所属的 callable 和 site id: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_local_runtime_error_contract_rejects_non_local_runtime_error_terminator() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let main = program.callable("main").expect("main callable 应存在");
                let local_runtime_error_states = main
                    .boundary_map()
                    .entries()
                    .iter()
                    .filter_map(|boundary| {
                        let Some(LateLoweredBoundaryLowering::Call(lowering)) = boundary.lowering()
                        else {
                            return None;
                        };
                        lowering
                            .consumed_runtime_error_case()
                            .map(|contract| contract.target_state())
                    })
                    .collect::<BTreeSet<_>>();
                let rewritten_states = main
                    .state_graph()
                    .states()
                    .iter()
                    .map(|state| {
                        if !local_runtime_error_states.contains(&state.state_id()) {
                            return state.clone();
                        }
                        crate::effect_lowered::ir::LateLoweredState::new(
                            state.state_id(),
                            state.role(),
                            state.source_slices().to_vec(),
                            crate::effect_lowered::ir::LateLoweredStateTerminator::Unreachable,
                        )
                    })
                    .collect::<Vec<_>>();
                let state_graph = crate::effect_lowered::ir::LateLoweredStateGraph::new(
                    main.state_graph().entry_state(),
                    main.state_graph().complete_state(),
                    main.state_graph().cleanup_state(),
                    main.state_graph().drop_state(),
                    rewritten_states,
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == main.step_schema() {
                            clone_callable_with_state_graph(candidate, state_graph.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 LocalRuntimeError terminal contract 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("不是 LocalRuntimeError terminator"),
                    "错误消息应指出 local runtime-error target state 缺少终止 contract: {message}"
                );
                assert!(
                    message.contains("main") && message.contains("call site 1"),
                    "错误消息应指出缺失 contract 所属的 callable 和 site id: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_handle_dispatch_contract_publishes_llvm_query_layout() {
        with_phase_fixture_query_result(
            "effect_facts",
            "nested_handle_self_contained_vs_outward.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("HandleDispatch contract 应可发布到 LLVM ABI query");
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("sample.nested_may_suspend_outward")
                    .expect("callable 应存在");
                let site_id = SiteId::from_raw(1);
                let contract = handle_dispatch_contract(callable, site_id);
                let published = query
                    .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                    .expect("query 应能稳定回查 HandleDispatch contract");
                let frame_layout = query
                    .frame_layout(callable.step_schema())
                    .expect("frame layout 应可查询");

                assert_eq!(published.owner_step_schema(), callable.step_schema());
                assert_eq!(published.site_id(), site_id);
                assert_eq!(published.lowered_contract(), contract);
                assert_eq!(
                    published.state_tag_field_index(),
                    frame_layout
                        .field_index_for_system(SystemSlotKind::StateTag)
                        .expect("frame 应保留 StateTag")
                );
                assert_eq!(
                    published.completion_tag_field_index(),
                    frame_layout
                        .field_index_for_system(SystemSlotKind::CompletionTag)
                        .expect("frame 应保留 CompletionTag")
                );
                assert_eq!(
                    published.payload_carrier_field_index(),
                    frame_layout
                        .field_index_for_system(SystemSlotKind::ResumePayloadCarrier)
                        .expect("frame 应保留 ResumePayloadCarrier")
                );
                assert!(
                    published
                        .completion_tag_value(LateLoweredHandlePendingCompletion::ContinueToExit)
                        .is_some()
                );
                assert!(
                    published
                        .completion_tag_value(
                            LateLoweredHandlePendingCompletion::ReturnFromFunction
                        )
                        .is_some()
                );
                assert!(
                    published
                        .completion_tag_value(LateLoweredHandlePendingCompletion::PropagateOutward(
                            crate::effect_facts::CaseTag::new(1),
                        ))
                        .is_none()
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_handle_dispatch_publishes_pending_payload_transport_layout() {
        with_inputs_query_result(
            build_fixture_inputs_from_source(SourceFile::new_virtual(
                "<mem>/llvm_handle_pending_payload_transport.scoop",
                r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
            )),
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query =
                    result.expect("pending payload transport 应可发布到 HandleDispatch LLVM query");
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("sample.propagate_before_finally")
                    .expect("sample.propagate_before_finally callable 应存在");
                let (site_id, contract) = handle_dispatch_with_pending_outward(callable);
                let published = query
                    .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                    .expect("query 应能稳定回查 pending payload transport contract");
                let pending_case = *contract
                    .body_outward_cases()
                    .first()
                    .expect("fixture 应发布 body outward case");
                let transport = published
                    .pending_payload_transport_layout(
                        LateLoweredHandlePendingCompletion::PropagateOutward(pending_case),
                    )
                    .expect("pending outward case 应发布 typed payload transport layout");
                let frame_layout = query
                    .frame_layout(callable.step_schema())
                    .expect("frame layout 应可查询");
                let slot = callable
                    .frame_schema()
                    .slot_for_kind(LateLoweredFrameSlotKind::HandlePendingPayload {
                        site_id,
                        case_tag: pending_case,
                    })
                    .expect("frame schema 应保留 HandlePendingPayload slot");

                assert_eq!(
                    transport.completion(),
                    LateLoweredHandlePendingCompletion::PropagateOutward(pending_case)
                );
                assert_eq!(transport.frame_slot(), slot.slot_id());
                assert_eq!(
                    transport.frame_field_index(),
                    frame_layout
                        .field_index_for_slot(slot.slot_id())
                        .expect("frame layout 应可回查 pending payload field")
                );
                assert_eq!(
                    transport.payload_tuple_ty(),
                    contract
                        .outward_emission(pending_case)
                        .expect("pending outward case 应保留 outward emission")
                        .payload_tuple_ty()
                );
                assert!(
                    published
                        .pending_payload_transport_layout(
                            LateLoweredHandlePendingCompletion::ContinueToExit,
                        )
                        .is_none()
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_handle_dispatch_rejects_missing_pending_payload_transport() {
        with_inputs_query_result(
            build_fixture_inputs_from_source(SourceFile::new_virtual(
                "<mem>/llvm_handle_pending_payload_transport_missing.scoop",
                r#"
package sample

effect Inner {
    fun go(): Int
}

effect Outer {
    fun again(): Unit
}

fun cleanup() {}

fun propagate_before_finally(): Int {
    return handle {
        val nested: Int = handle {
            Outer.again()
            0
        } with {
            Inner.go() -> 1
        } finally {
            cleanup()
        }
        nested + 10
    } with {
        Outer.again() -> 99
    }
}
"#,
            )),
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let callable = program
                    .callable("sample.propagate_before_finally")
                    .expect("callable 应存在");
                let (site_id, contract) = handle_dispatch_with_pending_outward(callable);
                let broken_contract = LateLoweredHandleDispatchContract::new(
                    contract.carrier(),
                    contract.body_complete_target(),
                    contract.arm_complete_target(),
                    contract.finally_complete_target(),
                    contract.body_completion_payload_source().cloned(),
                    contract.handled_arms().to_vec(),
                    contract.body_outward_cases().to_vec(),
                    contract.finally_outward_cases().to_vec(),
                    contract.outward_emissions().to_vec(),
                    contract.pending_completions().to_vec(),
                    Vec::new(),
                    contract.state_regions().to_vec(),
                    contract.boundary_routings().to_vec(),
                    contract.abandon_target(),
                );
                let state_graph = clone_state_graph_with_handle_contract(
                    callable.state_graph(),
                    site_id,
                    broken_contract,
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == callable.step_schema() {
                            clone_callable_with_state_graph(candidate, state_graph.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 pending payload transport 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("pending payload transport"),
                    "错误消息应指出缺失的是 pending payload transport contract: {message}"
                );
                assert!(
                    message.contains("sample.propagate_before_finally")
                        && message.contains("handle site"),
                    "错误消息应指出出错 callable 和 site: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_handle_dispatch_region_routing_publishes_query_lookup() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query =
                    result.expect("handle region routing contract 应可发布到 LLVM ABI query");
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("run")
                    .expect("run callable 应存在");
                let (site_id, contract) = first_handle_dispatch(callable);
                let published = query
                    .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                    .expect("query 应能稳定回查 handle region routing contract");
                let perform_boundary = callable
                    .boundary_map()
                    .entries()
                    .iter()
                    .find(|boundary| {
                        matches!(
                            boundary.source(),
                            LateLoweredBoundarySource::Site {
                                kind: BoundarySiteKind::Perform,
                                ..
                            }
                        )
                    })
                    .expect("fixture 应发布 body perform boundary");
                let routing = published
                    .boundary_routing(perform_boundary.boundary_id())
                    .expect("perform boundary 应可通过 query 回查 routing contract");
                let handled_arm = contract
                    .handled_arms()
                    .first()
                    .expect("fixture 应发布唯一 handled arm");
                let handled_route = routing
                    .case_routing(handled_arm.handled_case())
                    .expect("handled perform case 应发布 consume-to-arm routing");

                assert_eq!(
                    routing.owner_region(),
                    crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body
                );
                assert_eq!(
                    published.state_region(routing.owner_state()),
                    crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body
                );
                assert_eq!(
                    published.state_region(routing.resume_state()),
                    crate::effect_lowered::ir::LateLoweredHandleStateRegion::Body
                );
                assert!(matches!(
                    handled_route.action(),
                    crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                        arm_state,
                        arm_ordinal,
                        continuation_resume_state,
                    } if arm_state == handled_arm.arm_state()
                        && arm_ordinal == handled_arm.arm_ordinal()
                        && continuation_resume_state == routing.resume_state()
                ));
            },
        );
    }

    #[test]
    fn refactor_handle_dispatch_region_routing_rejects_resume_state_drift() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let callable = program.callable("run").expect("run callable 应存在");
                let (site_id, contract) = first_handle_dispatch(callable);
                let handled_case = contract
                    .handled_arms()
                    .first()
                    .expect("fixture 应发布唯一 handled arm")
                    .handled_case();
                let broken_routings = contract
                    .boundary_routings()
                    .iter()
                    .map(|routing| {
                        let broken_case_routings = routing
                            .case_routings()
                            .iter()
                            .map(|route| {
                                if route.case_tag() != handled_case {
                                    return *route;
                                }
                                let broken_action = match route.action() {
                                    crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                                        arm_state,
                                        arm_ordinal,
                                        ..
                                    } => crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                                        arm_state,
                                        arm_ordinal,
                                        continuation_resume_state: contract.body_complete_target(),
                                    },
                                    other => other,
                                };
                                crate::effect_lowered::ir::LateLoweredHandleBoundaryCaseRouting::new(
                                    route.case_tag(),
                                    broken_action,
                                )
                            })
                            .collect::<Vec<_>>();
                        crate::effect_lowered::ir::LateLoweredHandleBoundaryRouting::new(
                            routing.boundary_id(),
                            routing.owner_state(),
                            routing.owner_region(),
                            routing.resume_state(),
                            broken_case_routings,
                        )
                    })
                    .collect::<Vec<_>>();
                let broken_contract = clone_handle_dispatch_contract_with_regions_and_routes(
                    contract,
                    contract.state_regions().to_vec(),
                    broken_routings,
                );
                let state_graph = clone_state_graph_with_handle_contract(
                    callable.state_graph(),
                    site_id,
                    broken_contract,
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == callable.step_schema() {
                            clone_callable_with_state_graph(candidate, state_graph.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("handle boundary routing resume_state 漂移时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("boundary-routing contract 漂移")
                        || message.contains("consume_to_arm")
                        || message.contains("resume=st"),
                    "错误消息应指出 published routing 与 state graph/boundary map 不一致: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_handle_arm_binding_contract_publishes_llvm_query_layout() {
        with_inputs_query_result(
            build_fixture_inputs_from_source(SourceFile::new_virtual(
                "<mem>/llvm_handle_arm_binding_single.scoop",
                r#"
package sample

import scoop.core.*

effect Edge {
    fun visit(from: String, to: Int): Int
}

fun run(): Int {
    return handle {
        Edge.visit("alpha", 1)
    } with {
        Edge.visit(from, to), k -> {
            k.resume(to + 1)
        }
    }
}

fun main(): Int {
    return 0
}
"#,
            )),
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("handle arm binder contract 应可发布到 LLVM ABI query");
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("sample.run")
                    .expect("sample.run callable 应存在");
                let (site_id, contract) = first_handle_dispatch(callable);
                let published = query
                    .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                    .expect("query 应能稳定回查 HandleDispatch arm binder contract");
                let arm = published
                    .handled_arms()
                    .first()
                    .expect("单 arm fixture 应发布唯一 handled arm layout");

                assert_eq!(arm.arm_ordinal(), 0);
                assert_eq!(arm.payload_binders().len(), 2);
                assert_eq!(arm.payload_binders()[0].ordinal(), 0);
                assert_eq!(arm.payload_binders()[1].ordinal(), 1);
                let continuation_binder = arm
                    .continuation_binder()
                    .expect("escape continuation arm 应发布 continuation binder layout");
                assert_eq!(
                    continuation_binder.continuation_object(),
                    callable.continuation_object()
                );
                assert_eq!(
                    continuation_binder.surface_resume_source_kind(),
                    crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
                );
                assert_eq!(
                    continuation_binder.surface_resume_return_step_schema(),
                    callable.step_schema()
                );
            },
        );
    }

    #[test]
    fn refactor_handle_arm_continuation_binding_publishes_mixed_multi_arm_query_layout() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("mixed multi-arm handle 应可发布 arm binder query");
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("main")
                    .expect("main callable 应存在");
                let (site_id, contract) = first_handle_dispatch(callable);
                let published = query
                    .handle_dispatch_layout(callable.step_schema(), site_id, contract)
                    .expect("query 应能稳定回查 mixed handle arm binder contract");

                assert_eq!(published.handled_arms().len(), 2);
                let escape_arm = published
                    .handled_arms()
                    .iter()
                    .find(|arm| arm.continuation_binder().is_some())
                    .expect("mixed fixture 应发布带 continuation binder 的 arm layout");
                let payload_only_arm = published
                    .handled_arms()
                    .iter()
                    .find(|arm| arm.continuation_binder().is_none())
                    .expect("mixed fixture 应发布纯 payload arm layout");

                assert_eq!(escape_arm.payload_binders().len(), 1);
                assert_eq!(payload_only_arm.payload_binders().len(), 1);
                let continuation_binder = escape_arm
                    .continuation_binder()
                    .expect("escape arm 应带 continuation binder layout");
                assert_eq!(
                    continuation_binder.surface_resume_source_kind(),
                    crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
                );
            },
        );
    }

    #[test]
    fn refactor_completion_state_contract_rejects_missing_completion_tag_slot() {
        with_phase_fixture_query_result(
            "effect_facts",
            "nested_handle_self_contained_vs_outward.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let callable = program
                    .callable("sample.nested_may_suspend_outward")
                    .expect("callable 应存在");
                let frame_schema = LateLoweredFrameSchema::new(
                    callable
                        .frame_schema()
                        .slots()
                        .iter()
                        .filter(|slot| {
                            slot.kind()
                                != LateLoweredFrameSlotKind::System(SystemSlotKind::CompletionTag)
                        })
                        .cloned()
                        .collect(),
                )
                .with_resume_payload_bindings(
                    callable.frame_schema().resume_payload_bindings().to_vec(),
                )
                .with_completion_payload_bindings(
                    callable
                        .frame_schema()
                        .completion_payload_bindings()
                        .to_vec(),
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == callable.step_schema() {
                            clone_callable_with_frame_schema(candidate, frame_schema.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 CompletionTag system field 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("缺少 CompletionTag system field"),
                    "错误消息应指出缺失的是 CompletionTag 槽位: {message}"
                );
                assert!(
                    message.contains("sample.nested_may_suspend_outward"),
                    "错误消息应指出出错 callable: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_handle_arm_binding_contract_rejects_payload_binder_order_drift() {
        with_inputs_query_result(
            build_fixture_inputs_from_source(SourceFile::new_virtual(
                "<mem>/llvm_handle_arm_binding_order_drift.scoop",
                r#"
package sample

import scoop.core.*

effect Edge {
    fun visit(from: String, to: Int): Int
}

fun run(): Int {
    return handle {
        Edge.visit("alpha", 1)
    } with {
        Edge.visit(from, to), k -> {
            k.resume(to + 1)
        }
    }
}

fun main(): Int {
    return 0
}
"#,
            )),
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let callable = program
                    .callable("sample.run")
                    .expect("sample.run callable 应存在");
                let (site_id, contract) = first_handle_dispatch(callable);
                let original_arm = contract
                    .handled_arms()
                    .first()
                    .expect("fixture 应发布唯一 handled arm");
                let mut swapped_binders = original_arm.payload_binders().to_vec();
                swapped_binders.swap(0, 1);
                let broken_arm = crate::effect_lowered::ir::LateLoweredHandleArmDispatch::new(
                    original_arm.handled_case(),
                    original_arm.arm_state(),
                    original_arm.arm_ordinal(),
                    original_arm.payload_tuple_ty(),
                    original_arm.completion_payload_source().clone(),
                    swapped_binders,
                    original_arm.continuation_binder(),
                    original_arm.arm_outward_cases().to_vec(),
                );
                let broken_contract =
                    clone_handle_dispatch_contract_with_handled_arms(contract, vec![broken_arm]);
                let state_graph = clone_state_graph_with_handle_contract(
                    callable.state_graph(),
                    site_id,
                    broken_contract,
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == callable.step_schema() {
                            clone_callable_with_state_graph(candidate, state_graph.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("payload binder 次序漂移时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("payload binder ordinal 漂移")
                        || message.contains("payload binder #0 local 漂移"),
                    "错误消息应指出 payload binder 顺序漂移: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_handle_dispatch_contract_rejects_missing_handled_arm_mapping() {
        with_phase_fixture_query_result(
            "effect_facts",
            "nested_handle_self_contained_vs_outward.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let callable = program
                    .callable("sample.nested_may_suspend_outward")
                    .expect("callable 应存在");
                let site_id = SiteId::from_raw(1);
                let contract = handle_dispatch_contract(callable, site_id);
                let broken_contract = LateLoweredHandleDispatchContract::new(
                    contract.carrier(),
                    contract.body_complete_target(),
                    contract.arm_complete_target(),
                    contract.finally_complete_target(),
                    contract.body_completion_payload_source().cloned(),
                    Vec::new(),
                    contract.body_outward_cases().to_vec(),
                    contract.finally_outward_cases().to_vec(),
                    contract.outward_emissions().to_vec(),
                    contract.pending_completions().to_vec(),
                    contract.pending_payload_transports().to_vec(),
                    contract.state_regions().to_vec(),
                    contract.boundary_routings().to_vec(),
                    contract.abandon_target(),
                );
                let state_graph = clone_state_graph_with_handle_contract(
                    callable.state_graph(),
                    site_id,
                    broken_contract,
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == callable.step_schema() {
                            clone_callable_with_state_graph(candidate, state_graph.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 handled-arm 映射时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("handled-arm 数量"),
                    "错误消息应指出缺失的是 handled-arm mapping: {message}"
                );
                assert!(
                    message.contains("handle site 1") || message.contains("site 1"),
                    "错误消息应指出出错 site: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_handle_arm_continuation_binding_rejects_missing_published_continuation_binder() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let callable = program.callable("main").expect("main callable 应存在");
                let (site_id, contract) = first_handle_dispatch(callable);
                let broken_arms = contract
                    .handled_arms()
                    .iter()
                    .map(|arm| {
                        crate::effect_lowered::ir::LateLoweredHandleArmDispatch::new(
                            arm.handled_case(),
                            arm.arm_state(),
                            arm.arm_ordinal(),
                            arm.payload_tuple_ty(),
                            arm.completion_payload_source().clone(),
                            arm.payload_binders().to_vec(),
                            None,
                            arm.arm_outward_cases().to_vec(),
                        )
                    })
                    .collect::<Vec<_>>();
                let broken_contract =
                    clone_handle_dispatch_contract_with_handled_arms(contract, broken_arms);
                let state_graph = clone_state_graph_with_handle_contract(
                    callable.state_graph(),
                    site_id,
                    broken_contract,
                );
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.step_schema() == callable.step_schema() {
                            clone_callable_with_state_graph(candidate, state_graph.clone())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 published continuation binder 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("缺少 published continuation binder contract"),
                    "错误消息应指出缺失的是 continuation binder contract: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_layout_keeps_shared_schema_multi_case_object_publications() {
        with_fixture_query_result(
            "effect_refactor_step_enum_single_case.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("shared-schema fixture 应可物化 surface-resume ABI");
                let callable = inputs
                    .abi_visibility_program
                    .callable("fixtures.build.singleCaseWorker")
                    .expect("singleCaseWorker callable 应存在");
                let step = inputs
                    .abi_visibility_program
                    .step_type(callable.step_schema())
                    .expect("worker step shell 应存在");
                let shared_schema = step
                    .case(CaseTag::new(0))
                    .expect("worker c0 应存在")
                    .continuation_schema();
                let continuation_layout = query
                    .continuation_layout(callable.continuation_object())
                    .expect("continuation layout 应可查询");
                let surface_layout = query
                    .surface_resume_layout(shared_schema)
                    .expect("shared schema surface-resume layout 应可查询");
                let bindings = continuation_layout
                    .surface_resume_bindings(shared_schema)
                    .expect("object-side shared schema surface publication 应可查询");

                assert_eq!(
                    surface_layout.dispatch_source_kind(),
                    crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
                );
                assert_eq!(bindings.len(), 2);
                assert!(bindings.iter().any(|binding| {
                    binding.case_tag() == CaseTag::new(0)
                        && binding.reachability()
                            == crate::effect_lowered::ir::LateLoweredContinuationMethodReachability::Reachable
                }));
                assert!(bindings.iter().any(|binding| {
                    binding.case_tag() == CaseTag::new(1)
                        && binding.reachability()
                            == crate::effect_lowered::ir::LateLoweredContinuationMethodReachability::Unreachable
                }));
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_layout_resolves_resume_site_contracts() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, module| {
                let query = result.expect("resume fixture 应可物化 surface-resume ABI");
                let mut checked_resume_site = false;
                for callable in inputs.effect_lowered_stage_output.program().callables() {
                    for boundary in callable.boundary_map().entries() {
                        let Some(LateLoweredBoundaryLowering::Resume(lowering)) =
                            boundary.lowering()
                        else {
                            continue;
                        };
                        let facts = lowering.facts();
                        let surface_layout = query
                            .surface_resume_layout(facts.continuation_schema())
                            .expect("ResumeSiteEffectFacts 所需的 surface-resume layout 应已发布");

                        assert_eq!(
                            surface_layout.continuation_schema(),
                            facts.continuation_schema()
                        );
                        assert_eq!(surface_layout.resume_tuple_ty(), facts.resume_tuple_ty());
                        assert_eq!(surface_layout.answer_ty(), facts.answer_ty());
                        assert_eq!(surface_layout.return_step_schema(), facts.out_step_schema());
                        assert_eq!(surface_layout.param_count(), 2);
                        assert!(
                            !surface_layout.resume_payload_abi().is_elided(),
                            "Int resume payload 不应被零载荷退化"
                        );
                        assert!(
                            module.get_function(surface_layout.symbol_name()).is_some(),
                            "surface-resume symbol 应被声明到 module 中"
                        );
                        assert_eq!(
                            surface_layout.dispatch_source_kind(),
                            crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ResumeBoundaryOnly
                        );
                        checked_resume_site = true;
                    }
                }
                assert!(
                    checked_resume_site,
                    "fixture 应至少包含一个 resume boundary"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_layout_rejects_missing_published_contract() {
        with_fixture_query_result(
            "effect_refactor_dynamic_invoke_unit_payload.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let callable = program
                    .callable("fixtures.build.unitWorker")
                    .expect("callable 应存在");
                let continuation_objects = program
                    .continuation_objects()
                    .iter()
                    .map(|candidate| {
                        if candidate.object_id() == callable.continuation_object() {
                            clone_continuation_object_with_surface_resumes(candidate, Vec::new())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect::<Vec<_>>();

                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    continuation_objects,
                    program.callables().to_vec(),
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 published surface-resume contract 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("surface-resume 发布"),
                    "错误消息应指出缺失的是 surface-resume contract: {message}"
                );
                assert!(
                    message.contains("owner step schema"),
                    "错误消息应指出缺失 contract 所属的 owner step schema: {message}"
                );
                assert!(
                    message.contains("continuation schema k"),
                    "错误消息应指出缺失的 continuation schema: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_dispatch_layout_resolves_object_method_target() {
        with_fixture_query_result(
            "effect_refactor_step_enum_single_case.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("shared schema 应可发布 owner dispatch query");
                let callable = inputs
                    .abi_visibility_program
                    .callable("fixtures.build.singleCaseWorker")
                    .expect("singleCaseWorker callable 应存在");
                let step = inputs
                    .abi_visibility_program
                    .step_type(callable.step_schema())
                    .expect("worker step shell 应存在");
                let shared_schema = step
                    .case(CaseTag::new(0))
                    .expect("worker c0 应存在")
                    .continuation_schema();
                let surface_layout = query
                    .surface_resume_layout(shared_schema)
                    .expect("surface-resume layout 应可查询");
                let dispatch = query
                    .surface_resume_dispatch_layout(shared_schema)
                    .expect("owner dispatch contract 应可查询");

                assert_eq!(dispatch.continuation_schema(), shared_schema);
                assert_eq!(
                    dispatch.source_kind(),
                    crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
                );
                assert_eq!(dispatch.method_targets().len(), 1);

                let lookup = dispatch.method_targets()[0];
                assert_eq!(lookup.continuation_object(), callable.continuation_object());
                let continuation_layout = query
                    .continuation_layout(lookup.continuation_object())
                    .expect("continuation layout 应可查询");
                assert_eq!(
                    continuation_layout.field_index_for_packing(lookup.packing_interface_id()),
                    Some(lookup.packing_field_index())
                );
                let method_layout = query
                    .surface_resume_method_layout(lookup)
                    .expect("surface-resume packing method layout 应可直接查询");
                assert_eq!(lookup.vtable_index(), method_layout.vtable_index());
                assert_eq!(
                    method_layout.return_step_schema(),
                    surface_layout.return_step_schema()
                );

                match dispatch.target() {
                    RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(
                        trampoline,
                    ) => {
                        assert_eq!(
                            trampoline.owner_root_fqn(),
                            "fixtures.build.singleCaseWorker"
                        );
                        assert_eq!(
                            trampoline.owner_continuation_object(),
                            callable.continuation_object()
                        );
                        assert!(trampoline.resume_boundary_sites().is_empty());
                        assert!(trampoline.handle_binder_routes().is_empty());
                    }
                    RefactorContinuationSurfaceResumeDispatchTarget::Unreachable => {
                        panic!("shared schema object-method fixture 不应是 unreachable dispatch")
                    }
                }
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_dispatch_layout_resolves_handle_binder_owner_trampoline() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, module| {
                let query = result.expect("handle-binder schema 应可发布 owner trampoline query");
                let callable = inputs
                    .abi_visibility_program
                    .callable("run")
                    .expect("run callable 应存在");
                let (site_id, contract) = first_handle_dispatch(callable);
                let binder = contract
                    .handled_arms()
                    .iter()
                    .find_map(|arm| arm.continuation_binder())
                    .expect("fixture 应至少包含一个 continuation binder");
                let dispatch = query
                    .surface_resume_dispatch_layout(binder.continuation_schema())
                    .expect("handle-binder schema 的 owner dispatch contract 应可查询");

                assert_eq!(
                    dispatch.source_kind(),
                    crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::HandleContinuationBinderOnly
                );
                assert!(dispatch.method_targets().is_empty());
                match dispatch.target() {
                    RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(
                        trampoline,
                    ) => {
                        assert_eq!(trampoline.owner_root_fqn(), "run");
                        assert_eq!(
                            trampoline.owner_continuation_object(),
                            callable.continuation_object()
                        );
                        assert!(trampoline.resume_boundary_sites().is_empty());
                        assert_eq!(trampoline.handle_binder_routes().len(), 1);
                        assert_eq!(trampoline.handle_binder_routes()[0].site_id(), site_id);
                        assert_eq!(trampoline.handle_binder_routes()[0].arm_ordinal(), 0);
                        assert_eq!(
                            trampoline.handle_binder_routes()[0].handled_case(),
                            CaseTag::new(0)
                        );
                        assert!(module.get_function(trampoline.symbol_name()).is_some());
                    }
                    RefactorContinuationSurfaceResumeDispatchTarget::Unreachable => {
                        panic!("handle-binder-only schema 不应是 unreachable dispatch")
                    }
                }
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_dispatch_layout_resolves_multi_site_resume_owner_trampoline() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, module| {
                let query =
                    result.expect("multi-resume-site schema 应可发布 owner trampoline query");
                let callable = inputs
                    .abi_visibility_program
                    .callable("main")
                    .expect("main callable 应存在");
                let resume_lowering = callable
                    .boundary_map()
                    .entries()
                    .iter()
                    .find_map(|boundary| match boundary.lowering() {
                        Some(LateLoweredBoundaryLowering::Resume(lowering)) => Some(lowering),
                        _ => None,
                    })
                    .expect("fixture 应至少包含一个 resume boundary");
                let resume_schema = resume_lowering.facts().continuation_schema();
                let handle_state = handle_dispatch_state(callable, SiteId::from_raw(0));
                let LateLoweredStateTerminator::HandleDispatch { contract, .. } =
                    handle_state.terminator()
                else {
                    panic!("main site0 应保持 HandleDispatch terminator");
                };
                let binder = contract.handled_arms()[0]
                    .continuation_binder()
                    .expect("Ask handle arm 应发布 continuation binder");
                let dispatch = query
                    .surface_resume_dispatch_layout(resume_schema)
                    .expect("resume schema 的 owner dispatch contract 应可查询");

                assert_eq!(
                    dispatch.source_kind(),
                    crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ResumeBoundaryOnly
                );
                assert!(dispatch.method_targets().is_empty());
                match dispatch.target() {
                    RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(
                        trampoline,
                    ) => {
                        let sites = trampoline
                            .resume_boundary_sites()
                            .iter()
                            .map(|site_id| site_id.as_u32())
                            .collect::<Vec<_>>();
                        assert_eq!(trampoline.owner_root_fqn(), "main");
                        assert_eq!(
                            trampoline.owner_continuation_object(),
                            callable.continuation_object()
                        );
                        assert_eq!(sites, vec![25, 30, 35, 40]);
                        assert!(trampoline.handle_binder_routes().is_empty());
                        let projection = trampoline.wrapper_projection().expect(
                            "shared wrapper schema 应发布 owner-step -> wrapper-step projection",
                        );
                        let outward = projection
                            .outward_cases()
                            .first()
                            .expect("shared wrapper projection 应至少包含一个 outward case");
                        assert_eq!(
                            projection.underlying_route().continuation_schema(),
                            binder.continuation_schema()
                        );
                        assert!(matches!(
                            projection.underlying_route().publication(),
                            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                                owner_continuation_object,
                                site_id,
                                arm_ordinal,
                                handled_case,
                                ..
                            } if *owner_continuation_object == callable.continuation_object()
                                && site_id.as_u32() == 0
                                && *arm_ordinal == 0
                                && *handled_case == contract.handled_arms()[0].handled_case()
                        ));
                        assert_eq!(projection.owner_step_schema(), callable.step_schema());
                        assert_eq!(
                            projection.wrapper_step_schema(),
                            resume_lowering.facts().out_step_schema()
                        );
                        assert_eq!(
                            outward.owner_case_tag().as_u32(),
                            2,
                            "fixture 应把 owner runtime-error case 投影回 wrapper c0"
                        );
                        assert_eq!(outward.wrapper_case_tag().as_u32(), 0);
                        assert!(module.get_function(trampoline.symbol_name()).is_some());
                    }
                    RefactorContinuationSurfaceResumeDispatchTarget::Unreachable => {
                        panic!("resume-boundary-only schema 不应是 unreachable dispatch")
                    }
                }
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_dispatch_layout_rejects_missing_wrapper_projection_contract() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let callable = program.callable("main").expect("main callable 应存在");
                let resume_schema = callable
                    .boundary_map()
                    .entries()
                    .iter()
                    .find_map(|boundary| match boundary.lowering() {
                        Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                            Some(lowering.facts().continuation_schema())
                        }
                        _ => None,
                    })
                    .expect("fixture 应至少包含一个 resume boundary schema");
                let inventory = program
                    .surface_resume_dispatch_inventory()
                    .iter()
                    .map(|entry| {
                        LateLoweredSurfaceResumeDispatchInventoryEntry::new(
                            entry.continuation_schema(),
                            entry.contract(),
                            entry.source_kind(),
                            entry.publications().to_vec(),
                            if entry.continuation_schema() == resume_schema {
                                None
                            } else {
                                entry.wrapper_projection().cloned()
                            },
                        )
                    })
                    .collect::<Vec<_>>();
                program.with_surface_resume_dispatch_inventory(inventory)
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 shared wrapper projection contract 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("owner-step -> wrapper-step projection contract"),
                    "错误消息应指出缺失的是 shared wrapper projection contract: {message}"
                );
                assert!(
                    message.contains("underlying route k3"),
                    "错误消息应指出缺失投影所依赖的 underlying route: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_wrapper_completion_resolves_payload_source() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("refactor ABI materialization 应成功");
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("main")
                    .expect("main callable 应存在");
                let resume_schema = callable
                    .boundary_map()
                    .entries()
                    .iter()
                    .find_map(|boundary| match boundary.lowering() {
                        Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                            Some(lowering.facts().continuation_schema())
                        }
                        _ => None,
                    })
                    .expect("fixture 应包含 shared wrapper resume schema");
                let dispatch = query
                    .surface_resume_dispatch_layout(resume_schema)
                    .expect("shared wrapper dispatch 应可查询");

                let RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) =
                    dispatch.target()
                else {
                    panic!("shared wrapper schema 应发布 owner trampoline");
                };
                let projection = trampoline
                    .wrapper_projection()
                    .expect("shared wrapper schema 应发布 wrapper projection");

                assert_eq!(projection.complete().owner_answer_ty().as_u32(), 2);
                assert_eq!(projection.complete().wrapper_answer_ty().as_u32(), 5);
                assert!(matches!(
                    projection.complete().payload_source(),
                    LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(
                        LateLoweredCompletionPayloadSource::Operand(source)
                    ) if source.source_ty() == projection.complete().wrapper_answer_ty()
                        && matches!(source.value(), LateLoweredOperandValueSource::Local(_))
                ));
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_wrapper_completion_uses_owner_complete_for_matching_answer_type()
     {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_resume_if_else_branch_single_perform.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("refactor ABI materialization 应成功");
                let resume_schema = inputs
                    .abi_visibility_program
                    .surface_resume_dispatch_inventory()
                    .iter()
                    .find_map(|entry| {
                        let projection = entry.wrapper_projection()?;
                        (projection.complete().owner_answer_ty()
                            == projection.complete().wrapper_answer_ty())
                        .then_some(entry.continuation_schema())
                    })
                    .expect("fixture 应包含 owner/wrapper answer type 相同的 wrapper projection");
                let dispatch = query
                    .surface_resume_dispatch_layout(resume_schema)
                    .expect("shared wrapper dispatch 应可查询");

                let RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(trampoline) =
                    dispatch.target()
                else {
                    panic!("shared wrapper schema 应发布 owner trampoline");
                };
                let projection = trampoline
                    .wrapper_projection()
                    .expect("shared wrapper schema 应发布 wrapper projection");

                assert_eq!(
                    projection.complete().owner_answer_ty(),
                    projection.complete().wrapper_answer_ty()
                );
                assert!(matches!(
                    projection.complete().payload_source(),
                    LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { answer_ty }
                        if *answer_ty == projection.complete().wrapper_answer_ty()
                ));
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_wrapper_completion_rejects_type_drift() {
        with_phase_fixture_query_result(
            "run-pass",
            "effect_multi_escape_indirect_direct_while.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let callable = program.callable("main").expect("main callable 应存在");
                let resume_schema = callable
                    .boundary_map()
                    .entries()
                    .iter()
                    .find_map(|boundary| match boundary.lowering() {
                        Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                            Some(lowering.facts().continuation_schema())
                        }
                        _ => None,
                    })
                    .expect("fixture 应至少包含一个 resume boundary schema");
                let inventory = program
                    .surface_resume_dispatch_inventory()
                    .iter()
                    .map(|entry| {
                        let wrapper_projection = if entry.continuation_schema() == resume_schema {
                            entry.wrapper_projection().map(|projection| {
                                LateLoweredSurfaceResumeWrapperProjection::new(
                                    projection.underlying_route().clone(),
                                    projection.owner_step_schema(),
                                    projection.wrapper_step_schema(),
                                    LateLoweredSurfaceResumeWrapperCompleteProjection::new(
                                        projection.complete().owner_answer_ty(),
                                        projection.complete().wrapper_answer_ty(),
                                        LateLoweredSurfaceResumeWrapperCompletePayloadSource::wrapper_payload(
                                            LateLoweredCompletionPayloadSource::unit(
                                                projection.complete().wrapper_answer_ty(),
                                            ),
                                        ),
                                    ),
                                    projection.outward_cases().to_vec(),
                                )
                            })
                        } else {
                            entry.wrapper_projection().cloned()
                        };
                        LateLoweredSurfaceResumeDispatchInventoryEntry::new(
                            entry.continuation_schema(),
                            entry.contract(),
                            entry.source_kind(),
                            entry.publications().to_vec(),
                            wrapper_projection,
                        )
                    })
                    .collect::<Vec<_>>();
                program.with_surface_resume_dispatch_inventory(inventory)
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => {
                        panic!("non-Unit wrapper answer 的 Unit payload source 必须 fail fast")
                    }
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("wrapper complete payload")
                        || message.contains("wrapper-step projection contract 漂移"),
                    "错误消息应指出 wrapper complete payload contract 漂移: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_dispatch_layout_rejects_missing_internal_method_target() {
        with_fixture_query_result(
            "effect_refactor_step_enum_single_case.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let callable = program
                    .callable("fixtures.build.singleCaseWorker")
                    .expect("callable 应存在");
                let continuation_objects = program
                    .continuation_objects()
                    .iter()
                    .map(|candidate| {
                        if candidate.object_id() == callable.continuation_object() {
                            clone_continuation_object_with_methods(candidate, Vec::new())
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();

                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    continuation_objects,
                    program.callables().to_vec(),
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("缺失 internal method target 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("ContinuationObjectMethod"),
                    "错误消息应指出 source kind 与 method target 缺失的关系: {message}"
                );
                assert!(
                    message.contains("reachable internal method target"),
                    "错误消息应指出缺失的是 reachable internal method target: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_dispatch_layout_keeps_multi_method_lookup_set() {
        with_phase_fixture_query_result(
            "effect_facts",
            "dynamic_fallback_widening.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, module| {
                let query = result.expect("多 method 共享 schema 应可发布 owner dispatch contract");
                let callable = inputs
                    .abi_visibility_program
                    .callable("sample.callValue")
                    .expect("sample.callValue callable 应存在");
                let step = inputs
                    .abi_visibility_program
                    .step_type(callable.step_schema())
                    .expect("callValue step shell 应存在");
                let shared_schema = step
                    .case(CaseTag::new(0))
                    .expect("c0 应存在")
                    .continuation_schema();
                let dispatch = query
                    .surface_resume_dispatch_layout(shared_schema)
                    .expect("多 method 共享 schema 的 dispatch contract 应可查询");
                let method_keys = dispatch
                    .method_targets()
                    .iter()
                    .map(|lookup| {
                        (
                            lookup.packing_interface_id().as_u32(),
                            lookup.case_tag().as_u32(),
                        )
                    })
                    .collect::<Vec<_>>();

                assert_eq!(
                    dispatch.source_kind(),
                    crate::effect_lowered::ir::LateLoweredSurfaceResumeDispatchSourceKind::ContinuationObjectMethod
                );
                assert_eq!(method_keys, vec![(0, 0), (1, 1)]);
                match dispatch.target() {
                    RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(
                        trampoline,
                    ) => {
                        assert_eq!(trampoline.owner_root_fqn(), "sample.callValue");
                        assert_eq!(
                            trampoline.owner_continuation_object(),
                            callable.continuation_object()
                        );
                        assert!(module.get_function(trampoline.symbol_name()).is_some());
                    }
                    RefactorContinuationSurfaceResumeDispatchTarget::Unreachable => {
                        panic!("多 method 共享 schema 不应是 unreachable dispatch")
                    }
                }
            },
        );
    }

    #[test]
    fn refactor_llvm_surface_resume_dispatch_layout_rejects_multi_object_publication() {
        with_fixture_query_result(
            "effect_refactor_step_enum_single_case.scoop",
            |inputs| {
                let program = &inputs.abi_visibility_program;
                let callable = program
                    .callable("fixtures.build.singleCaseWorker")
                    .expect("callable 应存在");
                let next_object_id = ContinuationObjectId::new(
                    program
                        .continuation_objects()
                        .iter()
                        .map(|object| object.object_id().as_u32())
                        .max()
                        .map(|raw| raw.saturating_add(1))
                        .unwrap_or(0),
                );
                let duplicated_object = program
                    .continuation_object(callable.continuation_object())
                    .map(|object| clone_continuation_object_with_id(object, next_object_id))
                    .expect("continuation object 应存在");
                let mut continuation_objects = program.continuation_objects().to_vec();
                continuation_objects.push(duplicated_object);

                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    continuation_objects,
                    program.callables().to_vec(),
                )
            },
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("多 object 共享同一 schema 时必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("多个 continuation object 共享同一 schema"),
                    "错误消息应指出 multi-object publication 歧义: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_layout_binds_pure_direct_entries_without_legacy_typestore() {
        with_fixture_query(
            "effect_refactor_dynamic_entry_publication_emit_llvm.scoop",
            |inputs, query, _module| {
                let lambda_root = inputs
                    .abi_visibility_program
                    .callables()
                    .iter()
                    .find(|callable| callable.root_fqn().contains("$lambda"))
                    .map(|callable| callable.root_fqn().to_string())
                    .expect("fixture 应发布 lambda callable shell");
                let roots = vec![
                    "fixtures.build.makeClosure".to_string(),
                    "fixtures.build.Base.ping".to_string(),
                    lambda_root,
                ];
                let mut saw_scalar_invoke = false;
                let mut saw_tuple_invoke = false;

                for root in roots {
                    let callable = query
                        .callable_layout_by_root_fqn(&root)
                        .expect("callable layout 应存在");
                    let invoke_args = query
                        .source_value_layout(callable.direct_entry().invoke_args_tuple_ty())
                        .expect(
                            "direct entry invoke_args_tuple_ty 应发布 source-type ABI contract",
                        );
                    assert_eq!(
                        callable.direct_entry().param_count(),
                        usize::from(!invoke_args.abi().is_elided()),
                        "direct entry 形参个数必须由 published invoke carrier 是否零载荷唯一决定: {root}"
                    );
                    assert!(!invoke_args.abi().is_elided());
                    match invoke_args.kind() {
                        RefactorSourceAbiLayoutKind::Scalar => {
                            saw_scalar_invoke = true;
                            assert!(
                                invoke_args.fields().is_empty(),
                                "single-value invoke carrier 不应伪装成 tuple field 映射: {root}"
                            );
                        }
                        RefactorSourceAbiLayoutKind::Tuple => {
                            saw_tuple_invoke = true;
                            assert!(
                                !invoke_args.fields().is_empty(),
                                "tuple invoke carrier 至少应发布一个 source field: {root}"
                            );
                            for (idx, field) in invoke_args.fields().iter().enumerate() {
                                assert_eq!(field.source_index(), idx as u32);
                                assert_eq!(field.abi_field_index(), Some(idx as u32));
                                assert!(!field.is_elided());
                            }
                        }
                    }
                }

                assert!(
                    saw_scalar_invoke,
                    "fixture 应至少覆盖一个 single-value invoke carrier"
                );
                assert!(
                    saw_tuple_invoke,
                    "fixture 应至少覆盖一个 tuple invoke carrier"
                );

                let make_closure = query
                    .callable_layout_by_root_fqn("fixtures.build.makeClosure")
                    .expect("makeClosure callable layout 应存在");
                let complete_layout = query
                    .source_value_layout(
                        query
                            .step_layout(make_closure.step_schema())
                            .expect("step layout 应存在")
                            .complete_variant()
                            .payload_source_ty(),
                    )
                    .expect("complete payload source type 应发布 source-type ABI contract");
                assert_eq!(complete_layout.kind(), RefactorSourceAbiLayoutKind::Scalar);
                assert!(complete_layout.fields().is_empty());
                assert!(!complete_layout.abi().is_elided());
            },
        );
    }

    #[test]
    fn refactor_llvm_layout_resolves_unit_case_payload_contract() {
        with_fixture_query(
            "effect_refactor_dynamic_invoke_unit_payload.scoop",
            |inputs, query, _module| {
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("fixtures.build.unitWorker")
                    .expect("unitWorker callable 应存在");
                let step_layout = query
                    .step_layout(callable.step_schema())
                    .expect("step layout 应存在");
                let case_variant = step_layout
                    .case_layout(CaseTag::new(0))
                    .expect("case0 layout 应存在")
                    .variant();
                let case_payload_layout = query
                    .source_value_layout(case_variant.payload_source_ty())
                    .expect("case payload source type 应发布 source-type ABI contract");
                let complete_layout = query
                    .source_value_layout(step_layout.complete_variant().payload_source_ty())
                    .expect("complete payload source type 应发布 source-type ABI contract");

                assert_eq!(
                    case_payload_layout.kind(),
                    RefactorSourceAbiLayoutKind::Scalar
                );
                assert!(case_payload_layout.abi().is_elided());
                assert!(case_payload_layout.fields().is_empty());
                assert!(case_variant.payload_is_elided());
                assert_eq!(case_variant.payload_field_count(), 1);
                assert!(complete_layout.abi().is_elided());
                assert_eq!(step_layout.complete_variant().payload_field_count(), 0);
            },
        );
    }

    #[test]
    fn refactor_llvm_layout_resolves_tuple_resume_payload_and_answer_contract() {
        with_phase_fixture_query_result(
            "run-pass",
            "continuation_resume_surface_named_tuple_and_unit_basic.scoop",
            |inputs| inputs.abi_visibility_program.clone(),
            |inputs, result, _module| {
                let query = result.expect("tuple resume fixture 应可发布 source-type ABI contract");
                let pair_surface = inputs
                    .abi_visibility_program
                    .continuation_objects()
                    .iter()
                    .flat_map(|object| object.surface_resumes().iter())
                    .find(|surface| {
                        inputs
                            .effect_lowered_stage_output
                            .types()
                            .display(surface.resume_tuple_ty())
                            .to_string()
                            == "(Int, String)"
                    })
                    .expect("fixture 应包含 tuple resume surface");
                let surface_layout = query
                    .surface_resume_layout(pair_surface.continuation_schema())
                    .expect("surface-resume layout 应可查询");
                let resume_payload_layout = query
                    .source_value_layout(surface_layout.resume_tuple_ty())
                    .expect("resume tuple source type 应发布 source-type ABI contract");
                let answer_layout = query
                    .source_value_layout(surface_layout.answer_ty())
                    .expect("resume answer source type 应发布 source-type ABI contract");

                assert_eq!(
                    resume_payload_layout.kind(),
                    RefactorSourceAbiLayoutKind::Tuple
                );
                assert_eq!(resume_payload_layout.fields().len(), 2);
                assert_eq!(resume_payload_layout.abi_field_count(), 2);
                assert_eq!(resume_payload_layout.fields()[0].source_index(), 0);
                assert_eq!(resume_payload_layout.fields()[0].abi_field_index(), Some(0));
                assert_eq!(resume_payload_layout.fields()[1].source_index(), 1);
                assert_eq!(resume_payload_layout.fields()[1].abi_field_index(), Some(1));
                assert!(!resume_payload_layout.fields()[0].is_elided());
                assert!(!resume_payload_layout.fields()[1].is_elided());
                assert_eq!(answer_layout.kind(), RefactorSourceAbiLayoutKind::Scalar);
                assert!(answer_layout.abi().is_elided());
            },
        );
    }

    #[test]
    fn refactor_llvm_layout_rejects_unlowerable_invoke_args_type() {
        let inputs =
            build_fixture_inputs("effect_refactor_dynamic_entry_publication_emit_llvm.scoop");
        let mut source_types = inputs.effect_lowered_stage_output.types().clone();
        let param_ty = source_types.ty_param(TypeParamType {
            name: "SyntheticInvokeArgs".to_string(),
            decl_file: std::path::PathBuf::from("tests/p6_t02i.synthetic"),
            decl_span: dummy_span(),
        });

        with_inputs_query_result_for_source_types(
            inputs,
            move |inputs| {
                let program = &inputs.abi_visibility_program;
                let callables = program
                    .callables()
                    .iter()
                    .map(|candidate| {
                        if candidate.root_fqn() == "fixtures.build.makeClosure" {
                            clone_callable_with_dynamic_invoke_entry(
                                candidate,
                                LateLoweredDynamicInvokeEntry::new(
                                    param_ty,
                                    candidate.dynamic_invoke_entry().step_schema(),
                                    candidate.dynamic_invoke_entry().entry_state(),
                                    candidate.dynamic_invoke_entry().complete_state(),
                                ),
                            )
                        } else {
                            candidate.clone()
                        }
                    })
                    .collect();
                LateLoweredProgram::new(
                    program.step_types().to_vec(),
                    program.resume_packings().to_vec(),
                    program.continuation_objects().to_vec(),
                    callables,
                )
            },
            move |_inputs| source_types,
            |_inputs, result, _module| {
                let err = match result {
                    Ok(_) => panic!("不可 lowering 的 synthetic invoke args type 必须 fail fast"),
                    Err(err) => err,
                };
                let message = err.to_string();
                assert!(
                    message.contains("source-type ABI value lowering"),
                    "错误消息应指出缺失的是 source-type ABI lowering contract: {message}"
                );
                assert!(
                    message.contains("SyntheticInvokeArgs"),
                    "错误消息应指出不可 lowering 的 synthetic source type: {message}"
                );
                assert!(
                    message.contains("尚未实例化的类型参数"),
                    "错误消息应明确拒绝未实例化类型参数: {message}"
                );
            },
        );
    }

    #[test]
    fn refactor_llvm_unit_abi_elides_zero_sized_args_and_resume_payloads() {
        with_fixture_query_result(
            "effect_refactor_dynamic_invoke_unit_payload.scoop",
            unit_worker_program_with_ping_interface,
            |inputs, result, module| {
                let query = result.expect("published unit resume packing 应可物化 ABI");
                let callable = inputs
                    .effect_lowered_stage_output
                    .program()
                    .callable("fixtures.build.unitWorker")
                    .expect("callable 应存在");
                let callable_layout = query
                    .callable_layout(callable.step_schema())
                    .expect("callable layout 应可查询");
                let step_layout = query
                    .step_layout(callable.step_schema())
                    .expect("step layout 应可查询");
                let continuation_object = inputs
                    .effect_lowered_stage_output
                    .program()
                    .continuation_object(callable.continuation_object())
                    .expect("continuation object 应存在");
                let interface_id = *query
                    .callable_layout(callable.step_schema())
                    .expect("callable layout 应可查询")
                    .resume_packings()
                    .iter()
                    .find(|interface_id| {
                        query
                            .resume_packing_layout(**interface_id)
                            .is_some_and(|interface| {
                                interface.packing_family_fqn() == "fixtures.build.Ping"
                            })
                    })
                    .expect("应存在 Ping resume packing");
                let interface_layout = query
                    .resume_packing_layout(interface_id)
                    .expect("resume packing layout 应可查询");
                let method_layout = interface_layout
                    .method(CaseTag::new(0))
                    .expect("case0 method 应存在");
                let surface_resume_schema = continuation_object
                    .surface_resumes()
                    .iter()
                    .find(|surface| surface.case_tag() == CaseTag::new(0))
                    .expect("case0 surface resume 应存在")
                    .continuation_schema();
                let surface_layout = query
                    .surface_resume_layout(surface_resume_schema)
                    .expect("surface-resume layout 应可查询");

                assert!(callable_layout.dynamic_entry().args_abi().is_elided());
                assert!(callable_layout.direct_entry().args_abi().is_elided());
                assert_eq!(callable_layout.dynamic_entry().param_count(), 0);
                assert_eq!(callable_layout.direct_entry().param_count(), 0);
                assert!(step_layout.complete_variant().payload_is_elided());
                assert_eq!(step_layout.complete_variant().payload_field_count(), 0);
                assert!(method_layout.resume_payload_abi().is_elided());
                assert_eq!(method_layout.param_count(), 1);
                assert!(surface_layout.resume_payload_abi().is_elided());
                assert_eq!(surface_layout.param_count(), 1);
                assert_eq!(
                    step_layout
                        .case_layout(CaseTag::new(0))
                        .expect("case0 layout 应存在")
                        .variant()
                        .payload_field_count(),
                    1
                );
                assert!(
                    module
                        .get_function(callable_layout.dynamic_entry().symbol_name())
                        .is_some()
                );
                assert!(module.get_function(surface_layout.symbol_name()).is_some());
            },
        );
    }
}
