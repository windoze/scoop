//! Refactor LLVM body lowering（P6-T03）。
//!
//! This module lowers the P5 late-lowered state graph directly.  Generic MIR
//! lowering is reused only for effect-neutral source slices; every boundary,
//! resume payload binding, completion payload, and state transition comes from
//! the published late-lowered / ABI query contract.

use std::collections::{BTreeMap, HashSet};

use inkwell::AddressSpace;
use inkwell::basic_block::BasicBlock;
use inkwell::types::{BasicTypeEnum, StructType};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValueEnum, FunctionValue, IntValue, PointerValue,
};

use crate::effect_facts::{CaseTag, StepSchemaId};
use crate::effect_lowered::LateLoweredProgram;
use crate::effect_lowered::ir::{
    BoundaryId, LateLoweredBoundary, LateLoweredBoundaryLowering, LateLoweredBoundarySource,
    LateLoweredBoundarySourceConsumption, LateLoweredCallable, LateLoweredCompletionPayloadSource,
    LateLoweredHandleBoundaryCaseRoutingAction, LateLoweredOperandSource,
    LateLoweredOperandValueSource, LateLoweredResumePayloadBinding, LateLoweredState,
    LateLoweredStateTerminator, StateId,
};
use crate::llvm::LlvmEmitError;
use crate::mir::{self, LocalId, SiteId};
use crate::ty::{TypeId, TypeStore};

use super::super::MainCodegen;
use super::super::mir_body::MirLocalSlot;
use super::super::types::{CgTy, CgValue};
use super::types::{
    RefactorAbiQuery, RefactorCallTargetQuery, RefactorCallableEntryLayout,
    RefactorContinuationSurfaceResumeDispatchTarget, RefactorContinuationSurfaceResumeLayout,
    RefactorFrameLayout, RefactorSourceAbiLayoutKind, RefactorStepCaseLayout, RefactorStepLayout,
    RefactorStepVariantLayout,
};

