//! `CallableEmitter` definition: per-callable state machine emitter
//! that the `MainCodegen` entry layer constructs and drives. The constructor
//! captures the published late-lowered callable, allocates state blocks, and
//! sets up the frame layout caches. Other methods on this type live in the
//! sibling submodules grouped by concern.

use super::*;

pub(super) struct CallableEmitter<'cg, 'a, 'ctx> {
    pub(super) codegen: &'cg mut MainCodegen<'a, 'ctx>,
    pub(super) program: &'a LateLoweredProgram,
    pub(super) source_types: &'a TypeStore,
    pub(super) abi: &'cg ProgramAbiQuery<'ctx>,
    pub(super) callable: &'a LateLoweredCallable,
    pub(super) mir_fun: &'a LateLoweredSourceCallable,
    pub(super) body: &'a LateLoweredSourceBody,
    pub(super) function: FunctionValue<'ctx>,
    pub(super) slots: Vec<MirLocalSlot<'ctx>>,
    pub(super) used_locals: HashSet<LocalId>,
    pub(super) abi_step_schema: StepSchemaId,
    pub(super) frame_layout: &'cg FrameLayout<'ctx>,
    pub(super) step_layout: &'cg StepLayout<'ctx>,
    pub(super) frame_root_slot: PointerValue<'ctx>,
    pub(super) state_blocks: BTreeMap<StateId, BasicBlock<'ctx>>,
    pub(super) return_projection:
        Option<&'cg crate::effect_lowered::ir::LateLoweredSurfaceResumeWrapperProjection>,
    pub(super) return_step_schema: Option<StepSchemaId>,
    pub(super) surface_resume_handle_sites: Option<BTreeSet<SiteId>>,
    pub(super) handle_completion_mode: HandleCompletionMode,
    pub(super) return_mode: CallableReturnMode,
}

pub(super) struct ComposedBoundaryDispatchContext<'a> {
    pub(super) call_lowering: Option<&'a LateLoweredCallBoundaryLowering>,
    pub(super) dispatch: &'a LateLoweredStepDispatchPlan,
    pub(super) continuation_compositions: &'a [LateLoweredCallBoundaryContinuationComposition],
}

impl<'cg, 'a, 'ctx> CallableEmitter<'cg, 'a, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        codegen: &'cg mut MainCodegen<'a, 'ctx>,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        abi: &'cg ProgramAbiQuery<'ctx>,
        callable: &'a LateLoweredCallable,
        mir_fun: &'a LateLoweredSourceCallable,
        body: &'a LateLoweredSourceBody,
        function: FunctionValue<'ctx>,
        return_projection: Option<
            &'cg crate::effect_lowered::ir::LateLoweredSurfaceResumeWrapperProjection,
        >,
        return_step_schema: Option<StepSchemaId>,
        surface_resume_handle_sites: Option<BTreeSet<SiteId>>,
        handle_completion_mode: HandleCompletionMode,
    ) -> Result<Self, LlvmEmitError> {
        if let Some(callable_layout) = callable
            .effect_step_abi()
            .map(|_| abi.callable_layout_by_version_key(callable.body_version_key()))
            .transpose()?
            && callable_layout.root_fqn() != callable.root_fqn()
        {
            return Err(frontend_error(format!(
                "body lowering callable `{}` 的 ABI layout root 漂移：layout=`{}`",
                callable.root_fqn(),
                callable_layout.root_fqn(),
            )));
        }
        let body_step_schema = callable.body_step_schema().ok_or_else(|| {
            frontend_error(format!(
                "body lowering callable `{}` 缺少 control-body step schema",
                callable.root_fqn()
            ))
        })?;
        let abi_step_schema = abi
            .callable_layout_by_version_key(callable.body_version_key())
            .map(|layout| layout.step_schema())
            .or_else(|_| {
                abi.local_effect_step_schema_by_version_key(callable.body_version_key())
                    .ok_or_else(|| frontend_error("missing local effect ABI schema".to_string()))
            })
            .unwrap_or(body_step_schema);
        let abi_step_schema = if program.step_type(abi_step_schema).is_some() {
            abi_step_schema
        } else {
            body_step_schema
        };
        let frame_layout = abi.frame_layout(abi_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "body lowering 缺少 callable `{}` 的 ABI frame layout s{}",
                callable.root_fqn(),
                abi_step_schema.as_u32()
            ))
        })?;
        let step_layout = abi.step_layout(abi_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "body lowering 缺少 callable `{}` 的 ABI step layout s{}",
                callable.root_fqn(),
                abi_step_schema.as_u32()
            ))
        })?;
        let slots = codegen.create_mir_local_slots(body, source_types)?;
        codegen.current_source_id = codegen.source_id_for_path(
            callable
                .body_version_key()
                .surface_instance()
                .template
                .source_path
                .as_path(),
            mir_fun.span,
        )?;
        let used_locals = super::super::super::mir_body::collect_mir_local_uses(body);
        let frame_root_slot = codegen.create_gc_root_slot(mir_fun.span, "frame_root")?;
        let mut state_blocks = BTreeMap::new();
        for state in callable.state_graph().states() {
            if state_blocks
                .insert(
                    state.state_id(),
                    codegen.context.append_basic_block(
                        function,
                        &format!("lowered.st{}", state.state_id().as_u32()),
                    ),
                )
                .is_some()
            {
                return Err(frontend_error(format!(
                    "body verifier 发现 callable `{}` 重复发布 state st{}",
                    callable.root_fqn(),
                    state.state_id().as_u32()
                )));
            }
        }
        let emitter = Self {
            codegen,
            program,
            source_types,
            abi,
            callable,
            mir_fun,
            body,
            function,
            slots,
            used_locals,
            abi_step_schema,
            frame_layout,
            step_layout,
            frame_root_slot,
            state_blocks,
            return_projection,
            return_step_schema,
            surface_resume_handle_sites,
            handle_completion_mode,
            return_mode: CallableReturnMode::Step,
        };
        emitter.verify_body_contract()?;
        Ok(emitter)
    }

    pub(super) fn value_primitives(&mut self) -> ValuePrimitives<'_, 'a, 'ctx> {
        ValuePrimitives::new(
            &mut *self.codegen,
            self.program,
            self.callable.plain_abi().map(|plain| plain.call_sites()),
            self.source_types,
            self.body,
            &self.slots,
            self.abi,
        )
    }
}