const STEP_TAG_COMPLETE: u64 = 0;
const CONT_FIELD_CAPTURED_FRAME: u32 = 1;
const CONT_FIELD_RESUME_STATE: u32 = 2;
const CONT_FIELD_ONE_SHOT: u32 = 3;

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// Defines all refactor ABI function bodies published by the P5/P6 handoff.
    pub(crate) fn codegen_refactor_program_bodies(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &RefactorAbiQuery<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        for callable in program.callables() {
            let mut child = self.fresh_child_codegen();
            child.codegen_refactor_callable_entries(
                program,
                source_types,
                pass_view,
                abi,
                callable,
            )?;
        }
        for interface in program.resume_packings() {
            let packing = abi
                .resume_packing_layout(interface.interface_id())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor body lowering 缺少 resume packing ri{} 的 ABI layout",
                        interface.interface_id().as_u32()
                    ))
                })?;
            for method in interface.methods() {
                let method_layout = packing.method(method.case_tag()).ok_or_else(|| {
                    frontend_error(format!(
                        "refactor body lowering 缺少 resume packing ri{} case c{} method layout",
                        interface.interface_id().as_u32(),
                        method.case_tag().as_u32()
                    ))
                })?;
                let callable = program
                    .callables()
                    .iter()
                    .find(|callable| callable.step_schema() == method.out_step_schema())
                    .ok_or_else(|| frontend_error(format!(
                        "refactor body lowering 缺少 resume method case c{} 的 owner step schema s{} callable",
                        method.case_tag().as_u32(),
                        method.out_step_schema().as_u32()
                    )))?;
                let mut child = self.fresh_child_codegen();
                child.codegen_refactor_resume_method(
                    program,
                    source_types,
                    pass_view,
                    abi,
                    callable,
                    method_layout.symbol_name(),
                    method_layout.llvm_ty(),
                    method.case_tag(),
                    method.resume_tuple_ty(),
                )?;
            }
        }
        for entry in program.surface_resume_dispatch_inventory() {
            let surface = abi
                .surface_resume_layout(entry.continuation_schema())
                .ok_or_else(|| frontend_error(format!(
                    "refactor body lowering 缺少 continuation schema k{} 的 surface-resume layout",
                    entry.continuation_schema().as_u32()
                )))?;
            let dispatch = abi.surface_resume_dispatch_layout(entry.continuation_schema())?;
            if let RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(target) =
                dispatch.target()
            {
                let mut child = self.fresh_child_codegen();
                child.codegen_refactor_surface_resume_owner_trampoline(
                    program,
                    source_types,
                    pass_view,
                    abi,
                    surface,
                    target,
                )?;
            }
            let mut child = self.fresh_child_codegen();
            child.codegen_refactor_surface_resume(program, abi, surface)?;
        }
        Ok(())
    }

    /// Emits the C `main` exit path through the refactor direct-entry ABI.
    pub(crate) fn codegen_refactor_main_exit_code(
        &mut self,
        hir_main: &crate::hir::FunDecl,
        entry_argv_array: Option<PointerValue<'ctx>>,
        program: &LateLoweredProgram,
        abi: &RefactorAbiQuery<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        if entry_argv_array.is_some() {
            return Err(frontend_error(
                "refactor LLVM main wrapper 尚未发布 Array<String> argv tuple ABI".to_string(),
            ));
        }
        let callable = program.callable(&hir_main.fqn).ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM main wrapper 缺少入口 `{}` 的 callable body",
                hir_main.fqn
            ))
        })?;
        let layout = abi.callable_layout_by_version_key(callable.body_version_key())?;
        let direct = self.refactor_function(layout.direct_entry().symbol_name())?;
        let mut args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        if !layout.direct_entry().args_abi().is_elided() {
            args.push(
                layout
                    .direct_entry()
                    .args_abi()
                    .llvm_ty()
                    .const_zero()
                    .into(),
            );
        }
        let call = self
            .builder
            .build_call(direct, &args, "refactor_main_step")?;
        let step = call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error("refactor main direct entry 未返回 Step_F".to_string())
        })?;
        let step_layout = abi.step_layout(callable.step_schema()).ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM main wrapper 缺少入口 step schema s{} layout",
                callable.step_schema().as_u32()
            ))
        })?;
        let tag = self.refactor_extract_step_tag(step_layout, step)?;
        let ok_bb = self
            .context
            .append_basic_block(self.current_function()?, "refactor_main_complete");
        let bad_bb = self
            .context
            .append_basic_block(self.current_function()?, "refactor_main_unhandled");
        let done_bb = self
            .context
            .append_basic_block(self.current_function()?, "refactor_main_done");
        let is_complete = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            tag,
            self.context.i32_type().const_int(STEP_TAG_COMPLETE, false),
            "refactor_main_is_complete",
        )?;
        self.builder
            .build_conditional_branch(is_complete, ok_bb, bad_bb)?;

        self.builder.position_at_end(bad_bb);
        self.builder.build_unreachable()?;

        self.builder.position_at_end(ok_bb);
        let exit_value = match self.cg_ty_of(hir_main.return_ty) {
            Some(CgTy::Unit) => self.context.i32_type().const_zero(),
            Some(CgTy::Int(_)) => {
                let payload = self.refactor_extract_step_payload(
                    step_layout,
                    step,
                    step_layout.complete_variant(),
                    "refactor_main_complete_payload",
                )?;
                match payload {
                    Some(BasicValueEnum::IntValue(value)) => {
                        self.builder.build_int_truncate_or_bit_cast(
                            value,
                            self.context.i32_type(),
                            "refactor_main_exit_i32",
                        )?
                    }
                    Some(_) => {
                        return Err(frontend_error(
                            "refactor main Complete payload 不是整数值".to_string(),
                        ));
                    }
                    None => self.context.i32_type().const_zero(),
                }
            }
            _ => {
                return Err(frontend_error(format!(
                    "refactor main wrapper 不支持入口 `{}` 的返回类型",
                    hir_main.fqn
                )));
            }
        };
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.context.i32_type(), "refactor_main_exit")?;
        phi.add_incoming(&[(&exit_value, ok_bb)]);
        Ok(phi.as_basic_value().into_int_value())
    }

    fn codegen_refactor_callable_entries(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &RefactorAbiQuery<'ctx>,
        callable: &'a LateLoweredCallable,
    ) -> Result<(), LlvmEmitError> {
        let layout = abi.callable_layout_by_version_key(callable.body_version_key())?;
        let direct_fun = self.refactor_function(layout.direct_entry().symbol_name())?;
        if direct_fun.count_basic_blocks() == 0 {
            let mir_fun = refactor_mir_callable(pass_view, callable.root_fqn())?;
            let body = mir_fun.body.as_ref().ok_or_else(|| {
                frontend_error(format!(
                    "refactor body lowering callable `{}` 缺少 canonical MIR body",
                    callable.root_fqn()
                ))
            })?;
            let entry = self.context.append_basic_block(direct_fun, "entry");
            self.builder.position_at_end(entry);
            self.begin_function_explicit_frame_layout(direct_fun)?;
            RefactorCallableEmitter::new(
                self,
                program,
                source_types,
                abi,
                callable,
                mir_fun,
                body,
                direct_fun,
                None,
            )?
            .emit_direct(layout.direct_entry())?;
            self.finish_function_explicit_frame_layout(mir_fun.span)?;
        }

        let dynamic_fun = self.refactor_function(layout.dynamic_entry().symbol_name())?;
        if dynamic_fun.count_basic_blocks() == 0 {
            let entry = self.context.append_basic_block(dynamic_fun, "entry");
            self.builder.position_at_end(entry);
            let mut args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
            if layout.dynamic_entry().param_count() > 0 {
                let arg = dynamic_fun.get_nth_param(0).ok_or_else(|| {
                    frontend_error(format!(
                        "refactor dynamic entry `{}` 缺少 args tuple 参数",
                        layout.dynamic_entry().symbol_name()
                    ))
                })?;
                args.push(arg.into());
            }
            let call = self
                .builder
                .build_call(direct_fun, &args, "refactor_dynamic_to_direct")?;
            let value = call.try_as_basic_value().basic().ok_or_else(|| {
                frontend_error(format!(
                    "refactor direct entry `{}` 未返回 Step_F",
                    layout.direct_entry().symbol_name()
                ))
            })?;
            self.builder.build_return(Some(&value))?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn codegen_refactor_resume_method(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &RefactorAbiQuery<'ctx>,
        callable: &'a LateLoweredCallable,
        symbol_name: &str,
        fn_ty: inkwell::types::FunctionType<'ctx>,
        case_tag: CaseTag,
        resume_tuple_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        let function = self
            .module
            .get_function(symbol_name)
            .unwrap_or_else(|| self.module.add_function(symbol_name, fn_ty, None));
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let mir_fun = refactor_mir_callable(pass_view, callable.root_fqn())?;
        let body = mir_fun.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "refactor resume method `{symbol_name}` owner `{}` 缺少 canonical MIR body",
                callable.root_fqn()
            ))
        })?;
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(function)?;
        RefactorCallableEmitter::new(
            self,
            program,
            source_types,
            abi,
            callable,
            mir_fun,
            body,
            function,
            None,
        )?
        .emit_resume_method(case_tag, resume_tuple_ty)?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
        Ok(())
    }

    fn codegen_refactor_surface_resume(
        &mut self,
        _program: &LateLoweredProgram,
        abi: &RefactorAbiQuery<'ctx>,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self
            .module
            .get_function(surface.symbol_name())
            .unwrap_or_else(|| {
                self.module
                    .add_function(surface.symbol_name(), surface.llvm_ty(), None)
            });
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let dispatch = abi.surface_resume_dispatch_layout(surface.continuation_schema())?;
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        let target = match dispatch.target() {
            RefactorContinuationSurfaceResumeDispatchTarget::OwnerTrampoline(target) => target,
            RefactorContinuationSurfaceResumeDispatchTarget::Unreachable => {
                self.builder.build_unreachable()?;
                return Ok(());
            }
        };
        let cont = function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "refactor surface resume `{}` 缺少 continuation 参数",
                surface.symbol_name()
            ))
        })?;
        let mut args = vec![cont.into()];
        if surface.param_count() > 1 {
            let payload = function.get_nth_param(1).ok_or_else(|| {
                frontend_error(format!(
                    "refactor surface resume `{}` 缺少 resume payload 参数",
                    surface.symbol_name()
                ))
            })?;
            args.push(payload.into());
        }
        let trampoline_fun = self.refactor_function(target.symbol_name())?;
        let call =
            self.builder
                .build_call(trampoline_fun, &args, "refactor_surface_resume_call")?;
        let owner_step = call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error(format!(
                "refactor surface resume `{}` 调用 owner dispatch 未返回 Step_F",
                surface.symbol_name()
            ))
        })?;
        self.builder.build_return(Some(&owner_step))?;
        Ok(())
    }

    fn codegen_refactor_surface_resume_owner_trampoline(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &RefactorAbiQuery<'ctx>,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        target: &super::types::RefactorContinuationSurfaceResumeOwnerTrampolineLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self
            .module
            .get_function(target.symbol_name())
            .unwrap_or_else(|| {
                self.module
                    .add_function(target.symbol_name(), target.llvm_ty(), None)
            });
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let callable = program
            .callable_by_version_key(target.owner_version_key())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor surface resume owner dispatch k{} 缺少 owner callable {:?}",
                    surface.continuation_schema().as_u32(),
                    target.owner_version_key()
                ))
            })?;
        if callable.step_schema() != target.owner_step_schema() {
            return Err(frontend_error(format!(
                "refactor surface resume owner dispatch k{} owner step schema 漂移：callable=s{} target=s{}",
                surface.continuation_schema().as_u32(),
                callable.step_schema().as_u32(),
                target.owner_step_schema().as_u32()
            )));
        }
        let mir_fun = refactor_mir_callable(pass_view, callable.root_fqn())?;
        let body = mir_fun.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "refactor surface resume owner dispatch `{}` owner `{}` 缺少 canonical MIR body",
                target.symbol_name(),
                callable.root_fqn()
            ))
        })?;
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(function)?;
        RefactorCallableEmitter::new(
            self,
            program,
            source_types,
            abi,
            callable,
            mir_fun,
            body,
            function,
            target.wrapper_projection(),
        )?
        .emit_resume_entry(surface.resume_tuple_ty())?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
        Ok(())
    }

    fn refactor_project_owner_step_to_wrapper(
        &mut self,
        abi: &RefactorAbiQuery<'ctx>,
        projection: &crate::effect_lowered::ir::LateLoweredSurfaceResumeWrapperProjection,
        owner_step: BasicValueEnum<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let owner_step_schema = projection.owner_step_schema();
        let wrapper_step_schema = projection.wrapper_step_schema();
        let owner_layout = abi.step_layout(owner_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor wrapper projection 缺少 owner step schema s{} layout",
                owner_step_schema.as_u32()
            ))
        })?;
        let wrapper_layout = abi.step_layout(wrapper_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor wrapper projection 缺少 wrapper step schema s{} layout",
                wrapper_step_schema.as_u32()
            ))
        })?;
        let tag = self.refactor_extract_step_tag(owner_layout, owner_step)?;
        let function = self.current_function()?;
        let complete_bb = self
            .context
            .append_basic_block(function, "wrapper_project_complete");
        let unmatched_bb = self
            .context
            .append_basic_block(function, "wrapper_project_unmatched");
        let cases = projection
            .outward_cases()
            .into_iter()
            .map(|case| {
                let owner_case_tag = case.owner_case_tag();
                let wrapper_case_tag = case.wrapper_case_tag();
                let owner_case = owner_layout
                    .case_layout(owner_case_tag)
                    .expect("projection case was validated by helper");
                (
                    self.context
                        .i32_type()
                        .const_int(owner_case.variant().tag_value() as u64, false),
                    self.context.append_basic_block(
                        function,
                        &format!("wrapper_project_case{}", wrapper_case_tag.as_u32()),
                    ),
                    owner_case_tag,
                    wrapper_case_tag,
                )
            })
            .collect::<Vec<_>>();
        let switch_cases = cases
            .iter()
            .map(|(tag, bb, _, _)| (*tag, *bb))
            .collect::<Vec<_>>();
        let complete_tag = self.context.i32_type().const_int(STEP_TAG_COMPLETE, false);
        let is_complete = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            tag,
            complete_tag,
            "wrapper_project_is_complete",
        )?;
        let dispatch_bb = self
            .context
            .append_basic_block(function, "wrapper_project_dispatch");
        self.builder
            .build_conditional_branch(is_complete, complete_bb, dispatch_bb)?;

        self.builder.position_at_end(dispatch_bb);
        self.builder
            .build_switch(tag, unmatched_bb, &switch_cases)?;

        self.builder.position_at_end(complete_bb);
        let payload = self.refactor_extract_step_payload(
            owner_layout,
            owner_step,
            owner_layout.complete_variant(),
            "wrapper_project_complete_payload",
        )?;
        let projected = self.refactor_build_step_complete(wrapper_layout, payload)?;
        self.builder.build_return(Some(&projected))?;

        for (_, bb, owner_case, wrapper_case) in cases {
            self.builder.position_at_end(bb);
            let owner_case_layout = owner_layout.case_layout(owner_case).ok_or_else(|| {
                frontend_error(format!(
                    "wrapper projection 缺少 owner case c{}",
                    owner_case.as_u32()
                ))
            })?;
            let wrapper_case_layout =
                wrapper_layout.case_layout(wrapper_case).ok_or_else(|| {
                    frontend_error(format!(
                        "wrapper projection 缺少 wrapper case c{}",
                        wrapper_case.as_u32()
                    ))
                })?;
            let (payload, continuation) = self.refactor_extract_step_case_parts(
                owner_layout,
                owner_step,
                owner_case_layout,
                "wrapper_project_case_payload",
            )?;
            let projected = self.refactor_build_step_case(
                wrapper_layout,
                wrapper_case_layout,
                payload,
                continuation,
            )?;
            self.builder.build_return(Some(&projected))?;
        }

        self.builder.position_at_end(unmatched_bb);
        self.builder.build_unreachable()?;
        Ok(())
    }

    fn current_function(&self) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        self.builder
            .get_insert_block()
            .and_then(|bb| bb.get_parent())
            .ok_or_else(|| {
                frontend_error(
                    "refactor body lowering 当前 builder 没有 active function".to_string(),
                )
            })
    }

    fn refactor_function(&self, symbol_name: &str) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        self.module.get_function(symbol_name).ok_or_else(|| {
            frontend_error(format!(
                "refactor body lowering 缺少已发布 function shell `{symbol_name}`"
            ))
        })
    }
}

struct RefactorCallableEmitter<'cg, 'a, 'ctx> {
    codegen: &'cg mut MainCodegen<'a, 'ctx>,
    source_types: &'a TypeStore,
    abi: &'cg RefactorAbiQuery<'ctx>,
    callable: &'a LateLoweredCallable,
    mir_fun: &'a mir::FunDecl,
    body: &'a mir::Body,
    function: FunctionValue<'ctx>,
    slots: Vec<MirLocalSlot<'ctx>>,
    used_locals: HashSet<LocalId>,
    frame_layout: &'cg RefactorFrameLayout<'ctx>,
    step_layout: &'cg RefactorStepLayout<'ctx>,
    frame_ptr: PointerValue<'ctx>,
    state_blocks: BTreeMap<StateId, BasicBlock<'ctx>>,
    return_projection:
        Option<&'cg crate::effect_lowered::ir::LateLoweredSurfaceResumeWrapperProjection>,
}

impl<'cg, 'a, 'ctx> RefactorCallableEmitter<'cg, 'a, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        codegen: &'cg mut MainCodegen<'a, 'ctx>,
        _program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        abi: &'cg RefactorAbiQuery<'ctx>,
        callable: &'a LateLoweredCallable,
        mir_fun: &'a mir::FunDecl,
        body: &'a mir::Body,
        function: FunctionValue<'ctx>,
        return_projection: Option<
            &'cg crate::effect_lowered::ir::LateLoweredSurfaceResumeWrapperProjection,
        >,
    ) -> Result<Self, LlvmEmitError> {
        let frame_layout = abi.frame_layout(callable.step_schema()).ok_or_else(|| {
            frontend_error(format!(
                "refactor body lowering 缺少 callable `{}` 的 frame layout s{}",
                callable.root_fqn(),
                callable.step_schema().as_u32()
            ))
        })?;
        let step_layout = abi.step_layout(callable.step_schema()).ok_or_else(|| {
            frontend_error(format!(
                "refactor body lowering 缺少 callable `{}` 的 step layout s{}",
                callable.root_fqn(),
                callable.step_schema().as_u32()
            ))
        })?;
        let slots = codegen.create_mir_local_slots(body, source_types)?;
        let used_locals = super::super::mir_body::collect_mir_local_uses(body);
        let frame_ptr = codegen.refactor_alloc_struct(frame_layout.llvm_ty(), "refactor_frame")?;
        let mut state_blocks = BTreeMap::new();
        for state in callable.state_graph().states() {
            state_blocks.insert(
                state.state_id(),
                codegen.context.append_basic_block(
                    function,
                    &format!("refactor.st{}", state.state_id().as_u32()),
                ),
            );
        }
        Ok(Self {
            codegen,
            source_types,
            abi,
            callable,
            mir_fun,
            body,
            function,
            slots,
            used_locals,
            frame_layout,
            step_layout,
            frame_ptr,
            state_blocks,
            return_projection,
        })
    }

    fn emit_direct(
        mut self,
        entry_layout: &RefactorCallableEntryLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        self.bind_direct_args(entry_layout)?;
        let entry_state = self.callable.state_graph().entry_state();
        self.branch_to_state(entry_state)?;
        self.emit_states()
    }

    fn emit_resume_method(
        self,
        _case_tag: CaseTag,
        resume_tuple_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        self.emit_resume_entry(resume_tuple_ty)
    }

    fn emit_resume_entry(mut self, resume_tuple_ty: TypeId) -> Result<(), LlvmEmitError> {
        let cont = self.function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "refactor resume method `{}` 缺少 continuation 参数",
                self.function.get_name().to_str().unwrap_or("<invalid>")
            ))
        })?;
        let cont_ptr = self.cast_gc_ref_to_continuation(cont.into_pointer_value())?;
        self.frame_ptr = self.load_frame_from_continuation(cont_ptr)?;
        self.restore_frame_slots_to_locals()?;
        let payload = if self.function.count_params() > 1 {
            Some(self.function.get_nth_param(1).ok_or_else(|| {
                frontend_error("refactor resume method 缺少 payload 参数".to_string())
            })?)
        } else {
            None
        };
        let resume_state_tag = self.load_continuation_resume_state(cont_ptr)?;
        let already = self.load_continuation_one_shot(cont_ptr)?;
        let first_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_first");
        let double_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_double");
        self.codegen.builder.build_conditional_branch(
            already,
            double_resume_bb,
            first_resume_bb,
        )?;

        self.codegen.builder.position_at_end(double_resume_bb);
        self.codegen.builder.build_unreachable()?;

        self.codegen.builder.position_at_end(first_resume_bb);
        self.store_continuation_one_shot(cont_ptr, true)?;
        let invalid_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_invalid_state");
        let mut bindings_by_state = BTreeMap::<StateId, LateLoweredResumePayloadBinding>::new();
        for binding in self.callable.frame_schema().resume_payload_bindings() {
            if !self.resume_payload_binding_accepts_tuple(binding, resume_tuple_ty)? {
                continue;
            }
            if let Some(existing) = bindings_by_state.get(&binding.resume_state()) {
                if existing.consumer_local() != binding.consumer_local()
                    || existing.consumer_frame_slot() != binding.consumer_frame_slot()
                {
                    return Err(frontend_error(format!(
                        "refactor resume entry st{} 的 resumed local/home contract 冲突：bd{} 与 bd{}",
                        binding.resume_state().as_u32(),
                        existing.boundary_id().as_u32(),
                        binding.boundary_id().as_u32()
                    )));
                }
                continue;
            }
            let _ = self.abi.resume_payload_binding_for_state(
                self.callable.step_schema(),
                binding.resume_state(),
            )?;
            bindings_by_state.insert(binding.resume_state(), *binding);
        }
        let mut cases = Vec::new();
        for binding in bindings_by_state.values().copied() {
            let bb = self.codegen.context.append_basic_block(
                self.function,
                &format!("resume_payload_st{}", binding.resume_state().as_u32()),
            );
            cases.push((
                self.codegen
                    .context
                    .i32_type()
                    .const_int(binding.resume_state().as_u32() as u64, false),
                bb,
                binding,
            ));
        }
        let switch_cases = cases
            .iter()
            .map(|(tag, bb, _)| (*tag, *bb))
            .collect::<Vec<_>>();
        self.codegen
            .builder
            .build_switch(resume_state_tag, invalid_bb, &switch_cases)?;
        for (_, bb, binding) in cases {
            self.codegen.builder.position_at_end(bb);
            self.inject_resume_payload(binding, resume_tuple_ty, payload)?;
            self.branch_to_state(binding.resume_state())?;
        }
        self.codegen.builder.position_at_end(invalid_bb);
        self.codegen.builder.build_unreachable()?;
        self.emit_states()
    }

    fn resume_payload_binding_accepts_tuple(
        &mut self,
        binding: &LateLoweredResumePayloadBinding,
        resume_tuple_ty: TypeId,
    ) -> Result<bool, LlvmEmitError> {
        let Some(resume_cg) = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, resume_tuple_ty)
        else {
            return Ok(false);
        };
        let slot = self.codegen.mir_local_slot(
            self.mir_fun.span,
            &self.slots,
            binding.consumer_local(),
        )?;
        Ok(slot.cg_ty == resume_cg)
    }

    fn emit_states(&mut self) -> Result<(), LlvmEmitError> {
        for state in self.callable.state_graph().states() {
            let bb = self.state_block(state.state_id())?;
            self.codegen.builder.position_at_end(bb);
            self.lower_state_source_slices(state)?;
            self.lower_state_terminator(state)?;
        }
        Ok(())
    }

    fn bind_direct_args(
        &mut self,
        entry_layout: &RefactorCallableEntryLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let args_layout = self
            .abi
            .source_value_layout(entry_layout.invoke_args_tuple_ty())?;
        let raw_arg = if entry_layout.param_count() == 0 {
            None
        } else {
            Some(self.function.get_nth_param(0).ok_or_else(|| {
                frontend_error(format!(
                    "refactor direct entry `{}` 缺少 args tuple 参数",
                    entry_layout.symbol_name()
                ))
            })?)
        };
        for (index, param) in self.mir_fun.params.iter().enumerate() {
            let slot = self
                .codegen
                .mir_local_slot(param.span, &self.slots, param.local)?;
            let param_cg = self
                .codegen
                .cg_ty_of_mir_type(self.source_types, param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor direct param type",
                    at: param.span.into(),
                })?;
            let value = match args_layout.kind() {
                RefactorSourceAbiLayoutKind::Scalar
                    if index == 0 && !args_layout.abi().is_elided() =>
                {
                    let raw = raw_arg.ok_or_else(|| {
                        frontend_error("refactor direct scalar arg ABI 缺少 raw 参数".to_string())
                    })?;
                    self.codegen
                        .cg_value_from_loaded(param.span, param_cg, raw)?
                }
                RefactorSourceAbiLayoutKind::Tuple => {
                    if let Some(field) = args_layout.field(index) {
                        if field.is_elided() {
                            self.codegen.default_value(param.span, param_cg)?
                        } else {
                            let tuple = raw_arg.ok_or_else(|| {
                                frontend_error(
                                    "refactor direct args tuple ABI 缺少 raw 参数".to_string(),
                                )
                            })?;
                            let struct_value = tuple.into_struct_value();
                            let raw = self.codegen.builder.build_extract_value(
                                struct_value,
                                field
                                    .abi_field_index()
                                    .expect("non-elided field has ABI index"),
                                "refactor_arg_field",
                            )?;
                            self.codegen
                                .cg_value_from_loaded(param.span, param_cg, raw)?
                        }
                    } else {
                        self.codegen.default_value(param.span, param_cg)?
                    }
                }
                RefactorSourceAbiLayoutKind::Scalar => {
                    self.codegen.default_value(param.span, param_cg)?
                }
            };
            let value = self.codegen.coerce_value(param.span, value, slot.cg_ty)?;
            let _ = self
                .codegen
                .store_local_value(param.span, slot.ptr, slot.cg_ty, value)?;
        }
        Ok(())
    }

    fn lower_state_source_slices(&mut self, state: &LateLoweredState) -> Result<(), LlvmEmitError> {
        let skipped = self.skipped_statement_indices_for_state(state)?;
        for slice in state.source_slices() {
            let block = self
                .body
                .blocks
                .get(slice.block_id().as_u32() as usize)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor source slice block",
                    at: self.mir_fun.span.into(),
                })?;
            for stmt_index in slice.start_statement_index()..slice.end_statement_index() {
                if skipped.contains(&(slice.block_id(), stmt_index)) {
                    continue;
                }
                let stmt = block.stmts.get(stmt_index as usize).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor source slice statement",
                        at: self.mir_fun.span.into(),
                    },
                )?;
                if self.statement_is_published_resume_payload_injection(state.state_id(), stmt)? {
                    continue;
                }
                if let mir::StatementKind::Assign {
                    value: mir::Rvalue::TopLevelRef(mir::TopLevelRef { fqn }),
                    ..
                } = &stmt.kind
                    && !self.codegen.object_inits.contains_key(fqn)
                    && !self.codegen.top_level_consts.contains_key(fqn)
                    && !self.codegen.top_level_immutable_values.contains_key(fqn)
                    && !self.codegen.top_level_vars.contains_key(fqn)
                {
                    continue;
                }
                if matches!(
                    &stmt.kind,
                    mir::StatementKind::Assign {
                        value: mir::Rvalue::Call {
                            kind: mir::CallKind::Resume { .. }
                                | mir::CallKind::Virtual { .. }
                                | mir::CallKind::Interface { .. },
                            ..
                        },
                        ..
                    }
                ) {
                    continue;
                }
                if self.try_lower_refactor_specialized_direct_call(stmt)? {
                    continue;
                }
                self.codegen.codegen_mir_statement(
                    stmt,
                    self.body,
                    self.source_types,
                    &self.slots,
                    &self.used_locals,
                )?;
            }
        }
        Ok(())
    }

    fn lower_state_terminator(&mut self, state: &LateLoweredState) -> Result<(), LlvmEmitError> {
        if self.current_block_is_terminated() {
            return Ok(());
        }
        match state.terminator() {
            LateLoweredStateTerminator::Goto { target } => self.branch_to_state(*target),
            LateLoweredStateTerminator::Branch {
                cond_local,
                then_state,
                else_state,
            } => {
                let slot =
                    self.codegen
                        .mir_local_slot(self.mir_fun.span, &self.slots, *cond_local)?;
                let cond = self
                    .codegen
                    .load_mir_local(self.mir_fun.span, slot)?
                    .as_bool()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor state branch condition",
                        at: self.mir_fun.span.into(),
                    })?;
                self.codegen.builder.build_conditional_branch(
                    cond,
                    self.state_block(*then_state)?,
                    self.state_block(*else_state)?,
                )?;
                Ok(())
            }
            LateLoweredStateTerminator::Return {
                payload_source,
                complete_state: _,
            } => {
                let binding = self.abi.completion_payload_binding_for_state(
                    self.callable.step_schema(),
                    state.state_id(),
                )?;
                let _ = self.abi.completion_payload_binding_layout(
                    self.callable.step_schema(),
                    binding.binding(),
                )?;
                let payload = self.lower_completion_payload(payload_source)?;
                let step = self
                    .codegen
                    .refactor_build_step_complete(self.step_layout, payload)?;
                self.return_step(step)
            }
            LateLoweredStateTerminator::Suspend { boundary_ids, .. } => {
                self.lower_suspend(state, boundary_ids)
            }
            LateLoweredStateTerminator::HandleDispatch { body_state, .. } => {
                self.branch_to_state(*body_state)
            }
            LateLoweredStateTerminator::LocalRuntimeError {
                terminal_action, ..
            } => {
                let runtime = terminal_action.runtime_entry();
                let callee = self
                    .codegen
                    .module
                    .get_function(runtime.symbol_name())
                    .unwrap_or_else(|| self.codegen.declare_runtime_error_fatal());
                let null_payload = self.codegen.llvm_gc_i8_ptr_type().const_null();
                self.codegen.builder.build_call(
                    callee,
                    &[null_payload.into()],
                    "refactor_runtime_error",
                )?;
                self.codegen.builder.build_unreachable()?;
                Ok(())
            }
            LateLoweredStateTerminator::Unreachable
            | LateLoweredStateTerminator::ResumeUnwind
            | LateLoweredStateTerminator::Abandon => {
                self.codegen.builder.build_unreachable()?;
                Ok(())
            }
        }
    }

    fn try_lower_refactor_specialized_direct_call(
        &mut self,
        stmt: &mir::Statement,
    ) -> Result<bool, LlvmEmitError> {
        let mir::StatementKind::Assign {
            target,
            value:
                mir::Rvalue::Call {
                    kind: mir::CallKind::Direct { callee_fqn },
                    args,
                    ..
                },
        } = &stmt.kind
        else {
            return Ok(false);
        };
        if self.codegen.fun_index.contains_key(callee_fqn) {
            return Ok(false);
        }
        if callee_fqn == "scoop.core.println" || callee_fqn == "scoop.core.__scoop_println_string" {
            return self.lower_refactor_print_statement(stmt.span, *target, args, "scoop_println");
        }
        if callee_fqn == "scoop.core.print" || callee_fqn == "scoop.core.__scoop_print_string" {
            return self.lower_refactor_print_statement(stmt.span, *target, args, "scoop_print");
        }
        let Some(specialized) = self.specialized_direct_callee(callee_fqn, args) else {
            return Ok(false);
        };
        let value = self.codegen.codegen_mir_direct_call(
            stmt.span,
            specialized,
            args,
            self.body,
            &self.slots,
        )?;
        let slot = self
            .codegen
            .mir_local_slot(stmt.span, &self.slots, *target)?;
        let value = self.codegen.coerce_value(stmt.span, value, slot.cg_ty)?;
        let _ = self
            .codegen
            .store_local_value(stmt.span, slot.ptr, slot.cg_ty, value)?;
        Ok(true)
    }

    fn lower_refactor_print_statement(
        &mut self,
        span: crate::span::Span,
        target: LocalId,
        args: &[mir::CallArg],
        runtime_name: &str,
    ) -> Result<bool, LlvmEmitError> {
        if args.len() != 1 {
            return Ok(false);
        }
        let value = self
            .codegen
            .codegen_mir_operand(span, &args[0].value, &self.slots)?;
        let str_ptr = match value.ty {
            CgTy::String => {
                let value = self.codegen.coerce_value(span, value, CgTy::String)?;
                let Some(BasicValueEnum::PointerValue(ptr)) = value.value else {
                    return Err(frontend_error(
                        "refactor println String argument 缺少 pointer value".to_string(),
                    ));
                };
                ptr
            }
            CgTy::Int(int_ty) => {
                let (raw, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor println int value",
                    at: span.into(),
                })?;
                let int64_ty = super::super::types::IntTy {
                    bits: 64,
                    signed: int_ty.signed,
                };
                let int64 = self.codegen.cast_int(raw, int_ty, int64_ty)?;
                self.codegen
                    .codegen_int_to_string(span, int64, int64_ty.signed)?
            }
            _ => return Ok(false),
        };
        let rt_fun = self.codegen.declare_runtime_print_like(runtime_name);
        let _ = self.codegen.build_call_preserving_gc_local_roots(
            span,
            rt_fun,
            &[str_ptr.into()],
            "refactor_println",
        )?;
        let slot = self.codegen.mir_local_slot(span, &self.slots, target)?;
        let unit = self
            .codegen
            .coerce_value(span, CgValue::unit(), slot.cg_ty)?;
        let _ = self
            .codegen
            .store_local_value(span, slot.ptr, slot.cg_ty, unit)?;
        Ok(true)
    }

    fn specialized_direct_callee(
        &self,
        callee_fqn: &str,
        args: &[mir::CallArg],
    ) -> Option<&'static str> {
        if callee_fqn != "scoop.core.println" || args.len() != 1 {
            return None;
        }
        match self
            .codegen
            .mir_operand_cg_ty(self.body, self.source_types, &args[0].value)
        {
            Some(CgTy::Int(_)) => Some("scoop.core.println::<Int>"),
            Some(CgTy::String) => Some("scoop.core.println::<String>"),
            _ => None,
        }
    }

    fn lower_suspend(
        &mut self,
        state: &LateLoweredState,
        boundary_ids: &[BoundaryId],
    ) -> Result<(), LlvmEmitError> {
        let boundary = boundary_ids
            .iter()
            .filter_map(|id| self.callable.boundary_map().boundary(*id))
            .find(|boundary| {
                !matches!(
                    boundary.lowering(),
                    Some(LateLoweredBoundaryLowering::RuntimeError(_))
                )
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor suspend state st{} 缺少可 lower 的 primary boundary",
                    state.state_id().as_u32()
                ))
            })?;
        match boundary.lowering().ok_or_else(|| {
            frontend_error(format!(
                "refactor boundary bd{} 缺少 published lowering",
                boundary.boundary_id().as_u32()
            ))
        })? {
            LateLoweredBoundaryLowering::Call(lowering) => {
                let source = boundary_site(boundary, "Call")?;
                let _ = self.abi.call_boundary_operand_layout(
                    self.callable.step_schema(),
                    source,
                    lowering.operand_contract(),
                )?;
                let args_payload = self.pack_sources(
                    lowering.facts().invoke_args_tuple_ty(),
                    lowering.operand_contract().arg_sources(),
                    "refactor_call_args",
                )?;
                let target = self.abi.call_target_layout(
                    self.callable.step_schema(),
                    source,
                    lowering.facts(),
                )?;
                let (callee_fun, callee_step_schema, callee_args_abi) = match target {
                    RefactorCallTargetQuery::KnownInstance(layout) => (
                        self.codegen
                            .refactor_function(layout.direct_entry().symbol_name())?,
                        layout.step_schema(),
                        *layout.direct_entry().args_abi(),
                    ),
                    RefactorCallTargetQuery::DynamicInvoke(layout) => {
                        return Err(frontend_error(format!(
                            "refactor body lowering 尚未支持 dynamic invoke body call site {}，但 ABI query 已发布 contract",
                            layout.site_id().as_u32()
                        )));
                    }
                };
                let mut args = Vec::new();
                if !callee_args_abi.is_elided() {
                    args.push(
                        args_payload
                            .ok_or_else(|| {
                                frontend_error(format!(
                                    "refactor call site {} 需要 non-elided args payload",
                                    source.as_u32()
                                ))
                            })?
                            .into(),
                    );
                }
                let call =
                    self.codegen
                        .builder
                        .build_call(callee_fun, &args, "refactor_call_step")?;
                let step = call.try_as_basic_value().basic().ok_or_else(|| {
                    frontend_error("refactor call boundary callee 未返回 Step_F".to_string())
                })?;
                self.dispatch_boundary_step(boundary, callee_step_schema, step, lowering.dispatch())
            }
            LateLoweredBoundaryLowering::Perform(lowering) => {
                let source = boundary_site(boundary, "Perform")?;
                let _ = self.abi.perform_boundary_operand_layout(
                    self.callable.step_schema(),
                    source,
                    lowering.operand_contract(),
                )?;
                let payload = self.pack_sources(
                    lowering.emitted_step().payload_tuple_ty(),
                    lowering.operand_contract().payload_sources(),
                    "refactor_perform_payload",
                )?;
                self.emit_or_consume_outward_case(
                    boundary,
                    lowering.emitted_step().case_tag(),
                    payload,
                    lowering.emitted_step().payload_tuple_ty(),
                )
            }
            LateLoweredBoundaryLowering::Resume(lowering) => {
                let source = boundary_site(boundary, "Resume")?;
                let _ = self.abi.resume_boundary_operand_layout(
                    self.callable.step_schema(),
                    source,
                    lowering.operand_contract(),
                )?;
                let surface = self
                    .abi
                    .surface_resume_layout(lowering.facts().continuation_schema())
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor resume site {} 缺少 continuation schema k{} surface ABI",
                            source.as_u32(),
                            lowering.facts().continuation_schema().as_u32()
                        ))
                    })?;
                let cont_value =
                    self.lower_operand_source(lowering.operand_contract().continuation_source())?;
                let cont_ptr = cont_value.value.ok_or_else(|| {
                    frontend_error(format!(
                        "refactor resume site {} continuation source 被 elide",
                        source.as_u32()
                    ))
                })?;
                let BasicValueEnum::PointerValue(cont_ptr) = cont_ptr else {
                    return Err(frontend_error(format!(
                        "refactor resume site {} continuation source 不是 pointer",
                        source.as_u32()
                    )));
                };
                let args_payload = self.pack_sources(
                    surface.resume_tuple_ty(),
                    lowering.operand_contract().arg_sources(),
                    "refactor_resume_args",
                )?;
                let callee = self.codegen.refactor_function(surface.symbol_name())?;
                let mut args = vec![cont_ptr.into()];
                if !surface.resume_payload_abi().is_elided() {
                    args.push(
                        args_payload
                            .ok_or_else(|| {
                                frontend_error(format!(
                                    "refactor resume site {} 需要 non-elided payload",
                                    source.as_u32()
                                ))
                            })?
                            .into(),
                    );
                }
                let call =
                    self.codegen
                        .builder
                        .build_call(callee, &args, "refactor_resume_step")?;
                let step = call.try_as_basic_value().basic().ok_or_else(|| {
                    frontend_error("refactor resume boundary callee 未返回 Step_F".to_string())
                })?;
                self.dispatch_boundary_step(
                    boundary,
                    lowering.facts().out_step_schema(),
                    step,
                    lowering.dispatch(),
                )
            }
            LateLoweredBoundaryLowering::RuntimeError(_)
            | LateLoweredBoundaryLowering::Handle(_) => Err(frontend_error(format!(
                "refactor suspend state st{} primary boundary bd{} 不是 Call/Perform/Resume",
                state.state_id().as_u32(),
                boundary.boundary_id().as_u32()
            ))),
        }
    }

    fn dispatch_boundary_step(
        &mut self,
        boundary: &LateLoweredBoundary,
        input_step_schema: StepSchemaId,
        step: BasicValueEnum<'ctx>,
        dispatch: &crate::effect_lowered::ir::LateLoweredStepDispatchPlan,
    ) -> Result<(), LlvmEmitError> {
        let input_layout = self.abi.step_layout(input_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor boundary dispatch 缺少 input step schema s{} layout",
                input_step_schema.as_u32()
            ))
        })?;
        let function = self.function;
        let complete_bb = self.codegen.context.append_basic_block(
            function,
            &format!("bd{}_complete", boundary.boundary_id().as_u32()),
        );
        let unmatched_bb = self.codegen.context.append_basic_block(
            function,
            &format!("bd{}_unmatched", boundary.boundary_id().as_u32()),
        );
        let mut cases = Vec::new();
        for case in dispatch.outward_cases() {
            if let Some(case_layout) = input_layout.case_layout(case.input_case_tag()) {
                let bb = self.codegen.context.append_basic_block(
                    function,
                    &format!(
                        "bd{}_case{}",
                        boundary.boundary_id().as_u32(),
                        case.input_case_tag().as_u32()
                    ),
                );
                cases.push((
                    self.codegen
                        .context
                        .i32_type()
                        .const_int(case_layout.variant().tag_value() as u64, false),
                    bb,
                    case.input_case_tag(),
                    case.emission().case_tag(),
                    case.emission().payload_tuple_ty(),
                ));
            }
        }
        let tag = self.codegen.refactor_extract_step_tag(input_layout, step)?;
        let dispatch_bb = self.codegen.context.append_basic_block(
            function,
            &format!("bd{}_dispatch", boundary.boundary_id().as_u32()),
        );
        let is_complete = self.codegen.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            tag,
            self.codegen
                .context
                .i32_type()
                .const_int(STEP_TAG_COMPLETE, false),
            "refactor_step_is_complete",
        )?;
        self.codegen
            .builder
            .build_conditional_branch(is_complete, complete_bb, dispatch_bb)?;
        self.codegen.builder.position_at_end(dispatch_bb);
        let switch_cases = cases
            .iter()
            .map(|(tag, bb, _, _, _)| (*tag, *bb))
            .collect::<Vec<_>>();
        self.codegen
            .builder
            .build_switch(tag, unmatched_bb, &switch_cases)?;

        self.codegen.builder.position_at_end(complete_bb);
        let payload = self.codegen.refactor_extract_step_payload(
            input_layout,
            step,
            input_layout.complete_variant(),
            "refactor_boundary_complete_payload",
        )?;
        self.store_boundary_result(boundary.boundary_id(), payload, boundary.resume_state())?;
        self.branch_to_state(boundary.resume_state())?;

        for (_, bb, input_case, output_case, payload_ty) in cases {
            self.codegen.builder.position_at_end(bb);
            let case_layout = input_layout.case_layout(input_case).ok_or_else(|| {
                frontend_error(format!(
                    "refactor boundary dispatch 缺少 case c{}",
                    input_case.as_u32()
                ))
            })?;
            let (payload, _callee_cont) = self.codegen.refactor_extract_step_case_parts(
                input_layout,
                step,
                case_layout,
                "refactor_boundary_case_payload",
            )?;
            self.emit_or_consume_outward_case(boundary, output_case, payload, payload_ty)?;
        }

        self.codegen.builder.position_at_end(unmatched_bb);
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    fn emit_or_consume_outward_case(
        &mut self,
        boundary: &LateLoweredBoundary,
        case_tag: CaseTag,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        self.sync_frame_slots_from_locals()?;
        if let Some((arm_state, continuation_local)) =
            self.local_handle_consumption(boundary.boundary_id(), case_tag)
        {
            if let Some(local) = continuation_local {
                let continuation = self.create_continuation_object(boundary.resume_state())?;
                self.store_gc_ref_to_local(local, continuation)?;
            }
            self.store_case_payload_to_arm_binders(
                boundary.boundary_id(),
                case_tag,
                payload,
                payload_ty,
            )?;
            return self.branch_to_state(arm_state);
        }
        let continuation = self.create_continuation_object(boundary.resume_state())?;
        let out_layout = self.step_layout.case_layout(case_tag).ok_or_else(|| {
            frontend_error(format!(
                "refactor callable `{}` step schema s{} 缺少 outward case c{}",
                self.callable.root_fqn(),
                self.callable.step_schema().as_u32(),
                case_tag.as_u32()
            ))
        })?;
        let step = self.codegen.refactor_build_step_case(
            self.step_layout,
            out_layout,
            payload,
            continuation,
        )?;
        self.return_step(step)
    }

    fn return_step(&mut self, step: BasicValueEnum<'ctx>) -> Result<(), LlvmEmitError> {
        if let Some(projection) = self.return_projection {
            self.codegen
                .refactor_project_owner_step_to_wrapper(self.abi, projection, step)
        } else {
            self.codegen.builder.build_return(Some(&step))?;
            Ok(())
        }
    }

    fn local_handle_consumption(
        &self,
        boundary_id: BoundaryId,
        case_tag: CaseTag,
    ) -> Option<(StateId, Option<LocalId>)> {
        for state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch { contract, .. } = state.terminator()
            else {
                continue;
            };
            let routing = contract.boundary_routing(boundary_id)?;
            let case = routing.case_routing(case_tag)?;
            if let LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                arm_state,
                arm_ordinal,
                ..
            } = case.action()
            {
                let continuation_local = contract
                    .handled_arms()
                    .iter()
                    .find(|arm| arm.arm_ordinal() == arm_ordinal)
                    .and_then(|arm| arm.continuation_binder())
                    .map(|binder| binder.local());
                return Some((arm_state, continuation_local));
            }
        }
        None
    }

    fn store_case_payload_to_arm_binders(
        &mut self,
        boundary_id: BoundaryId,
        case_tag: CaseTag,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        for state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch { contract, .. } = state.terminator()
            else {
                continue;
            };
            let Some(routing) = contract.boundary_routing(boundary_id) else {
                continue;
            };
            let Some(case) = routing.case_routing(case_tag) else {
                continue;
            };
            let LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm { arm_ordinal, .. } =
                case.action()
            else {
                continue;
            };
            let Some(arm) = contract
                .handled_arms()
                .iter()
                .find(|arm| arm.arm_ordinal() == arm_ordinal)
            else {
                continue;
            };
            for binder in arm.payload_binders() {
                let value = self.unpack_payload_field(payload, payload_ty, binder.ordinal())?;
                let slot =
                    self.codegen
                        .mir_local_slot(self.mir_fun.span, &self.slots, binder.local())?;
                if let Some(raw) = value {
                    let cg =
                        self.codegen
                            .cg_value_from_loaded(self.mir_fun.span, slot.cg_ty, raw)?;
                    let _ = self.codegen.store_local_value(
                        self.mir_fun.span,
                        slot.ptr,
                        slot.cg_ty,
                        cg,
                    )?;
                }
            }
        }
        Ok(())
    }

    fn store_boundary_result(
        &mut self,
        boundary_id: BoundaryId,
        payload: Option<BasicValueEnum<'ctx>>,
        resume_state: StateId,
    ) -> Result<(), LlvmEmitError> {
        let binding = self
            .callable
            .frame_schema()
            .resume_payload_binding(boundary_id)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor boundary bd{} 缺少 resumed local/home binding",
                    boundary_id.as_u32()
                ))
            })?;
        if binding.resume_state() != resume_state {
            return Err(frontend_error(format!(
                "refactor boundary bd{} resume state 漂移：boundary=st{} binding=st{}",
                boundary_id.as_u32(),
                resume_state.as_u32(),
                binding.resume_state().as_u32()
            )));
        }
        let _ = self
            .abi
            .resume_payload_binding_layout(self.callable.step_schema(), binding)?;
        self.store_payload_to_binding(binding, payload)
    }

    fn inject_resume_payload(
        &mut self,
        binding: LateLoweredResumePayloadBinding,
        resume_tuple_ty: TypeId,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        self.store_payload_to_binding(&binding, payload)?;
        let _ = resume_tuple_ty;
        Ok(())
    }

    fn store_payload_to_binding(
        &mut self,
        binding: &LateLoweredResumePayloadBinding,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        let slot = self.codegen.mir_local_slot(
            self.mir_fun.span,
            &self.slots,
            binding.consumer_local(),
        )?;
        if let Some(raw) = payload {
            let value = self
                .codegen
                .cg_value_from_loaded(self.mir_fun.span, slot.cg_ty, raw)?;
            let _ =
                self.codegen
                    .store_local_value(self.mir_fun.span, slot.ptr, slot.cg_ty, value)?;
        }
        if let Some(frame_slot) = binding.consumer_frame_slot() {
            self.store_local_to_frame_slot(binding.consumer_local(), frame_slot)?;
        }
        Ok(())
    }

    fn lower_completion_payload(
        &mut self,
        payload_source: &LateLoweredCompletionPayloadSource,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        match payload_source {
            LateLoweredCompletionPayloadSource::Unit { .. } => Ok(None),
            LateLoweredCompletionPayloadSource::Operand(source) => {
                let value = self.lower_operand_source(source)?;
                Ok(value.value)
            }
        }
    }

    fn lower_operand_source(
        &mut self,
        source: &LateLoweredOperandSource,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let span = source.span().unwrap_or(self.mir_fun.span);
        let expected = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, source.source_ty())
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor operand source type",
                at: span.into(),
            })?;
        let operand = match source.value() {
            LateLoweredOperandValueSource::Local(local) => mir::Operand::Local(*local),
            LateLoweredOperandValueSource::Const(value) => mir::Operand::Const(value.clone()),
        };
        let value = self.codegen.codegen_mir_operand_expected(
            span,
            &operand,
            &self.slots,
            Some(expected),
        )?;
        self.codegen.coerce_value(span, value, expected)
    }

    fn pack_sources(
        &mut self,
        source_ty: TypeId,
        sources: &[LateLoweredOperandSource],
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(source_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        match layout.kind() {
            RefactorSourceAbiLayoutKind::Scalar => {
                let source = sources.first().ok_or_else(|| {
                    frontend_error(format!("refactor ABI scalar payload `{name}` 缺少 source"))
                })?;
                Ok(self.lower_operand_source(source)?.value)
            }
            RefactorSourceAbiLayoutKind::Tuple => {
                let BasicTypeEnum::StructType(struct_ty) = layout.abi().llvm_ty() else {
                    return Err(frontend_error(format!(
                        "refactor ABI tuple payload `{name}` layout 不是 struct"
                    )));
                };
                let mut aggregate = struct_ty.get_undef();
                for (index, source) in sources.iter().enumerate() {
                    let Some(field) = layout.field(index) else {
                        return Err(frontend_error(format!(
                            "refactor ABI tuple payload `{name}` source index {index} 超出 layout 字段"
                        )));
                    };
                    if field.is_elided() {
                        continue;
                    }
                    let raw = self.lower_operand_source(source)?.value.ok_or_else(|| {
                        frontend_error(format!(
                            "refactor ABI tuple payload `{name}` source index {index} 被 elide 但 field 需要值"
                        ))
                    })?;
                    aggregate = self
                        .codegen
                        .builder
                        .build_insert_value(
                            aggregate,
                            raw,
                            field
                                .abi_field_index()
                                .expect("non-elided field has ABI index"),
                            &format!("{name}_field{index}"),
                        )?
                        .into_struct_value();
                }
                Ok(Some(aggregate.into()))
            }
        }
    }

    fn create_continuation_object(
        &mut self,
        resume_state: StateId,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let cont_layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor callable `{}` 缺少 continuation object ko{} layout",
                    self.callable.root_fqn(),
                    self.callable.continuation_object().as_u32()
                ))
            })?;
        let cont_ptr = self
            .codegen
            .refactor_alloc_struct(cont_layout.llvm_ty(), "refactor_cont")?;
        let frame_gc = self.codegen.refactor_cast_ptr(
            self.frame_ptr,
            self.codegen.llvm_gc_i8_ptr_type(),
            "refactor_frame_gc",
        )?;
        let frame_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_CAPTURED_FRAME,
            "refactor_cont_frame_gep",
        )?;
        self.codegen.builder.build_store(frame_gep, frame_gc)?;
        let state_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_RESUME_STATE,
            "refactor_cont_state_gep",
        )?;
        self.codegen.builder.build_store(
            state_gep,
            self.codegen
                .context
                .i32_type()
                .const_int(resume_state.as_u32() as u64, false),
        )?;
        let one_shot_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_ONE_SHOT,
            "refactor_cont_one_shot_gep",
        )?;
        self.codegen
            .builder
            .build_store(one_shot_gep, self.codegen.context.bool_type().const_zero())?;
        self.codegen.refactor_cast_ptr(
            cont_ptr,
            self.codegen.llvm_gc_i8_ptr_type(),
            "refactor_cont_gc",
        )
    }

    fn cast_gc_ref_to_continuation(
        &mut self,
        ptr: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let target_ty = self.codegen.context.ptr_type(AddressSpace::default());
        self.codegen
            .refactor_cast_ptr(ptr, target_ty, "refactor_cont_typed")
    }

    fn load_frame_from_continuation(
        &mut self,
        cont_ptr: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .ok_or_else(|| {
                frontend_error("refactor resume 缺少 continuation layout".to_string())
            })?;
        let gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_CAPTURED_FRAME,
            "refactor_load_frame_gep",
        )?;
        let raw = self
            .codegen
            .builder
            .build_load(
                self.codegen.llvm_gc_i8_ptr_type(),
                gep,
                "refactor_load_frame_gc",
            )?
            .into_pointer_value();
        self.codegen.refactor_cast_ptr(
            raw,
            self.codegen.context.ptr_type(AddressSpace::default()),
            "refactor_frame_typed",
        )
    }

    fn load_continuation_resume_state(
        &mut self,
        cont_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .unwrap();
        let gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_RESUME_STATE,
            "refactor_resume_state_gep",
        )?;
        Ok(self
            .codegen
            .builder
            .build_load(
                self.codegen.context.i32_type(),
                gep,
                "refactor_resume_state",
            )?
            .into_int_value())
    }

    fn load_continuation_one_shot(
        &mut self,
        cont_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .unwrap();
        let gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_ONE_SHOT,
            "refactor_one_shot_gep",
        )?;
        Ok(self
            .codegen
            .builder
            .build_load(self.codegen.context.bool_type(), gep, "refactor_one_shot")?
            .into_int_value())
    }

    fn store_continuation_one_shot(
        &mut self,
        cont_ptr: PointerValue<'ctx>,
        value: bool,
    ) -> Result<(), LlvmEmitError> {
        let layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .unwrap();
        let gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_ONE_SHOT,
            "refactor_store_one_shot_gep",
        )?;
        self.codegen.builder.build_store(
            gep,
            self.codegen
                .context
                .bool_type()
                .const_int(value as u64, false),
        )?;
        Ok(())
    }

    fn sync_frame_slots_from_locals(&mut self) -> Result<(), LlvmEmitError> {
        for slot in self.callable.frame_schema().slots() {
            if let Some(local) = frame_slot_local(slot.kind()) {
                self.store_local_to_frame_slot(local, slot.slot_id())?;
            }
        }
        Ok(())
    }

    fn restore_frame_slots_to_locals(&mut self) -> Result<(), LlvmEmitError> {
        for slot in self.callable.frame_schema().slots() {
            let Some(local) = frame_slot_local(slot.kind()) else {
                continue;
            };
            let local_slot = self
                .codegen
                .mir_local_slot(self.mir_fun.span, &self.slots, local)?;
            if local_slot.cg_ty == CgTy::Unit || local_slot.cg_ty == CgTy::Never {
                continue;
            }
            let field_index = self
                .frame_layout
                .field_index_for_slot(slot.slot_id())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor frame layout 缺少 slot{} field index",
                        slot.slot_id().as_u32()
                    ))
                })?;
            let field_ptr = self.codegen.builder.build_struct_gep(
                self.frame_layout.llvm_ty(),
                self.frame_ptr,
                field_index,
                "refactor_frame_slot_load_gep",
            )?;
            let loaded = self.codegen.builder.build_load(
                self.codegen
                    .llvm_basic_type_of(self.mir_fun.span, local_slot.cg_ty)?,
                field_ptr,
                "refactor_frame_slot_load",
            )?;
            let value =
                self.codegen
                    .cg_value_from_loaded(self.mir_fun.span, local_slot.cg_ty, loaded)?;
            let _ = self.codegen.store_local_value(
                self.mir_fun.span,
                local_slot.ptr,
                local_slot.cg_ty,
                value,
            )?;
        }
        Ok(())
    }

    fn store_local_to_frame_slot(
        &mut self,
        local: LocalId,
        frame_slot: crate::effect_lowered::ir::FrameSlotId,
    ) -> Result<(), LlvmEmitError> {
        let local_slot = self
            .codegen
            .mir_local_slot(self.mir_fun.span, &self.slots, local)?;
        if local_slot.cg_ty == CgTy::Unit || local_slot.cg_ty == CgTy::Never {
            return Ok(());
        }
        let field_index = self
            .frame_layout
            .field_index_for_slot(frame_slot)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor frame layout 缺少 slot{} field index",
                    frame_slot.as_u32()
                ))
            })?;
        let field_ptr = self.codegen.builder.build_struct_gep(
            self.frame_layout.llvm_ty(),
            self.frame_ptr,
            field_index,
            "refactor_frame_slot_store_gep",
        )?;
        let value = self.codegen.load_mir_local(self.mir_fun.span, local_slot)?;
        if let Some(raw) = value.value {
            self.codegen.builder.build_store(field_ptr, raw)?;
        }
        Ok(())
    }

    fn store_gc_ref_to_local(
        &mut self,
        local: LocalId,
        value: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let slot = self
            .codegen
            .mir_local_slot(self.mir_fun.span, &self.slots, local)?;
        let cg = CgValue {
            ty: slot.cg_ty,
            value: Some(value.into()),
        };
        let _ = self
            .codegen
            .store_local_value(self.mir_fun.span, slot.ptr, slot.cg_ty, cg)?;
        Ok(())
    }

    fn unpack_payload_field(
        &mut self,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
        ordinal: u32,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(payload_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        match layout.kind() {
            RefactorSourceAbiLayoutKind::Scalar => Ok(payload),
            RefactorSourceAbiLayoutKind::Tuple => {
                let Some(field) = layout.field(ordinal as usize) else {
                    return Err(frontend_error(format!(
                        "refactor payload tuple t{} 缺少 ordinal {}",
                        payload_ty.as_u32(),
                        ordinal
                    )));
                };
                if field.is_elided() {
                    return Ok(None);
                }
                let Some(BasicValueEnum::StructValue(tuple)) = payload else {
                    return Err(frontend_error(format!(
                        "refactor payload tuple t{} 缺少 struct payload",
                        payload_ty.as_u32()
                    )));
                };
                Ok(Some(
                    self.codegen.builder.build_extract_value(
                        tuple,
                        field
                            .abi_field_index()
                            .expect("non-elided field has ABI index"),
                        "refactor_payload_field",
                    )?,
                ))
            }
        }
    }

    fn skipped_statement_indices_for_state(
        &self,
        state: &LateLoweredState,
    ) -> Result<HashSet<(mir::BasicBlockId, u32)>, LlvmEmitError> {
        let mut skipped = HashSet::new();
        if let LateLoweredStateTerminator::Suspend { boundary_ids, .. } = state.terminator() {
            for boundary_id in boundary_ids {
                let Some(boundary) = self.callable.boundary_map().boundary(*boundary_id) else {
                    continue;
                };
                let Some(consumption) = boundary_consumption(boundary) else {
                    continue;
                };
                if let LateLoweredBoundarySourceConsumption::Statement {
                    source_slice,
                    statement_index,
                    ..
                } = consumption
                {
                    skipped.insert((source_slice.block_id(), statement_index));
                }
            }
        }
        Ok(skipped)
    }

    fn statement_is_published_resume_payload_injection(
        &self,
        state_id: StateId,
        stmt: &mir::Statement,
    ) -> Result<bool, LlvmEmitError> {
        let Some(binding) = self
            .callable
            .frame_schema()
            .resume_payload_bindings()
            .iter()
            .find(|binding| binding.resume_state() == state_id)
        else {
            return Ok(false);
        };
        Ok(matches!(
            &stmt.kind,
            mir::StatementKind::Assign {
                target,
                value: mir::Rvalue::PerformResult { .. },
            } if *target == binding.consumer_local()
        ))
    }

    fn branch_to_state(&mut self, state_id: StateId) -> Result<(), LlvmEmitError> {
        if self.current_block_is_terminated() {
            return Ok(());
        }
        let target = self.state_block(state_id)?;
        self.codegen.builder.build_unconditional_branch(target)?;
        Ok(())
    }

    fn state_block(&self, state_id: StateId) -> Result<BasicBlock<'ctx>, LlvmEmitError> {
        self.state_blocks.get(&state_id).copied().ok_or_else(|| {
            frontend_error(format!(
                "refactor state graph 缺少 StateId st{} 的 LLVM block",
                state_id.as_u32()
            ))
        })
    }

    fn current_block_is_terminated(&self) -> bool {
        self.codegen
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    fn refactor_alloc_struct(
        &mut self,
        struct_ty: StructType<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let malloc = self.declare_libc_malloc();
        let size = self.target_data.get_store_size(&struct_ty);
        let call = self.builder.build_call(
            malloc,
            &[self.context.i64_type().const_int(size, false).into()],
            &format!("{name}_raw"),
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| frontend_error("malloc 未返回 pointer".to_string()))?
            .into_pointer_value();
        let ptr =
            self.refactor_cast_ptr(raw, self.context.ptr_type(AddressSpace::default()), name)?;
        self.builder.build_store(ptr, struct_ty.const_zero())?;
        Ok(ptr)
    }

    fn refactor_cast_ptr(
        &self,
        ptr: PointerValue<'ctx>,
        target_ty: inkwell::types::PointerType<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if ptr.get_type().get_address_space() == target_ty.get_address_space() {
            Ok(self.builder.build_pointer_cast(ptr, target_ty, name)?)
        } else {
            Ok(self
                .builder
                .build_address_space_cast(ptr, target_ty, name)?)
        }
    }

    fn refactor_build_step_complete(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        self.refactor_build_step_variant(
            step_layout,
            step_layout.complete_variant(),
            STEP_TAG_COMPLETE as u32,
            payload,
            None,
        )
    }

    fn refactor_build_step_case(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        case_layout: &RefactorStepCaseLayout<'ctx>,
        payload: Option<BasicValueEnum<'ctx>>,
        continuation: PointerValue<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        self.refactor_build_step_variant(
            step_layout,
            case_layout.variant(),
            case_layout.variant().tag_value(),
            payload,
            Some(continuation),
        )
    }

    fn refactor_build_step_variant(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        variant: &RefactorStepVariantLayout<'ctx>,
        tag: u32,
        payload: Option<BasicValueEnum<'ctx>>,
        continuation: Option<PointerValue<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let step_ptr = self
            .builder
            .build_alloca(step_layout.llvm_ty(), "refactor_step_tmp")?;
        self.builder
            .build_store(step_ptr, step_layout.llvm_ty().const_zero())?;
        let tag_ptr = self.builder.build_struct_gep(
            step_layout.llvm_ty(),
            step_ptr,
            0,
            "refactor_step_tag_gep",
        )?;
        self.builder.build_store(
            tag_ptr,
            self.context.i32_type().const_int(u64::from(tag), false),
        )?;
        let storage_ptr = self.builder.build_struct_gep(
            step_layout.llvm_ty(),
            step_ptr,
            1,
            "refactor_step_storage_gep",
        )?;
        let payload_ptr = self.refactor_cast_ptr(
            storage_ptr,
            self.context.ptr_type(AddressSpace::default()),
            "refactor_step_payload_ptr",
        )?;
        let mut payload_value = variant.payload_ty().get_undef();
        let mut next_field = 0u32;
        if !variant.payload_is_elided() {
            let payload = payload.ok_or_else(|| {
                frontend_error(format!(
                    "refactor Step variant tag {} 需要 payload，但 lowering 未提供",
                    tag
                ))
            })?;
            payload_value = self
                .builder
                .build_insert_value(
                    payload_value,
                    payload,
                    next_field,
                    "refactor_step_payload_insert",
                )?
                .into_struct_value();
            next_field += 1;
        }
        if let Some(continuation) = continuation {
            payload_value = self
                .builder
                .build_insert_value(
                    payload_value,
                    continuation,
                    next_field,
                    "refactor_step_cont_insert",
                )?
                .into_struct_value();
        }
        self.builder.build_store(payload_ptr, payload_value)?;
        Ok(self
            .builder
            .build_load(step_layout.llvm_ty(), step_ptr, "refactor_step")?)
    }

    fn refactor_extract_step_tag(
        &mut self,
        _step_layout: &RefactorStepLayout<'ctx>,
        step: BasicValueEnum<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let BasicValueEnum::StructValue(step) = step else {
            return Err(frontend_error(
                "refactor Step value 不是 struct".to_string(),
            ));
        };
        Ok(self
            .builder
            .build_extract_value(step, 0, "refactor_step_tag")?
            .into_int_value())
    }

    fn refactor_extract_step_payload(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        step: BasicValueEnum<'ctx>,
        variant: &RefactorStepVariantLayout<'ctx>,
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let (payload, _) =
            self.refactor_extract_step_payload_struct(step_layout, step, variant, name)?;
        if variant.payload_is_elided() {
            return Ok(None);
        }
        Ok(Some(self.builder.build_extract_value(payload, 0, name)?))
    }

    fn refactor_extract_step_case_parts(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        step: BasicValueEnum<'ctx>,
        case_layout: &RefactorStepCaseLayout<'ctx>,
        name: &str,
    ) -> Result<(Option<BasicValueEnum<'ctx>>, PointerValue<'ctx>), LlvmEmitError> {
        let variant = case_layout.variant();
        let (payload_struct, _) =
            self.refactor_extract_step_payload_struct(step_layout, step, variant, name)?;
        let payload = if variant.payload_is_elided() {
            None
        } else {
            Some(
                self.builder
                    .build_extract_value(payload_struct, 0, &format!("{name}_payload"))?,
            )
        };
        let cont_index = if variant.payload_is_elided() { 0 } else { 1 };
        let cont = self
            .builder
            .build_extract_value(payload_struct, cont_index, &format!("{name}_cont"))?
            .into_pointer_value();
        Ok((payload, cont))
    }

    fn refactor_extract_step_payload_struct(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        step: BasicValueEnum<'ctx>,
        variant: &RefactorStepVariantLayout<'ctx>,
        name: &str,
    ) -> Result<(inkwell::values::StructValue<'ctx>, PointerValue<'ctx>), LlvmEmitError> {
        let step_ptr = self
            .builder
            .build_alloca(step_layout.llvm_ty(), &format!("{name}_step_tmp"))?;
        self.builder.build_store(step_ptr, step)?;
        let storage_ptr = self.builder.build_struct_gep(
            step_layout.llvm_ty(),
            step_ptr,
            1,
            &format!("{name}_storage_gep"),
        )?;
        let payload_ptr = self.refactor_cast_ptr(
            storage_ptr,
            self.context.ptr_type(AddressSpace::default()),
            &format!("{name}_payload_ptr"),
        )?;
        let payload = self
            .builder
            .build_load(variant.payload_ty(), payload_ptr, name)?
            .into_struct_value();
        Ok((payload, payload_ptr))
    }
}

fn refactor_mir_callable<'a>(
    pass_view: &'a mir::MaterializedMirPassView<'a>,
    fqn: &str,
) -> Result<&'a mir::FunDecl, LlvmEmitError> {
    pass_view
        .callable(fqn)
        .or_else(|| {
            pass_view
                .materialized()
                .file
                .items
                .iter()
                .find_map(|item| match item {
                    mir::Item::Fun(fun) if fun.fqn == fqn && fun.body.is_some() => Some(fun),
                    _ => None,
                })
        })
        .or_else(|| {
            pass_view
                .materialized()
                .caller_side_pass_candidate_bodies()
                .iter()
                .find(|fun| fun.fqn == fqn && fun.body.is_some())
        })
        .ok_or_else(|| {
            frontend_error(format!(
                "refactor body lowering 缺少 callable `{fqn}` 的 materialized MIR body"
            ))
        })
}

fn boundary_site(boundary: &LateLoweredBoundary, expected: &str) -> Result<SiteId, LlvmEmitError> {
    match boundary.source() {
        LateLoweredBoundarySource::Site { site_id, .. } => Ok(site_id),
        other => Err(frontend_error(format!(
            "refactor {expected} boundary bd{} 绑定到非 site source {other:?}",
            boundary.boundary_id().as_u32()
        ))),
    }
}

fn boundary_consumption(
    boundary: &LateLoweredBoundary,
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

fn frame_slot_local(kind: crate::effect_lowered::ir::LateLoweredFrameSlotKind) -> Option<LocalId> {
    match kind {
        crate::effect_lowered::ir::LateLoweredFrameSlotKind::SourceLocal(local)
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::CompilerTemporary(local)
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::JoinValue { local, .. }
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandleBinder { local, .. }
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::BoundaryResult { local, .. } => {
            Some(local)
        }
        crate::effect_lowered::ir::LateLoweredFrameSlotKind::HandlePendingPayload { .. }
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::ResumePayload { .. }
        | crate::effect_lowered::ir::LateLoweredFrameSlotKind::System(_) => None,
    }
}

fn frontend_error(message: String) -> LlvmEmitError {
    LlvmEmitError::Frontend { message }
}
