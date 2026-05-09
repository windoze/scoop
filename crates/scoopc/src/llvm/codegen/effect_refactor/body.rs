//! Refactor LLVM body lowering（P6-T03）。
//!
//! This module lowers the P5 late-lowered state graph directly.  Generic MIR
//! lowering is reused only for effect-neutral source slices; every boundary,
//! resume payload binding, completion payload, and state transition comes from
//! the published late-lowered / ABI query contract.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use inkwell::AddressSpace;
use inkwell::basic_block::BasicBlock;
use inkwell::types::{BasicMetadataTypeEnum, BasicTypeEnum, StructType};
use inkwell::values::{
    BasicMetadataValueEnum, BasicValueEnum, FunctionValue, GlobalValue, IntValue, PointerValue,
};

use crate::effect_facts::{CaseTag, StepSchemaId};
use crate::effect_lowered::LateLoweredProgram;
use crate::effect_lowered::ir::{
    BoundaryId, BoundarySiteKind, LateLoweredBoundary, LateLoweredBoundaryLowering,
    LateLoweredBoundarySource, LateLoweredBoundarySourceConsumption,
    LateLoweredCallBoundaryContinuationComposition, LateLoweredCallBoundaryLowering,
    LateLoweredCallable, LateLoweredCompletionPayloadSource, LateLoweredConsumedRuntimeErrorCase,
    LateLoweredContinuationResumeBody, LateLoweredHandleBoundaryCaseRoutingAction,
    LateLoweredHandlePendingCompletion, LateLoweredHandlePendingCompletionOrigin,
    LateLoweredHandleStateRegion, LateLoweredOperandSource, LateLoweredOperandValueSource,
    LateLoweredPlainBodySlice, LateLoweredPlainCallable, LateLoweredResumePayloadBinding,
    LateLoweredSourceStatementClassificationKind, LateLoweredState, LateLoweredStateRole,
    LateLoweredStateTerminator, LateLoweredStepCaseForwarding, LateLoweredStepDispatchPlan,
    LateLoweredSurfaceResumeDispatchPublication,
    LateLoweredSurfaceResumeWrapperCompletePayloadSource, ResumeInterfaceId, StateId,
};
use crate::llvm::LlvmEmitError;
use crate::mir::{self, LocalId, SiteId};
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::super::mir_body::{MirLocalSlot, collect_mir_local_uses};
use super::super::types::{CgTy, CgValue, IntTy};
use super::super::{
    MainCodegen, RefactorCallableCarrierKind, TypeDescriptorSpec, sanitize_llvm_ident,
};
use super::types::{
    RefactorAbiQuery, RefactorCallTargetQuery, RefactorCallableEntryLayout, RefactorCallableLayout,
    RefactorContinuationSurfaceResumeLayout, RefactorDynamicInvokeCarrierLayout,
    RefactorDynamicInvokeLayout, RefactorFrameLayout, RefactorHandleContinuationBinderLayout,
    RefactorHandlePayloadBinderLayout, RefactorLocalRuntimeErrorTerminalAction,
    RefactorPlainCallableLayout, RefactorSourceAbiLayout, RefactorSourceAbiLayoutKind,
    RefactorStepCaseLayout, RefactorStepLayout, RefactorStepVariantLayout,
};
use super::value::RefactorValuePrimitives;

const STEP_TAG_COMPLETE: u64 = 0;
const REFACTOR_MAIN_UNHANDLED_EXIT_CODE: u64 = 3;
const CONT_FIELD_CAPTURED_FRAME: u32 = 1;
const CONT_FIELD_RESUME_STATE: u32 = 2;
const CONT_FIELD_ONE_SHOT: u32 = 3;
const CONT_FIELD_COMPOSED_CALLEE: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefactorHandleCompletionMode {
    ContinueToExit,
    ReturnFromFunction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RefactorCallableReturnMode {
    Step,
    Plain { declared_return_cg: CgTy },
}

impl RefactorHandleCompletionMode {
    fn pending_completion(self) -> LateLoweredHandlePendingCompletion {
        match self {
            Self::ContinueToExit => LateLoweredHandlePendingCompletion::ContinueToExit,
            Self::ReturnFromFunction => LateLoweredHandlePendingCompletion::ReturnFromFunction,
        }
    }
}

#[derive(Clone)]
struct RefactorHandleConsumeArmRuntime {
    arm_state: StateId,
    payload_binders: Vec<RefactorHandlePayloadBinderLayout>,
    continuation_binder: Option<RefactorHandleContinuationBinderLayout>,
}

#[derive(Clone, Copy)]
struct RefactorHandlePendingPayloadRuntime {
    completion: LateLoweredHandlePendingCompletion,
    payload_tuple_ty: TypeId,
    frame_field_index: u32,
}

#[derive(Clone)]
struct RefactorHandlePendingCompletionRuntime {
    site_id: SiteId,
    completion: LateLoweredHandlePendingCompletion,
    completion_tag_value: u32,
    completion_tag_field_index: u32,
    finally_state: StateId,
    payload_transport: Option<RefactorHandlePendingPayloadRuntime>,
}

#[derive(Clone)]
struct RefactorLocalRuntimeErrorRuntime {
    site_id: SiteId,
    input_case_tag: CaseTag,
    payload_tuple_ty: TypeId,
    target_state: StateId,
    runtime_symbol: String,
    runtime_param_count: usize,
}

#[derive(Clone)]
enum RefactorHandleBoundaryRuntimeAction {
    ConsumeToArm(RefactorHandleConsumeArmRuntime),
    PendingCompletion(RefactorHandlePendingCompletionRuntime),
    EmitOutward,
}

#[derive(Clone)]
enum RefactorHandleGotoRuntimeAction {
    BeginCompletion(RefactorHandlePendingCompletionRuntime),
    FinishFinally(RefactorHandleFinallyRuntime),
}

#[derive(Clone, Copy)]
struct RefactorHandleOutwardCompletionRuntime {
    boundary_id: BoundaryId,
    completion_tag_value: u32,
    case_tag: CaseTag,
    payload_tuple_ty: TypeId,
    resume_state: StateId,
    payload_transport: Option<RefactorHandlePendingPayloadRuntime>,
}

#[derive(Clone)]
struct RefactorHandleFinallyRuntime {
    site_id: SiteId,
    completion_tag_field_index: u32,
    exit_state: StateId,
    continue_to_exit_tag: u32,
    return_from_function_tag: u32,
    return_payload_source: Option<LateLoweredCompletionPayloadSource>,
    propagate_outward: Vec<RefactorHandleOutwardCompletionRuntime>,
}

#[derive(Clone, Copy)]
struct RefactorResumeUnwindOrigin<'a> {
    suspend_state: StateId,
    cleanup_state: StateId,
    resume_state: StateId,
    boundary_ids: &'a [BoundaryId],
}

enum RefactorClassCtorBoundarySource<'a> {
    ClassCtor {
        span: crate::span::Span,
        ctor: &'a mir::ClassCtorCallMetadata,
        args: &'a [mir::CallArg],
    },
    ObjectProperty {
        span: crate::span::Span,
        fqn: &'a str,
    },
    TopLevelRef {
        span: crate::span::Span,
        fqn: &'a str,
    },
}

impl RefactorClassCtorBoundarySource<'_> {
    fn span(&self) -> crate::span::Span {
        match self {
            Self::ClassCtor { span, .. }
            | Self::ObjectProperty { span, .. }
            | Self::TopLevelRef { span, .. } => *span,
        }
    }
}

struct TaskTransportResumeCandidate<'a, 'ctx> {
    callable: &'a LateLoweredCallable,
    adapter: FunctionValue<'ctx>,
    type_desc_i8: PointerValue<'ctx>,
    dispatch_plan: LateLoweredStepDispatchPlan,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    /// Defines all refactor ABI function bodies published by the P5/P6 handoff.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn codegen_refactor_program_bodies(
        &mut self,
        program: &'a LateLoweredProgram,
        abi_program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi_source_types: &'a TypeStore,
        abi_pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &RefactorAbiQuery<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        for callable in program.callables() {
            let mut child = self.fresh_child_codegen();
            if callable.plain_abi().is_some() {
                child.codegen_refactor_plain_callable_entry(
                    program,
                    source_types,
                    pass_view,
                    abi,
                    callable,
                )?;
            } else {
                child.codegen_refactor_callable_entries(
                    program,
                    source_types,
                    pass_view,
                    abi,
                    callable,
                )?;
            }
        }
        for (kind, carrier_fqn, target) in abi.callable_carrier_target_layouts() {
            let mut child = self.fresh_child_codegen();
            child.codegen_refactor_callable_carrier_entry_shell(
                kind,
                carrier_fqn,
                target,
                abi_source_types,
                abi_pass_view,
                abi,
            )?;
        }
        for interface in abi_program.resume_packings() {
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
                if !resume_packing_method_is_reachable(
                    abi_program,
                    interface.interface_id(),
                    method.case_tag(),
                ) {
                    let mut child = self.fresh_child_codegen();
                    child.codegen_refactor_unreachable_resume_method(
                        method_layout.symbol_name(),
                        method_layout.llvm_ty(),
                    )?;
                    continue;
                }
                let callable = abi_program
                .callables()
                .iter()
                    .find(|callable| callable.body_step_schema() == Some(method.out_step_schema()))
                    .ok_or_else(|| frontend_error(format!(
                        "refactor body lowering 缺少 resume method case c{} 的 owner step schema s{} callable",
                        method.case_tag().as_u32(),
                        method.out_step_schema().as_u32()
                    )))?;
                let mut child = self.fresh_child_codegen();
                child.codegen_refactor_resume_method(
                    abi_program,
                    abi_source_types,
                    abi_pass_view,
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
            let mut child = self.fresh_child_codegen();
            child.codegen_refactor_surface_resume(program, abi, surface)?;
        }
        for entry in abi_program.surface_resume_dispatch_inventory() {
            let surface = abi
                .surface_resume_layout(entry.continuation_schema())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor body lowering 缺少 ABI continuation schema k{} 的 surface-resume layout",
                        entry.continuation_schema().as_u32()
                    ))
                })?;
            let dispatch = abi.surface_resume_dispatch_layout(entry.continuation_schema())?;
            for target in dispatch.target().owner_trampolines() {
                let mut child = self.fresh_child_codegen();
                child.codegen_refactor_surface_resume_owner_trampoline(
                    abi_program,
                    abi_source_types,
                    abi_pass_view,
                    abi,
                    surface,
                    target,
                )?;
            }
            let mut child = self.fresh_child_codegen();
            child.codegen_refactor_surface_resume(abi_program, abi, surface)?;
        }
        for callable in program.callables() {
            if !callable.has_control_body() {
                continue;
            }
            for boundary in callable.boundary_map().entries() {
                let compositions = match boundary.lowering() {
                    Some(LateLoweredBoundaryLowering::Call(lowering)) => {
                        lowering.continuation_compositions()
                    }
                    Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                        lowering.continuation_compositions()
                    }
                    _ => continue,
                };
                for composition in compositions {
                    let continuation_schema = composition
                        .callee_continuation_contract()
                        .continuation_schema();
                    let surface = abi.surface_resume_layout(continuation_schema).ok_or_else(|| {
                        frontend_error(format!(
                            "refactor body lowering 缺少 dynamic continuation schema k{} 的 surface-resume layout",
                            continuation_schema.as_u32()
                        ))
                    })?;
                    let Some(function) = self.module.get_function(surface.symbol_name()) else {
                        continue;
                    };
                    if function.count_basic_blocks() > 0 {
                        continue;
                    }
                    let mut child = self.fresh_child_codegen();
                    child.codegen_refactor_dynamic_surface_resume_adapter(program, abi, surface)?;
                }
            }
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
        let mut entry_callables = program
            .callables()
            .iter()
            .filter(|callable| callable.root_fqn() == hir_main.fqn);
        let callable = entry_callables.next().ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM main wrapper 缺少入口 `{}` 的 callable body",
                hir_main.fqn
            ))
        })?;
        if entry_callables.next().is_some() {
            return Err(frontend_error(format!(
                "refactor LLVM main wrapper 发现入口 `{}` 存在多个 callable body version；必须通过 body version key 明确选择入口 shell",
                hir_main.fqn
            )));
        }
        if callable.plain_abi().is_some() {
            return self.codegen_refactor_plain_main_exit_code(
                hir_main,
                entry_argv_array,
                callable,
                abi,
            );
        }
        if entry_argv_array.is_some() {
            return Err(frontend_error(
                "refactor LLVM effect-step main wrapper 尚未发布 Array<String> argv Step ABI"
                    .to_string(),
            ));
        }

        let layout = abi.callable_layout_by_version_key(callable.body_version_key())?;
        let direct = self.refactor_function(layout.direct_entry().symbol_name())?;
        let args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        if !layout.direct_entry().args_abi().is_elided() {
            return Err(frontend_error(format!(
                "refactor LLVM main wrapper 入口 `{}` 的 direct entry args ABI 非 elided；Array<String> argv tuple ABI 尚未发布或 entry contract 漂移",
                hir_main.fqn
            )));
        }
        let call = self
            .builder
            .build_call(direct, &args, "refactor_main_step")?;
        let step = call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error("refactor main direct entry 未返回 Step_F".to_string())
        })?;
        let step_layout = abi.step_layout(layout.step_schema()).ok_or_else(|| {
            frontend_error(format!(
                "refactor LLVM main wrapper 缺少入口 step schema s{} layout",
                layout.step_schema().as_u32()
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
        let unhandled_exit = if step_layout.cases().is_empty() {
            self.builder.build_unreachable()?;
            None
        } else {
            // The process ABI cannot return `Step_F`; an escaped outward case is a
            // terminal program result at this boundary.
            let exit = self
                .context
                .i32_type()
                .const_int(REFACTOR_MAIN_UNHANDLED_EXIT_CODE, false);
            self.builder.build_unconditional_branch(done_bb)?;
            Some(exit)
        };

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
        if let Some(exit) = unhandled_exit {
            phi.add_incoming(&[(&exit, bad_bb)]);
        }
        Ok(phi.as_basic_value().into_int_value())
    }

    fn codegen_refactor_plain_main_exit_code(
        &mut self,
        hir_main: &crate::hir::FunDecl,
        entry_argv_array: Option<PointerValue<'ctx>>,
        callable: &LateLoweredCallable,
        abi: &RefactorAbiQuery<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let layout = abi.plain_callable_layout_by_version_key(callable.body_version_key())?;
        if layout.root_fqn() != callable.root_fqn() {
            return Err(frontend_error(format!(
                "refactor plain main `{}` 的 ABI layout root 漂移：layout=`{}`",
                callable.root_fqn(),
                layout.root_fqn(),
            )));
        }
        let entry = layout.direct_entry();
        let args = match (hir_main.params.as_slice(), entry_argv_array) {
            ([], None) if entry.param_count() == 0 => Vec::new(),
            ([_param], Some(argv_array)) if entry.param_count() == 1 => vec![argv_array.into()],
            ([], Some(_)) => {
                return Err(frontend_error(format!(
                    "refactor plain main `{}` 没有 source argv 参数，但 wrapper 收到了 argv array",
                    hir_main.fqn,
                )));
            }
            ([_], None) => {
                return Err(frontend_error(format!(
                    "refactor plain main `{}` 需要 argv array，但 wrapper 未收到入口 argv",
                    hir_main.fqn,
                )));
            }
            _ => {
                return Err(frontend_error(format!(
                    "refactor plain main `{}` argv ABI 漂移：source_params={} direct_params={}",
                    hir_main.fqn,
                    hir_main.params.len(),
                    entry.param_count(),
                )));
            }
        };
        let direct = self.refactor_function(entry.symbol_name())?;
        let call = self
            .builder
            .build_call(direct, &args, "refactor_plain_main")?;
        match self.cg_ty_of(hir_main.return_ty) {
            Some(CgTy::Unit) => Ok(self.context.i32_type().const_zero()),
            Some(CgTy::Int(_)) => {
                let raw = call.try_as_basic_value().basic().ok_or_else(|| {
                    frontend_error(format!(
                        "refactor plain main `{}` 的普通入口未返回整数值",
                        hir_main.fqn
                    ))
                })?;
                let BasicValueEnum::IntValue(value) = raw else {
                    return Err(frontend_error(format!(
                        "refactor plain main `{}` 的普通入口返回值不是整数",
                        hir_main.fqn
                    )));
                };
                Ok(self.builder.build_int_truncate_or_bit_cast(
                    value,
                    self.context.i32_type(),
                    "refactor_plain_main_exit_i32",
                )?)
            }
            _ => Err(frontend_error(format!(
                "refactor plain main wrapper 不支持入口 `{}` 的返回类型",
                hir_main.fqn
            ))),
        }
    }

    fn codegen_refactor_plain_callable_entry(
        &mut self,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &RefactorAbiQuery<'ctx>,
        callable: &'a LateLoweredCallable,
    ) -> Result<(), LlvmEmitError> {
        let plain = callable.plain_abi().ok_or_else(|| {
            frontend_error(format!(
                "refactor plain body lowering callable `{}` 缺少 plain ABI handoff",
                callable.root_fqn()
            ))
        })?;
        let layout = abi.plain_callable_layout_by_version_key(callable.body_version_key())?;
        validate_plain_callable_layout(callable, layout)?;
        let function = self.refactor_function(layout.direct_entry().symbol_name())?;
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }

        let hir_fun = self.hir_fun_for_callable_fqn(callable.root_fqn());
        let mir_fun = refactor_mir_callable(pass_view, callable.root_fqn())?;
        let is_materialized_closure = hir_fun.is_none() && mir_fun.name.starts_with("$lambda");
        if hir_fun.is_none() && !is_materialized_closure {
            return Err(frontend_error(format!(
                "refactor plain body lowering callable `{}` 缺少 HIR signature",
                callable.root_fqn()
            )));
        }
        let body = mir_fun.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "refactor plain body lowering callable `{}` 缺少 canonical MIR body",
                callable.root_fqn()
            ))
        })?;
        let mir_types = &pass_view.materialized().types;
        body.validate_cfg()
            .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                kind: "refactor plain callable cfg",
                at: mir_fun.span.into(),
            })?;
        self.verify_mir_body_composite_transport_contract(
            callable.root_fqn(),
            mir_fun.span,
            body,
            mir_types,
        )?;
        let body_slices = validate_plain_body_slices(callable.root_fqn(), plain, body)?;

        self.current_source_id = if let Some(hir_fun) = hir_fun {
            self.source_id_for_path(hir_fun.source_path.as_path(), hir_fun.span)?
        } else {
            self.materialized_mir_callable_source_id(callable.root_fqn(), mir_fun.span)?
        };
        self.function_cx.current_callable_fqn = Some(callable.root_fqn().to_string());
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        self.begin_function_explicit_frame_layout(function)?;

        let declared_return_cg = self.cg_ty_of_mir_type(mir_types, mir_fun.return_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "refactor plain callable return type",
                at: mir_fun.span.into(),
            },
        )?;
        self.function_cx.current_fun_return_ty = Some(declared_return_cg);
        let uses_hidden_sret = self
            .hidden_sret_result_ty(mir_fun.span, declared_return_cg)?
            .is_some();
        self.function_cx.current_sret_return_ptr = if uses_hidden_sret {
            Some(
                function
                    .get_nth_param(0)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing refactor plain llvm function sret param",
                        at: mir_fun.span.into(),
                    })?
                    .into_pointer_value(),
            )
        } else {
            None
        };

        let (return_bb, return_alloca) =
            self.setup_function_return_context(mir_fun.span, function, declared_return_cg)?;
        if plain.local_effect_control().is_some() {
            let emitter = RefactorCallableEmitter::new(
                self,
                program,
                source_types,
                pass_view,
                abi,
                callable,
                mir_fun,
                body,
                function,
                None,
                None,
                None,
                RefactorHandleCompletionMode::ContinueToExit,
            )?;
            if let Some(hir_fun) = hir_fun {
                emitter.emit_plain_direct(
                    hir_fun,
                    u32::from(uses_hidden_sret),
                    declared_return_cg,
                )?;
            } else if is_materialized_closure {
                emitter.emit_plain_direct_mir_params(
                    u32::from(uses_hidden_sret),
                    declared_return_cg,
                )?;
            } else {
                return Err(frontend_error(format!(
                    "refactor plain callable `{}` 的 local effect/control path 缺少 HIR function shell",
                    callable.root_fqn(),
                )));
            }
            self.emit_function_return_block(
                mir_fun.span,
                declared_return_cg,
                return_bb,
                return_alloca,
            )?;
            self.finish_function_explicit_frame_layout(mir_fun.span)?;
            self.function_cx.current_sret_return_ptr = None;
            return Ok(());
        }
        let mut slots = self.create_mir_local_slots(body, mir_types)?;
        if let Ok(source_id) =
            self.materialized_mir_callable_source_id(callable.root_fqn(), mir_fun.span)
        {
            self.current_source_id = source_id;
        }
        if let Some(hir_fun) = hir_fun {
            self.bind_mir_params(
                hir_fun,
                mir_fun,
                function,
                u32::from(uses_hidden_sret),
                &mut slots,
            )?;
        } else {
            self.bind_mir_closure_params(
                mir_fun,
                mir_types,
                function,
                u32::from(uses_hidden_sret),
                &mut slots,
            )?;
        }
        let used_locals = collect_mir_local_uses(body);
        let llvm_blocks = body
            .blocks
            .iter()
            .enumerate()
            .map(|(idx, _)| {
                self.context
                    .append_basic_block(function, &format!("plain.bb{idx}"))
            })
            .collect::<Vec<_>>();
        let start_bb = llvm_blocks
            .get(body.start.as_u32() as usize)
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor plain callable start block",
                at: mir_fun.span.into(),
            })?;
        self.builder.build_unconditional_branch(start_bb)?;

        for (idx, block) in body.blocks.iter().enumerate() {
            self.builder.position_at_end(llvm_blocks[idx]);
            let block_id = mir::BasicBlockId::from_raw(idx as u32);
            let slice = body_slices.get(&block_id).ok_or_else(|| {
                frontend_error(format!(
                    "refactor plain body lowering callable `{}` 缺少 bb{} 的 published source slice",
                    callable.root_fqn(),
                    block_id.as_u32(),
                ))
            })?;
            {
                let mut values = RefactorValuePrimitives::new(self, mir_types, body, &slots, abi);
                for stmt in &block.stmts
                    [slice.start_statement_index() as usize..slice.end_statement_index() as usize]
                {
                    values.lower_effect_neutral_statement(stmt, &used_locals)?;
                }
            }
            self.codegen_refactor_plain_terminator(
                &block.terminator,
                &slots,
                &llvm_blocks,
                declared_return_cg,
            )?;
        }

        self.emit_function_return_block(
            mir_fun.span,
            declared_return_cg,
            return_bb,
            return_alloca,
        )?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
        self.function_cx.current_sret_return_ptr = None;
        Ok(())
    }

    fn codegen_refactor_plain_terminator(
        &mut self,
        terminator: &mir::Terminator,
        slots: &[MirLocalSlot<'ctx>],
        llvm_blocks: &[BasicBlock<'ctx>],
        declared_return_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        if self
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_some())
        {
            return Ok(());
        }
        match &terminator.kind {
            mir::TerminatorKind::Return { value } => {
                let value = match value {
                    Some(operand) => self.codegen_mir_operand_expected(
                        terminator.span,
                        operand,
                        slots,
                        Some(declared_return_cg),
                    )?,
                    None => self.default_value(terminator.span, declared_return_cg)?,
                };
                let value = self
                    .coerce_value(terminator.span, value, declared_return_cg)
                    .map_err(|err| match err {
                        LlvmEmitError::Frontend { message } => frontend_error(format!(
                            "refactor plain return coercion failed at {:?}: {message}",
                            terminator.span
                        )),
                        other => other,
                    })?;
                self.finish_function_return_path(terminator.span, declared_return_cg, value)
            }
            mir::TerminatorKind::Goto { target } => {
                let target_bb = llvm_blocks.get(target.as_u32() as usize).copied().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor plain goto target",
                        at: terminator.span.into(),
                    },
                )?;
                self.builder.build_unconditional_branch(target_bb)?;
                Ok(())
            }
            mir::TerminatorKind::CondBr {
                cond,
                then_target,
                else_target,
            } => {
                let cond = self
                    .codegen_mir_operand(terminator.span, cond, slots)?
                    .as_bool()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor plain branch condition",
                        at: terminator.span.into(),
                    })?;
                let then_bb = llvm_blocks
                    .get(then_target.as_u32() as usize)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor plain then target",
                        at: terminator.span.into(),
                    })?;
                let else_bb = llvm_blocks
                    .get(else_target.as_u32() as usize)
                    .copied()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor plain else target",
                        at: terminator.span.into(),
                    })?;
                self.builder
                    .build_conditional_branch(cond, then_bb, else_bb)?;
                Ok(())
            }
            mir::TerminatorKind::Unreachable => {
                self.builder.build_unreachable()?;
                Ok(())
            }
            mir::TerminatorKind::Perform { .. }
            | mir::TerminatorKind::ResumeUnwind
            | mir::TerminatorKind::Handle { .. }
            | mir::TerminatorKind::Todo(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor plain callable effect/control terminator",
                at: terminator.span.into(),
            }),
        }
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
        validate_callable_entry_layout(layout)?;
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
                pass_view,
                abi,
                callable,
                mir_fun,
                body,
                direct_fun,
                None,
                None,
                None,
                RefactorHandleCompletionMode::ContinueToExit,
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

    fn codegen_refactor_callable_carrier_entry_shell(
        &mut self,
        kind: RefactorCallableCarrierKind,
        carrier_fqn: &str,
        target: &super::types::RefactorCallableCarrierTargetLayout,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &RefactorAbiQuery<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let target_layout = abi.callable_layout_by_version_key(target.body_version_key())?;
        let function = self.refactor_function(target.symbol_name())?;
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let mir_fun = refactor_mir_callable(pass_view, target_layout.root_fqn())?;
        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        let direct_entry = target_layout.direct_entry();
        let args_payload = self.build_refactor_carrier_direct_args(
            kind,
            carrier_fqn,
            function,
            mir_fun,
            source_types,
            abi,
            direct_entry,
        )?;
        let direct_fun = self.refactor_function(direct_entry.symbol_name())?;
        let mut args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        if !direct_entry.args_abi().is_elided() {
            args.push(
                args_payload
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor carrier shell `{}` 需要 non-elided direct args payload",
                            target.symbol_name()
                        ))
                    })?
                    .into(),
            );
        }
        let call = self
            .builder
            .build_call(direct_fun, &args, "refactor_carrier_to_direct")?;
        let step = call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error(format!(
                "refactor carrier shell `{}` direct entry 未返回 Step_F",
                target.symbol_name()
            ))
        })?;
        let returned_step = if target.step_schema() == target_layout.step_schema() {
            step
        } else {
            self.project_refactor_step_to_schema(
                abi,
                step,
                target_layout.step_schema(),
                target.step_schema(),
            )?
        };
        self.builder.build_return(Some(&returned_step))?;
        Ok(())
    }

    fn project_refactor_step_to_schema(
        &mut self,
        abi: &RefactorAbiQuery<'ctx>,
        owner_step: BasicValueEnum<'ctx>,
        owner_step_schema: StepSchemaId,
        wrapper_step_schema: StepSchemaId,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let owner_layout = abi.step_layout(owner_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor carrier projection 缺少 owner step schema s{} layout",
                owner_step_schema.as_u32()
            ))
        })?;
        let wrapper_layout = abi.step_layout(wrapper_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor carrier projection 缺少 wrapper step schema s{} layout",
                wrapper_step_schema.as_u32()
            ))
        })?;
        let tag = self.refactor_extract_step_tag(owner_layout, owner_step)?;
        let function = self.current_function()?;
        let complete_bb = self
            .context
            .append_basic_block(function, "refactor_carrier_project_complete");
        let dispatch_bb = self
            .context
            .append_basic_block(function, "refactor_carrier_project_dispatch");
        let unmatched_bb = self
            .context
            .append_basic_block(function, "refactor_carrier_project_unmatched");
        let done_bb = self
            .context
            .append_basic_block(function, "refactor_carrier_project_done");
        let is_complete = self.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            tag,
            self.context.i32_type().const_int(STEP_TAG_COMPLETE, false),
            "refactor_carrier_project_is_complete",
        )?;
        self.builder
            .build_conditional_branch(is_complete, complete_bb, dispatch_bb)?;

        self.builder.position_at_end(dispatch_bb);
        let mut case_targets = Vec::new();
        for wrapper_case in wrapper_layout.cases() {
            let Some(owner_case) = owner_layout.cases().iter().find(|owner_case| {
                owner_case.1.concrete_op_key() == wrapper_case.1.concrete_op_key()
                    && owner_case.1.payload_tuple_ty() == wrapper_case.1.payload_tuple_ty()
            }) else {
                continue;
            };
            let bb = self.context.append_basic_block(
                function,
                &format!(
                    "refactor_carrier_project_case{}",
                    wrapper_case.1.case_tag().as_u32()
                ),
            );
            case_targets.push((
                self.context
                    .i32_type()
                    .const_int(owner_case.1.variant().tag_value() as u64, false),
                bb,
                owner_case.1.case_tag(),
                wrapper_case.1.case_tag(),
            ));
        }
        let switch_cases = case_targets
            .iter()
            .map(|(tag, bb, _, _)| (*tag, *bb))
            .collect::<Vec<_>>();
        self.builder
            .build_switch(tag, unmatched_bb, &switch_cases)?;

        let phi_ty = wrapper_layout.llvm_ty();
        self.builder.position_at_end(complete_bb);
        let complete_payload = self.refactor_extract_step_payload(
            owner_layout,
            owner_step,
            owner_layout.complete_variant(),
            "refactor_carrier_project_complete_payload",
        )?;
        let complete_step = self.refactor_build_step_complete(wrapper_layout, complete_payload)?;
        self.builder.build_unconditional_branch(done_bb)?;
        let complete_incoming = self.builder.get_insert_block().ok_or_else(|| {
            frontend_error("refactor carrier projection complete block missing".to_string())
        })?;

        let mut incomings = vec![(complete_step, complete_incoming)];
        for (_, bb, owner_case_tag, wrapper_case_tag) in case_targets {
            self.builder.position_at_end(bb);
            let owner_case = owner_layout.case_layout(owner_case_tag).ok_or_else(|| {
                frontend_error(format!(
                    "refactor carrier projection 缺少 owner case c{}",
                    owner_case_tag.as_u32()
                ))
            })?;
            let wrapper_case = wrapper_layout
                .case_layout(wrapper_case_tag)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor carrier projection 缺少 wrapper case c{}",
                        wrapper_case_tag.as_u32()
                    ))
                })?;
            let (payload, continuation) = self.refactor_extract_step_case_parts(
                owner_layout,
                owner_step,
                owner_case,
                "refactor_carrier_project_case_payload",
            )?;
            let projected =
                self.refactor_build_step_case(wrapper_layout, wrapper_case, payload, continuation)?;
            self.builder.build_unconditional_branch(done_bb)?;
            let incoming_bb = self.builder.get_insert_block().ok_or_else(|| {
                frontend_error("refactor carrier projection case block missing".to_string())
            })?;
            incomings.push((projected, incoming_bb));
        }

        self.builder.position_at_end(unmatched_bb);
        self.builder.build_unreachable()?;

        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(phi_ty, "refactor_carrier_projected_step")?;
        for (value, bb) in &incomings {
            phi.add_incoming(&[(value, *bb)]);
        }
        Ok(phi.as_basic_value())
    }

    #[allow(clippy::too_many_arguments)]
    fn build_refactor_carrier_direct_args(
        &mut self,
        kind: RefactorCallableCarrierKind,
        carrier_fqn: &str,
        function: FunctionValue<'ctx>,
        mir_fun: &mir::FunDecl,
        source_types: &TypeStore,
        abi: &RefactorAbiQuery<'ctx>,
        direct_entry: &RefactorCallableEntryLayout<'ctx>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let direct_args_layout = abi.source_value_layout(direct_entry.invoke_args_tuple_ty())?;
        let direct_component_count = refactor_source_layout_component_count(direct_args_layout);
        let receiver = function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "refactor carrier shell `{}` 缺少 receiver/carrier 参数",
                carrier_fqn
            ))
        })?;
        let mut components = vec![None; direct_component_count.max(mir_fun.params.len())];
        let (explicit_param_start, explicit_component_start) = match kind {
            RefactorCallableCarrierKind::ClosureObject if mir_fun.name.starts_with("$lambda") => {
                let flatten_env = mir_fun.params.first().is_some_and(|param| {
                    mir_fun.params.len() == 1 && param.ty == direct_entry.invoke_args_tuple_ty()
                });
                let env_components = self.load_refactor_closure_env_components(
                    receiver.into_pointer_value(),
                    mir_fun,
                    source_types,
                    flatten_env,
                )?;
                let env_component_count = env_components.len();
                components = env_components;
                components.resize(direct_component_count.max(env_component_count), None);
                (1, env_component_count)
            }
            RefactorCallableCarrierKind::ClosureObject => (0, 0),
            RefactorCallableCarrierKind::ClassVtable
            | RefactorCallableCarrierKind::InterfaceItable => {
                if components.is_empty() {
                    return Err(frontend_error(format!(
                        "refactor dispatch carrier `{carrier_fqn}` direct entry 缺少 receiver 参数"
                    )));
                }
                components[0] = Some(receiver);
                (1, 1)
            }
        };
        // Closure carriers for a single tuple-typed parameter already receive the exact
        // invoke-args ABI payload as their explicit args parameter; forwarding it intact
        // preserves the authoritative tuple source layout without dropping components.
        if matches!(kind, RefactorCallableCarrierKind::ClosureObject)
            && explicit_param_start == 0
            && explicit_component_start == 0
            && mir_fun.params.len() == 1
            && mir_fun.params[0].ty == direct_entry.invoke_args_tuple_ty()
            && matches!(
                source_types.kind(mir_fun.params[0].ty),
                TypeKind::Value(ValueTypeKind::Tuple(_))
            )
        {
            return if direct_args_layout.abi().is_elided() {
                Ok(None)
            } else {
                function.get_nth_param(1).map(Some).ok_or_else(|| {
                    frontend_error(format!(
                        "refactor carrier shell `{}` 缺少 explicit args payload 参数",
                        mir_fun.fqn
                    ))
                })
            };
        }
        self.unpack_refactor_carrier_explicit_args(
            function,
            mir_fun,
            explicit_param_start,
            explicit_component_start,
            source_types,
            &mut components,
        )?;
        self.build_refactor_source_payload_from_components(
            direct_args_layout,
            &components,
            "refactor_carrier_direct_args",
        )
    }

    fn unpack_refactor_carrier_explicit_args(
        &mut self,
        function: FunctionValue<'ctx>,
        mir_fun: &mir::FunDecl,
        explicit_param_start: usize,
        explicit_component_start: usize,
        source_types: &TypeStore,
        components: &mut [Option<BasicValueEnum<'ctx>>],
    ) -> Result<(), LlvmEmitError> {
        if explicit_param_start > mir_fun.params.len() {
            return Err(frontend_error(format!(
                "refactor carrier shell `{}` explicit arg 起点越界：start={} params={}",
                mir_fun.fqn,
                explicit_param_start,
                mir_fun.params.len(),
            )));
        }
        let explicit_params = &mir_fun.params[explicit_param_start..];
        if explicit_params.is_empty() {
            return Ok(());
        }
        let needed_components = explicit_component_start + explicit_params.len();
        if needed_components > components.len() {
            return Err(frontend_error(format!(
                "refactor carrier shell `{}` explicit arg component range 越界：start={} count={} components={}",
                mir_fun.fqn,
                explicit_component_start,
                explicit_params.len(),
                components.len(),
            )));
        }
        let elided = explicit_params
            .iter()
            .map(|param| self.refactor_source_type_is_elided(param.span, source_types, param.ty))
            .collect::<Result<Vec<_>, _>>()?;
        if elided.iter().all(|is_elided| *is_elided) {
            return Ok(());
        }
        let raw = function.get_nth_param(1).ok_or_else(|| {
            frontend_error(format!(
                "refactor carrier shell `{}` 缺少 explicit args payload 参数",
                mir_fun.fqn
            ))
        })?;
        if explicit_params.len() == 1 {
            components[explicit_component_start] = Some(raw);
            return Ok(());
        }
        let BasicValueEnum::StructValue(tuple) = raw else {
            return Err(frontend_error(format!(
                "refactor carrier shell `{}` explicit args payload 不是 tuple struct",
                mir_fun.fqn
            )));
        };
        let mut abi_field = 0u32;
        for (offset, is_elided) in elided.into_iter().enumerate() {
            if is_elided {
                continue;
            }
            let raw_field = self.builder.build_extract_value(
                tuple,
                abi_field,
                &format!("refactor_carrier_arg{offset}"),
            )?;
            components[explicit_component_start + offset] = Some(raw_field);
            abi_field += 1;
        }
        Ok(())
    }

    fn refactor_source_type_is_elided(
        &mut self,
        span: crate::span::Span,
        source_types: &TypeStore,
        ty: TypeId,
    ) -> Result<bool, LlvmEmitError> {
        let cg_ty =
            self.cg_ty_of_mir_type(source_types, ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor carrier arg type",
                    at: span.into(),
                })?;
        let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
        Ok(self.target_data.get_store_size(&llvm_ty) == 0)
    }

    fn load_refactor_closure_env_components(
        &mut self,
        closure_obj_i8: PointerValue<'ctx>,
        mir_fun: &mir::FunDecl,
        source_types: &TypeStore,
        flatten_env: bool,
    ) -> Result<Vec<Option<BasicValueEnum<'ctx>>>, LlvmEmitError> {
        let Some(env_param) = mir_fun.params.first() else {
            return Err(frontend_error(format!(
                "refactor closure carrier `{}` 缺少 lambda env 参数",
                mir_fun.fqn
            )));
        };
        let env_cg = self.cg_ty_of_mir_type(source_types, env_param.ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "refactor closure env type",
                at: env_param.span.into(),
            },
        )?;
        if env_cg == CgTy::Unit {
            return Ok(Vec::new());
        }
        let CgTy::Tuple(tuple_ty) = env_cg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor closure env payload shape",
                at: env_param.span.into(),
            });
        };
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor closure env tuple type",
                at: env_param.span.into(),
            });
        };
        let capture_cgs = elements
            .iter()
            .map(|ty| {
                self.cg_ty_of(*ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor closure env capture type",
                        at: env_param.span.into(),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let env_obj_ty =
            self.refactor_closure_env_object_type(env_param.span, &mir_fun.fqn, &capture_cgs)?;
        let env_i8 = self.load_refactor_closure_env_ref(closure_obj_i8)?;
        let env_ptr = self.refactor_cast_ptr(
            env_i8,
            self.context.ptr_type(self.gc_address_space()),
            "refactor_closure_env_obj",
        )?;
        let mut components = Vec::new();
        let mut aggregate = if flatten_env {
            None
        } else {
            let BasicTypeEnum::StructType(env_tuple_ty) =
                self.llvm_basic_type_of(env_param.span, env_cg)?
            else {
                return Err(frontend_error(format!(
                    "refactor closure env `{}` 的 env tuple LLVM type 不是 struct",
                    mir_fun.fqn,
                )));
            };
            Some(env_tuple_ty.get_undef())
        };
        for (index, capture_cg) in capture_cgs.iter().enumerate() {
            let field_ty = self.llvm_basic_type_of(env_param.span, *capture_cg)?;
            let raw = if matches!(capture_cg, CgTy::Unit | CgTy::Never) {
                self.zero_initializer_for_basic_type(field_ty)
            } else {
                let env_field_index = (index + 1) as u32;
                if env_field_index >= env_obj_ty.count_fields() {
                    return Err(frontend_error(format!(
                        "refactor closure env object `{}` 缺少 capture field {}（field_count={}）",
                        mir_fun.fqn,
                        env_field_index,
                        env_obj_ty.count_fields(),
                    )));
                }
                let field_gep = self.builder.build_struct_gep(
                    env_obj_ty,
                    env_ptr,
                    env_field_index,
                    &format!("refactor_closure_env_field{index}_gep"),
                )?;
                self.builder.build_load(
                    field_ty,
                    field_gep,
                    &format!("refactor_closure_env_field{index}"),
                )?
            };
            if let Some(current) = aggregate.take() {
                aggregate = Some(
                    self.builder
                        .build_insert_value(
                            current,
                            raw,
                            index as u32,
                            &format!("refactor_closure_env_tuple_field{index}"),
                        )?
                        .into_struct_value(),
                );
            } else {
                components.push(Some(raw));
            }
        }
        if let Some(aggregate) = aggregate {
            Ok(vec![Some(aggregate.into())])
        } else {
            Ok(components)
        }
    }

    fn load_refactor_closure_env_ref(
        &mut self,
        closure_obj_i8: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let closure_ty = self.llvm_closure_object_type();
        if closure_ty.count_fields() <= 1 {
            return Err(frontend_error(format!(
                "refactor closure object layout 缺少 env field（field_count={}）",
                closure_ty.count_fields(),
            )));
        }
        let closure_ptr = self.refactor_cast_ptr(
            closure_obj_i8,
            self.context.ptr_type(self.gc_address_space()),
            "refactor_closure_obj",
        )?;
        let env_gep = self.builder.build_struct_gep(
            closure_ty,
            closure_ptr,
            1,
            "refactor_closure_env_gep",
        )?;
        Ok(self
            .builder
            .build_load(self.llvm_gc_i8_ptr_type(), env_gep, "refactor_closure_env")?
            .into_pointer_value())
    }

    fn refactor_closure_env_object_type(
        &mut self,
        span: crate::span::Span,
        fn_ptr: &str,
        field_cgs: &[CgTy],
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let name = format!("scoop.mir.lambda_env${}", sanitize_llvm_ident(fn_ptr));
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }
        let env_ty = self.context.opaque_struct_type(&name);
        let mut fields = Vec::with_capacity(1 + field_cgs.len());
        fields.push(self.llvm_gc_object_header_type().into());
        for cg in field_cgs {
            fields.push(self.llvm_basic_type_of(span, *cg)?);
        }
        env_ty.set_body(&fields, false);
        Ok(env_ty)
    }

    fn build_refactor_source_payload_from_components(
        &mut self,
        layout: &RefactorSourceAbiLayout<'ctx>,
        components: &[Option<BasicValueEnum<'ctx>>],
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        if layout.abi().is_elided() {
            return Ok(None);
        }
        match layout.kind() {
            RefactorSourceAbiLayoutKind::Scalar => components
                .first()
                .and_then(|value| *value)
                .map(Some)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor ABI scalar payload `{name}` 缺少 source component"
                    ))
                }),
            RefactorSourceAbiLayoutKind::Tuple => {
                let BasicTypeEnum::StructType(struct_ty) = layout.abi().llvm_ty() else {
                    return Err(frontend_error(format!(
                        "refactor ABI tuple payload `{name}` layout 不是 struct"
                    )));
                };
                let mut aggregate = struct_ty.get_undef();
                for field in layout.fields() {
                    if field.is_elided() {
                        continue;
                    }
                    let source_index = field.source_index() as usize;
                    let raw = components
                        .get(source_index)
                        .and_then(|value| *value)
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "refactor ABI tuple payload `{name}` 缺少 source component {source_index}"
                            ))
                        })?;
                    aggregate = self
                        .builder
                        .build_insert_value(
                            aggregate,
                            raw,
                            field
                                .abi_field_index()
                                .expect("non-elided field has ABI index"),
                            &format!("{name}_field{source_index}"),
                        )?
                        .into_struct_value();
                }
                Ok(Some(aggregate.into()))
            }
        }
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
            pass_view,
            abi,
            callable,
            mir_fun,
            body,
            function,
            None,
            None,
            None,
            RefactorHandleCompletionMode::ReturnFromFunction,
        )?
        .emit_resume_method(case_tag, resume_tuple_ty)?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
        Ok(())
    }

    fn codegen_refactor_unreachable_resume_method(
        &mut self,
        symbol_name: &str,
        fn_ty: inkwell::types::FunctionType<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self
            .module
            .get_function(symbol_name)
            .unwrap_or_else(|| self.module.add_function(symbol_name, fn_ty, None));
        if function.count_basic_blocks() == 0 {
            let entry = self.context.append_basic_block(function, "entry");
            self.builder.position_at_end(entry);
            self.builder.build_unreachable()?;
        }
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
        let targets = dispatch.target().owner_trampolines();
        if targets.is_empty() {
            self.builder.build_unreachable()?;
            return Ok(());
        }
        let cont = function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "refactor surface resume `{}` 缺少 continuation 参数",
                surface.symbol_name()
            ))
        })?;
        let cont_ptr = cont.into_pointer_value();
        let mut args = vec![cont_ptr.into()];
        if surface.param_count() > 1 {
            let payload = function.get_nth_param(1).ok_or_else(|| {
                frontend_error(format!(
                    "refactor surface resume `{}` 缺少 resume payload 参数",
                    surface.symbol_name()
                ))
            })?;
            args.push(payload.into());
        }
        if targets.len() == 1 {
            let trampoline_fun = self.refactor_function(targets[0].symbol_name())?;
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
            return Ok(());
        }

        let current_desc = self.load_gc_object_type_desc(cont_ptr, "surface_resume_cont_desc")?;
        let word_ty = self.context.i64_type();
        let current_desc_int =
            self.builder
                .build_ptr_to_int(current_desc, word_ty, "surface_resume_cont_desc_int")?;
        let first_check = self
            .context
            .append_basic_block(function, "surface_resume_check0");
        self.builder.build_unconditional_branch(first_check)?;
        let mut check_bb = first_check;
        for (index, target) in targets.iter().enumerate() {
            let next_bb = self
                .context
                .append_basic_block(function, &format!("surface_resume_check{}", index + 1));
            let hit_bb = self.context.append_basic_block(
                function,
                &format!(
                    "surface_resume_hit_ko{}",
                    target.owner_continuation_object().as_u32()
                ),
            );
            self.builder.position_at_end(check_bb);
            let continuation_layout = abi
                .continuation_layout(target.owner_continuation_object())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor surface resume `{}` 缺少 owner continuation object ko{} layout",
                        surface.symbol_name(),
                        target.owner_continuation_object().as_u32(),
                    ))
                })?;
            let type_desc = self.get_or_create_refactor_gc_type_descriptor(
                crate::span::Span::new(0, 0),
                continuation_layout.llvm_ty(),
                continuation_layout.layout_anchor_name(),
            )?;
            let type_desc_i8 = self.builder.build_pointer_cast(
                type_desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "surface_resume_target_desc",
            )?;
            let target_desc_int = self.builder.build_ptr_to_int(
                type_desc_i8,
                word_ty,
                "surface_resume_target_desc_int",
            )?;
            let is_match = self.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                current_desc_int,
                target_desc_int,
                "surface_resume_desc_match",
            )?;
            self.builder
                .build_conditional_branch(is_match, hit_bb, next_bb)?;

            self.builder.position_at_end(hit_bb);
            let trampoline_fun = self.refactor_function(target.symbol_name())?;
            let call = self.builder.build_call(
                trampoline_fun,
                &args,
                "refactor_surface_resume_owner_call",
            )?;
            let owner_step = call.try_as_basic_value().basic().ok_or_else(|| {
                frontend_error(format!(
                    "refactor surface resume `{}` 调用 owner dispatch `{}` 未返回 Step_F",
                    surface.symbol_name(),
                    target.symbol_name(),
                ))
            })?;
            self.builder.build_return(Some(&owner_step))?;
            check_bb = next_bb;
        }

        self.builder.position_at_end(check_bb);
        self.builder.build_unreachable()?;
        Ok(())
    }

    fn codegen_refactor_dynamic_surface_resume_adapter(
        &mut self,
        program: &'a LateLoweredProgram,
        abi: &RefactorAbiQuery<'ctx>,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let function = self.refactor_function(surface.symbol_name())?;
        if function.count_basic_blocks() > 0 {
            return Ok(());
        }
        let wrapper_step = program
            .step_type(surface.return_step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor dynamic surface resume k{} 缺少 wrapper step schema s{}",
                    surface.continuation_schema().as_u32(),
                    surface.return_step_schema().as_u32()
                ))
            })?;
        let wrapper_case = wrapper_step
            .cases()
            .iter()
            .find(|case| {
                case.continuation_contract().continuation_schema() == surface.continuation_schema()
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor dynamic surface resume k{} 无法在 wrapper step s{} 中找到对应 case",
                    surface.continuation_schema().as_u32(),
                    wrapper_step.step_schema().as_u32()
                ))
            })?;

        let mut candidates = Vec::new();
        for callable in program.callables() {
            if !callable.has_control_body() || callable.step_schema() == wrapper_step.step_schema()
            {
                continue;
            }
            let Some(owner_step) = program.step_type(callable.step_schema()) else {
                continue;
            };
            let Some(owner_case) = owner_step.cases().iter().find(|case| {
                case.concrete_op_key() == wrapper_case.concrete_op_key()
                    && case.payload_tuple_ty() == wrapper_case.payload_tuple_ty()
                    && case.answer_ty() == wrapper_case.answer_ty()
            }) else {
                continue;
            };
            let Some(continuation_layout) = abi.continuation_layout(callable.continuation_object())
            else {
                continue;
            };
            let Some(owner_surface) =
                abi.surface_resume_layout(owner_case.continuation_contract().continuation_schema())
            else {
                continue;
            };
            candidates.push((callable, continuation_layout, owner_surface));
        }
        if candidates.is_empty() {
            let entry = self.context.append_basic_block(function, "entry");
            self.builder.position_at_end(entry);
            self.builder.build_unreachable()?;
            return Ok(());
        }

        let entry = self.context.append_basic_block(function, "entry");
        self.builder.position_at_end(entry);
        let cont = function.get_nth_param(0).ok_or_else(|| {
            frontend_error(format!(
                "refactor dynamic surface resume `{}` 缺少 continuation 参数",
                surface.symbol_name()
            ))
        })?;
        let cont_ptr = cont.into_pointer_value();
        let payload = if surface.param_count() > 1 {
            Some(function.get_nth_param(1).ok_or_else(|| {
                frontend_error(format!(
                    "refactor dynamic surface resume `{}` 缺少 payload 参数",
                    surface.symbol_name()
                ))
            })?)
        } else {
            None
        };
        let current_desc = self.load_gc_object_type_desc(cont_ptr, "dynamic_surface_cont_desc")?;
        let word_ty = self.context.i64_type();
        let current_desc_int = self.builder.build_ptr_to_int(
            current_desc,
            word_ty,
            "dynamic_surface_cont_desc_int",
        )?;
        let first_check = self
            .context
            .append_basic_block(function, "dynamic_surface_check0");
        self.builder.build_unconditional_branch(first_check)?;

        let mut check_bb = first_check;
        for (index, (callable, continuation_layout, owner_surface)) in
            candidates.into_iter().enumerate()
        {
            let next_bb = self
                .context
                .append_basic_block(function, &format!("dynamic_surface_check{}", index + 1));
            let hit_bb = self.context.append_basic_block(
                function,
                &format!("dynamic_surface_hit_s{}", callable.step_schema().as_u32()),
            );
            self.builder.position_at_end(check_bb);
            let type_desc = self.get_or_create_refactor_gc_type_descriptor(
                crate::span::Span::new(0, 0),
                continuation_layout.llvm_ty(),
                continuation_layout.layout_anchor_name(),
            )?;
            let type_desc_i8 = self.builder.build_pointer_cast(
                type_desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "dynamic_surface_target_desc",
            )?;
            let target_desc_int = self.builder.build_ptr_to_int(
                type_desc_i8,
                word_ty,
                "dynamic_surface_target_desc_int",
            )?;
            let is_match = self.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                current_desc_int,
                target_desc_int,
                "dynamic_surface_desc_match",
            )?;
            self.builder
                .build_conditional_branch(is_match, hit_bb, next_bb)?;

            self.builder.position_at_end(hit_bb);
            let owner_fun = self.refactor_function(owner_surface.symbol_name())?;
            let mut args = Vec::<BasicMetadataValueEnum<'ctx>>::from([cont_ptr.into()]);
            if owner_surface.param_count() > 1 {
                args.push(
                    payload
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "refactor dynamic surface resume `{}` target `{}` 需要 payload",
                                surface.symbol_name(),
                                owner_surface.symbol_name()
                            ))
                        })?
                        .into(),
                );
            }
            let call = self.build_call_preserving_gc_local_roots(
                crate::span::Span::new(0, 0),
                owner_fun,
                &args,
                "dynamic_surface_owner_resume",
            )?;
            let owner_step = call.try_as_basic_value().basic().ok_or_else(|| {
                frontend_error(format!(
                    "refactor dynamic surface resume `{}` target `{}` 未返回 Step_F",
                    surface.symbol_name(),
                    owner_surface.symbol_name()
                ))
            })?;
            let projected = self.project_refactor_step_to_schema(
                abi,
                owner_step,
                callable.step_schema(),
                surface.return_step_schema(),
            )?;
            self.builder.build_return(Some(&projected))?;
            check_bb = next_bb;
        }

        self.builder.position_at_end(check_bb);
        self.builder.build_unreachable()?;
        Ok(())
    }

    fn load_gc_object_type_desc(
        &mut self,
        obj: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr =
            self.builder
                .build_pointer_cast(obj, header_ptr_ty, &format!("{name}_hdr"))?;
        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, &format!("{name}_gep"))?;
        Ok(self
            .builder
            .build_load(self.llvm_i8_ptr_type(), type_desc_ptr, name)?
            .into_pointer_value())
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
            .or_else(|| {
                program.callables().iter().find(|candidate| {
                    candidate.body_version_key().surface_instance()
                        == target.owner_version_key().surface_instance()
                })
            })
            .or_else(|| {
                program.callables().iter().find(|candidate| {
                    candidate.root_fqn()
                        == target.owner_version_key().surface_instance().template.fqn
                })
            })
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor surface resume owner dispatch k{} 缺少 owner callable {:?}",
                    surface.continuation_schema().as_u32(),
                    target.owner_version_key()
                ))
            })?;
        if callable.step_schema() != target.owner_step_schema()
            && callable.body_version_key().surface_instance()
                != target.owner_version_key().surface_instance()
        {
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
        let mut surface_handle_sites = target
            .handle_binder_routes()
            .iter()
            .map(|route| route.site_id())
            .collect::<BTreeSet<_>>();
        if let Some(projection) = target.wrapper_projection()
            && let LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                site_id,
                ..
            } = projection.underlying_route().publication()
        {
            surface_handle_sites.insert(*site_id);
        }
        let surface_handle_sites =
            (!surface_handle_sites.is_empty()).then_some(surface_handle_sites);
        if abi.frame_layout(target.owner_step_schema()).is_none()
            && target.resume_boundary_sites().is_empty()
            && target.handle_binder_routes().is_empty()
            && target.wrapper_projection().is_none()
        {
            self.builder.build_unreachable()?;
            return Ok(());
        }
        let return_step_schema = (target.wrapper_projection().is_none()
            && target.owner_step_schema() != surface.return_step_schema())
        .then_some(surface.return_step_schema());
        RefactorCallableEmitter::new(
            self,
            program,
            source_types,
            pass_view,
            abi,
            callable,
            mir_fun,
            body,
            function,
            target.wrapper_projection(),
            return_step_schema,
            surface_handle_sites,
            RefactorHandleCompletionMode::ReturnFromFunction,
        )?
        .emit_resume_entry(surface.resume_tuple_ty())?;
        self.finish_function_explicit_frame_layout(mir_fun.span)?;
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
    program: &'a LateLoweredProgram,
    source_types: &'a TypeStore,
    pass_view: &'a mir::MaterializedMirPassView<'a>,
    abi: &'cg RefactorAbiQuery<'ctx>,
    callable: &'a LateLoweredCallable,
    mir_fun: &'a mir::FunDecl,
    body: &'a mir::Body,
    function: FunctionValue<'ctx>,
    slots: Vec<MirLocalSlot<'ctx>>,
    used_locals: HashSet<LocalId>,
    abi_step_schema: StepSchemaId,
    frame_layout: &'cg RefactorFrameLayout<'ctx>,
    step_layout: &'cg RefactorStepLayout<'ctx>,
    frame_root_slot: PointerValue<'ctx>,
    state_blocks: BTreeMap<StateId, BasicBlock<'ctx>>,
    return_projection:
        Option<&'cg crate::effect_lowered::ir::LateLoweredSurfaceResumeWrapperProjection>,
    return_step_schema: Option<StepSchemaId>,
    surface_resume_handle_sites: Option<BTreeSet<SiteId>>,
    handle_completion_mode: RefactorHandleCompletionMode,
    return_mode: RefactorCallableReturnMode,
}

struct ComposedBoundaryDispatchContext<'a> {
    call_lowering: Option<&'a LateLoweredCallBoundaryLowering>,
    dispatch: &'a LateLoweredStepDispatchPlan,
    continuation_compositions: &'a [LateLoweredCallBoundaryContinuationComposition],
}

impl<'cg, 'a, 'ctx> RefactorCallableEmitter<'cg, 'a, 'ctx> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        codegen: &'cg mut MainCodegen<'a, 'ctx>,
        program: &'a LateLoweredProgram,
        source_types: &'a TypeStore,
        pass_view: &'a mir::MaterializedMirPassView<'a>,
        abi: &'cg RefactorAbiQuery<'ctx>,
        callable: &'a LateLoweredCallable,
        mir_fun: &'a mir::FunDecl,
        body: &'a mir::Body,
        function: FunctionValue<'ctx>,
        return_projection: Option<
            &'cg crate::effect_lowered::ir::LateLoweredSurfaceResumeWrapperProjection,
        >,
        return_step_schema: Option<StepSchemaId>,
        surface_resume_handle_sites: Option<BTreeSet<SiteId>>,
        handle_completion_mode: RefactorHandleCompletionMode,
    ) -> Result<Self, LlvmEmitError> {
        if let Some(callable_layout) = callable
            .effect_step_abi()
            .map(|_| abi.callable_layout_by_version_key(callable.body_version_key()))
            .transpose()?
            && callable_layout.root_fqn() != callable.root_fqn()
        {
            return Err(frontend_error(format!(
                "refactor body lowering callable `{}` 的 ABI layout root 漂移：layout=`{}`",
                callable.root_fqn(),
                callable_layout.root_fqn(),
            )));
        }
        let body_step_schema = callable.body_step_schema().ok_or_else(|| {
            frontend_error(format!(
                "refactor body lowering callable `{}` 缺少 control-body step schema",
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
        let frame_layout = abi.frame_layout(abi_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor body lowering 缺少 callable `{}` 的 ABI frame layout s{}",
                callable.root_fqn(),
                abi_step_schema.as_u32()
            ))
        })?;
        let step_layout = abi.step_layout(abi_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor body lowering 缺少 callable `{}` 的 ABI step layout s{}",
                callable.root_fqn(),
                abi_step_schema.as_u32()
            ))
        })?;
        let slots = codegen.create_mir_local_slots(body, source_types)?;
        if let Ok(source_id) =
            codegen.materialized_mir_callable_source_id(callable.root_fqn(), mir_fun.span)
        {
            codegen.current_source_id = source_id;
        }
        let used_locals = super::super::mir_body::collect_mir_local_uses(body);
        let frame_root_slot =
            codegen.create_refactor_gc_root_slot(mir_fun.span, "refactor_frame_root")?;
        let mut state_blocks = BTreeMap::new();
        for state in callable.state_graph().states() {
            if state_blocks
                .insert(
                    state.state_id(),
                    codegen.context.append_basic_block(
                        function,
                        &format!("refactor.st{}", state.state_id().as_u32()),
                    ),
                )
                .is_some()
            {
                return Err(frontend_error(format!(
                    "refactor body verifier 发现 callable `{}` 重复发布 state st{}",
                    callable.root_fqn(),
                    state.state_id().as_u32()
                )));
            }
        }
        let emitter = Self {
            codegen,
            program,
            source_types,
            pass_view,
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
            return_mode: RefactorCallableReturnMode::Step,
        };
        emitter.verify_body_contract()?;
        Ok(emitter)
    }

    fn value_primitives(&mut self) -> RefactorValuePrimitives<'_, 'a, 'ctx> {
        RefactorValuePrimitives::new(
            &mut *self.codegen,
            self.source_types,
            self.body,
            &self.slots,
            self.abi,
        )
    }

    fn verify_body_contract(&self) -> Result<(), LlvmEmitError> {
        self.verify_state_graph_contract()?;
        self.verify_frame_contract()?;
        self.verify_boundary_contracts()?;
        Ok(())
    }

    fn verify_state_graph_contract(&self) -> Result<(), LlvmEmitError> {
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

    fn verify_state_source_slices(&self, state: &LateLoweredState) -> Result<(), LlvmEmitError> {
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

    fn verify_source_statement_classification(
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

    fn verify_state_terminator_contract(
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

    fn completion_payload_binding_feeds_return(
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

    fn verify_suspend_primary_boundary_contract(
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

    fn verify_resume_unwind_contract(&self, state: &LateLoweredState) -> Result<(), LlvmEmitError> {
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

    fn verify_resume_unwind_source(&self, state: &LateLoweredState) -> Result<(), LlvmEmitError> {
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

    fn resume_unwind_cleanup_origin(
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

    fn cleanup_route_reaches_resume_unwind(&self, start: StateId, target: StateId) -> bool {
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

    fn verify_resume_unwind_handle_contract(
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

    fn verify_abandon_contract(&self, state: &LateLoweredState) -> Result<(), LlvmEmitError> {
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

    fn verify_frame_contract(&self) -> Result<(), LlvmEmitError> {
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

    fn verify_boundary_contracts(&self) -> Result<(), LlvmEmitError> {
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

    fn verify_boundary_source_consumption(
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

    fn verify_step_case_payload_contract(
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

    fn verify_completion_payload_source(
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

    fn verify_operand_source(
        &self,
        source: &LateLoweredOperandSource,
    ) -> Result<(), LlvmEmitError> {
        self.verify_source_value_layout(source.source_ty(), "operand source")?;
        if let LateLoweredOperandValueSource::Local(local) = source.value() {
            self.verify_local_exists(*local, "operand source local")?;
        }
        Ok(())
    }

    fn verify_state_exists(&self, state_id: StateId, label: &str) -> Result<(), LlvmEmitError> {
        if self.state_blocks.contains_key(&state_id) {
            Ok(())
        } else {
            Err(frontend_error(format!(
                "refactor body verifier 发现 {label} 引用缺失 state st{}",
                state_id.as_u32()
            )))
        }
    }

    fn verify_boundary_exists(
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

    fn verify_local_exists(&self, local: LocalId, label: &str) -> Result<(), LlvmEmitError> {
        if self.slots.get(local.as_u32() as usize).is_some() {
            Ok(())
        } else {
            Err(frontend_error(format!(
                "refactor body verifier 发现 {label} 引用缺失 local l{}",
                local.as_u32()
            )))
        }
    }

    fn verify_source_value_layout(&self, ty: TypeId, label: &str) -> Result<(), LlvmEmitError> {
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

    fn source_ty_is_unit(&self, ty: TypeId) -> bool {
        matches!(
            self.source_types.kind(ty),
            TypeKind::Value(ValueTypeKind::Unit)
        )
    }

    fn source_ty_is_runtime_error(&self, ty: TypeId) -> bool {
        matches!(
            self.source_types.kind(ty),
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                | TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.RuntimeError"
        )
    }

    fn local_runtime_error_runtime_for_call(
        &self,
        site_id: SiteId,
        contract: &LateLoweredConsumedRuntimeErrorCase,
    ) -> Result<RefactorLocalRuntimeErrorRuntime, LlvmEmitError> {
        let published =
            self.abi
                .call_local_runtime_error_contract(self.abi_step_schema, site_id, contract)?;
        if published.owner_step_schema() != self.abi_step_schema || published.site_id() != site_id {
            return Err(frontend_error(format!(
                "refactor body verifier 发现 local runtime-error contract identity 漂移：layout=(s{}, site={}) expected=(s{}, site={})",
                published.owner_step_schema().as_u32(),
                published.site_id().as_u32(),
                self.abi_step_schema.as_u32(),
                site_id.as_u32()
            )));
        }
        if published.payload_abi().is_elided() {
            return Err(frontend_error(format!(
                "refactor body verifier 发现 call site {} 的 local runtime-error payload ABI 被错误 elide",
                site_id.as_u32()
            )));
        }
        let runtime_entry = match published.terminal_action() {
            RefactorLocalRuntimeErrorTerminalAction::RuntimeFatal { runtime_entry } => {
                runtime_entry
            }
        };
        Ok(RefactorLocalRuntimeErrorRuntime {
            site_id,
            input_case_tag: published.input_case_tag(),
            payload_tuple_ty: published.payload_tuple_ty(),
            target_state: published.target_state(),
            runtime_symbol: runtime_entry.symbol_name().to_string(),
            runtime_param_count: runtime_entry.param_count(),
        })
    }

    fn local_runtime_error_runtime_for_target_state(
        &self,
        target_state: StateId,
        payload_tuple_ty: TypeId,
        terminal_action: crate::effect_lowered::ir::LateLoweredLocalRuntimeErrorTerminalAction,
    ) -> Result<RefactorLocalRuntimeErrorRuntime, LlvmEmitError> {
        let mut selected = None::<RefactorLocalRuntimeErrorRuntime>;
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
                    "refactor body verifier 发现 LocalRuntimeError st{} 与 call site {} consumed contract 漂移",
                    target_state.as_u32(),
                    site_id.as_u32()
                )));
            }
            let runtime = self.local_runtime_error_runtime_for_call(site_id, contract)?;
            if let Some(existing) = &selected {
                return Err(frontend_error(format!(
                    "refactor body verifier 发现 LocalRuntimeError st{} 被多个 call site 消费：{} 与 {}",
                    target_state.as_u32(),
                    existing.site_id.as_u32(),
                    site_id.as_u32()
                )));
            }
            selected = Some(runtime);
        }
        selected.ok_or_else(|| {
            frontend_error(format!(
                "refactor body verifier 发现 LocalRuntimeError st{} 缺少对应 consumed runtime-error case contract",
                target_state.as_u32()
            ))
        })
    }

    fn lower_effect_neutral_statement(
        &mut self,
        stmt: &mir::Statement,
    ) -> Result<(), LlvmEmitError> {
        if self.is_builtin_string_concat_callee_statement(stmt) {
            return Ok(());
        }
        let codegen = &mut *self.codegen;
        let source_types = self.source_types;
        let body = self.body;
        let slots = &self.slots;
        let abi = self.abi;
        let used_locals = &self.used_locals;
        RefactorValuePrimitives::new(codegen, source_types, body, slots, abi)
            .lower_effect_neutral_statement(stmt, used_locals)
    }

    fn is_builtin_string_concat_callee_statement(&self, stmt: &mir::Statement) -> bool {
        let mir::StatementKind::Assign { target, value } = &stmt.kind else {
            return false;
        };
        let mir::Rvalue::MemberAccess { member, .. } = value else {
            return false;
        };
        if member.name != "concat"
            || self
                .codegen
                .cg_ty_of_mir_type(self.source_types, member.receiver_ty)
                != Some(CgTy::String)
        {
            return false;
        }
        self.body.blocks.iter().any(|block| {
            block.stmts.iter().any(|candidate| {
                matches!(
                    &candidate.kind,
                    mir::StatementKind::Assign {
                        value: mir::Rvalue::Call {
                            kind: mir::CallKind::FunValue { callee: mir::Operand::Local(local) },
                            ..
                        },
                        ..
                    } if local == target
                )
            })
        })
    }

    fn lower_published_call_statement(
        &mut self,
        stmt: &mir::Statement,
    ) -> Result<bool, LlvmEmitError> {
        let mir::StatementKind::Assign {
            target,
            value:
                mir::Rvalue::Call {
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
            mir::CallKind::Closure { .. }
                | mir::CallKind::FunValue { .. }
                | mir::CallKind::Virtual { .. }
                | mir::CallKind::Interface { .. }
        ) {
            return Ok(false);
        }
        if let Some(value) = self.lower_builtin_string_concat_call(stmt.span, kind, args)? {
            self.store_local_value(stmt.span, *target, value)?;
            return Ok(true);
        }
        if let Some(value) = self.lower_builtin_string_length_call(stmt.span, kind, args)? {
            self.store_local_value(stmt.span, *target, value)?;
            return Ok(true);
        }
        if let Some(value) = self.lower_builtin_to_string_call(stmt.span, kind, args)? {
            self.store_local_value(stmt.span, *target, value)?;
            return Ok(true);
        }
        let Some(layout) = self
            .abi
            .dynamic_invoke_layout(self.abi_step_schema, *site_id)
        else {
            let value = self
                .codegen
                .codegen_mir_refactor_plain_dynamic_call(
                    stmt.span,
                    kind,
                    args,
                    self.body,
                    self.source_types,
                    &self.slots,
                )
                .map_err(|err| {
                    frontend_error(format!(
                        "refactor source-slice dynamic call site {} 缺少 published dynamic-invoke contract，且 plain callable lowering 失败: {err}",
                        site_id.as_u32(),
                    ))
                })?;
            self.store_local_value(stmt.span, *target, value)?;
            return Ok(true);
        };
        let args_payload = self.pack_call_args_for_invoke(
            stmt.span,
            layout.invoke_args_tuple_ty(),
            args,
            "refactor_dynamic_call",
        )?;
        let carrier = self.lower_dynamic_call_carrier(stmt.span, kind, layout)?;
        let step = self.emit_refactor_dynamic_invoke_step(layout, carrier, args_payload)?;
        self.store_no_outward_call_complete(
            stmt.span,
            *site_id,
            layout.return_step_schema(),
            step,
            *target,
        )?;
        Ok(true)
    }

    fn lower_builtin_string_concat_call(
        &mut self,
        span: crate::span::Span,
        kind: &mir::CallKind,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let mir::CallKind::FunValue { callee } = kind else {
            return Ok(None);
        };
        let Some(receiver) = self.string_member_receiver(callee, "concat") else {
            return Ok(None);
        };
        if args.len() != 1 || args[0].name.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor builtin String.concat args",
                at: span.into(),
            });
        }
        let receiver = self.codegen.codegen_mir_operand_expected(
            span,
            &receiver,
            &self.slots,
            Some(CgTy::String),
        )?;
        let receiver = self.codegen.coerce_value(span, receiver, CgTy::String)?;
        let Some(BasicValueEnum::PointerValue(receiver_ptr)) = receiver.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor builtin String.concat receiver value",
                at: span.into(),
            });
        };
        let arg = &args[0];
        let arg_value = self.codegen.codegen_mir_operand_expected(
            arg.span,
            &arg.value,
            &self.slots,
            Some(CgTy::String),
        )?;
        let arg_value = self
            .codegen
            .coerce_value(arg.span, arg_value, CgTy::String)?;
        let Some(BasicValueEnum::PointerValue(arg_ptr)) = arg_value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor builtin String.concat arg value",
                at: arg.span.into(),
            });
        };
        let runtime = self.codegen.declare_runtime_string_concat();
        let call = self.codegen.build_call_preserving_gc_local_roots(
            span,
            runtime,
            &[receiver_ptr.into(), arg_ptr.into()],
            "refactor_string_concat",
        )?;
        self.string_result_from_runtime_call(span, call, "String.concat")
            .map(Some)
    }

    fn lower_builtin_string_length_call(
        &mut self,
        span: crate::span::Span,
        kind: &mir::CallKind,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let mir::CallKind::FunValue { callee } = kind else {
            return Ok(None);
        };
        let Some(receiver) = self.string_member_receiver(callee, "length") else {
            return Ok(None);
        };
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor builtin String.length args",
                at: span.into(),
            });
        }
        let receiver = self.codegen.codegen_mir_operand_expected(
            span,
            &receiver,
            &self.slots,
            Some(CgTy::String),
        )?;
        let receiver = self.codegen.coerce_value(span, receiver, CgTy::String)?;
        let Some(BasicValueEnum::PointerValue(receiver_ptr)) = receiver.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor builtin String.length receiver value",
                at: span.into(),
            });
        };
        let runtime = self.codegen.declare_runtime_string_length();
        let call = self.codegen.build_call_preserving_gc_local_roots(
            span,
            runtime,
            &[receiver_ptr.into()],
            "refactor_string_length",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor builtin String.length return value",
                at: span.into(),
            })?;
        Ok(Some(self.codegen.cg_value_from_loaded(
            span,
            CgTy::Int(IntTy {
                bits: self.codegen.host.word_bit_width(),
                signed: true,
            }),
            raw,
        )?))
    }

    fn string_member_receiver(
        &self,
        callee: &mir::Operand,
        member_name: &str,
    ) -> Option<mir::Operand> {
        let mir::Operand::Local(callee_local) = callee else {
            return None;
        };
        self.body.blocks.iter().find_map(|block| {
            block.stmts.iter().find_map(|stmt| {
                let mir::StatementKind::Assign { target, value } = &stmt.kind else {
                    return None;
                };
                if target != callee_local {
                    return None;
                }
                let mir::Rvalue::MemberAccess {
                    receiver, member, ..
                } = value
                else {
                    return None;
                };
                (member.name == member_name
                    && self
                        .codegen
                        .cg_ty_of_mir_type(self.source_types, member.receiver_ty)
                        == Some(CgTy::String))
                .then_some(receiver.clone())
            })
        })
    }

    fn lower_builtin_to_string_call(
        &mut self,
        span: crate::span::Span,
        kind: &mir::CallKind,
        args: &[mir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let mir::CallKind::Interface { receiver, dispatch } = kind else {
            return Ok(None);
        };
        if dispatch.owner_fqn != "scoop.core.ToString" || dispatch.member_name != "toString" {
            return Ok(None);
        }
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor builtin ToString.toString args",
                at: span.into(),
            });
        }
        let receiver_cg = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, dispatch.receiver_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor builtin ToString receiver type",
                at: span.into(),
            })?;
        let value = self.codegen.codegen_mir_operand_expected(
            span,
            receiver,
            &self.slots,
            Some(receiver_cg),
        )?;
        let value = self.codegen.coerce_value(span, value, receiver_cg)?;
        match receiver_cg {
            CgTy::String => Ok(Some(value)),
            CgTy::Bool => {
                let Some(BasicValueEnum::IntValue(raw)) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor builtin ToString Bool value",
                        at: span.into(),
                    });
                };
                let widened = self.codegen.builder.build_int_z_extend(
                    raw,
                    self.codegen.context.i64_type(),
                    "refactor_bool_to_string_arg",
                )?;
                let runtime = self.codegen.declare_runtime_bool_to_string();
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    runtime,
                    &[widened.into()],
                    "refactor_bool_to_string",
                )?;
                self.string_result_from_runtime_call(span, call, "Bool")
                    .map(Some)
            }
            CgTy::Int(_) => {
                let Some(BasicValueEnum::IntValue(raw)) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor builtin ToString Int value",
                        at: span.into(),
                    });
                };
                let runtime = self.codegen.declare_runtime_int_to_string();
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    runtime,
                    &[raw.into()],
                    "refactor_int_to_string",
                )?;
                self.string_result_from_runtime_call(span, call, "Int")
                    .map(Some)
            }
            CgTy::Float64 | CgTy::Float32 => {
                let Some(BasicValueEnum::FloatValue(raw)) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor builtin ToString Float value",
                        at: span.into(),
                    });
                };
                let runtime = match receiver_cg {
                    CgTy::Float64 => self.codegen.declare_runtime_float64_to_string(),
                    CgTy::Float32 => self.codegen.declare_runtime_float32_to_string(),
                    _ => unreachable!("receiver_cg matched float above"),
                };
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    span,
                    runtime,
                    &[raw.into()],
                    "refactor_float_to_string",
                )?;
                self.string_result_from_runtime_call(span, call, "Float")
                    .map(Some)
            }
            _ => Ok(None),
        }
    }

    fn string_result_from_runtime_call(
        &self,
        span: crate::span::Span,
        call: inkwell::values::CallSiteValue<'ctx>,
        label: &'static str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor builtin ToString runtime ret",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(str_ptr) = ret else {
            return Err(frontend_error(format!(
                "refactor builtin ToString {label} runtime ret type mismatch"
            )));
        };
        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    fn load_local_value(
        &mut self,
        span: crate::span::Span,
        local: LocalId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.value_primitives().load_local(span, local)
    }

    fn store_local_value(
        &mut self,
        span: crate::span::Span,
        local: LocalId,
        value: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.value_primitives().store_local(span, local, value)
    }

    fn store_loaded_raw_local(
        &mut self,
        span: crate::span::Span,
        local: LocalId,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.value_primitives()
            .store_loaded_raw_local(span, local, raw)
    }

    fn initialize_new_frame(&mut self) -> Result<(), LlvmEmitError> {
        let frame_ptr = self.codegen.refactor_alloc_gc_struct(
            self.mir_fun.span,
            self.frame_layout.llvm_ty(),
            self.frame_layout.layout_anchor_name(),
            "refactor_frame",
        )?;
        self.store_frame_root(frame_ptr)
    }

    fn store_frame_root(&mut self, frame_ptr: PointerValue<'ctx>) -> Result<(), LlvmEmitError> {
        let frame_gc = self.codegen.refactor_cast_ptr(
            frame_ptr,
            self.codegen.llvm_gc_i8_ptr_type(),
            "refactor_frame_root_value",
        )?;
        self.codegen.store_refactor_gc_root_slot(
            self.mir_fun.span,
            self.frame_root_slot,
            frame_gc,
            "refactor_frame_root",
        )?;
        Ok(())
    }

    fn clear_frame_root(&mut self) -> Result<(), LlvmEmitError> {
        let null = self.codegen.llvm_gc_i8_ptr_type().const_null();
        self.codegen.store_refactor_gc_root_slot(
            self.mir_fun.span,
            self.frame_root_slot,
            null,
            "refactor_frame_root",
        )
    }

    fn release_frame_root_for_frame_free_tail(
        &mut self,
        resume_state: StateId,
    ) -> Result<(), LlvmEmitError> {
        if !matches!(self.return_mode, RefactorCallableReturnMode::Plain { .. })
            || self.callable.needs_reentry()
            || self.return_projection.is_some()
            || self.surface_resume_handle_sites.is_some()
            || !self.reachable_tail_is_frame_free(resume_state)
        {
            return Ok(());
        }
        self.clear_frame_root()
    }

    fn reachable_tail_is_frame_free(&self, start: StateId) -> bool {
        let mut seen = BTreeSet::new();
        let mut stack = vec![start];
        while let Some(state_id) = stack.pop() {
            if !seen.insert(state_id) {
                continue;
            }
            let Some(state) = self.callable.state_graph().state(state_id) else {
                return false;
            };
            if matches!(state.role(), LateLoweredStateRole::Cleanup) {
                return false;
            }
            if matches!(
                state.terminator(),
                LateLoweredStateTerminator::Suspend { .. }
                    | LateLoweredStateTerminator::HandleDispatch { .. }
            ) {
                return false;
            }
            stack.extend(state.successors().iter().copied());
        }
        true
    }

    fn current_frame_ptr(&mut self) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let frame_gc = self
            .codegen
            .load_refactor_gc_root_slot(self.frame_root_slot, "refactor_frame_root")?;
        self.codegen.refactor_cast_ptr(
            frame_gc,
            self.codegen.llvm_ptr_type(self.codegen.gc_address_space()),
            "refactor_frame_current",
        )
    }

    fn root_gc_pointer(
        &mut self,
        value: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let slot = self
            .codegen
            .create_refactor_gc_root_slot(self.mir_fun.span, name)?;
        let value = self.codegen.refactor_cast_ptr(
            value,
            self.codegen.llvm_gc_i8_ptr_type(),
            &format!("{name}_value"),
        )?;
        self.codegen
            .store_refactor_gc_root_slot(self.mir_fun.span, slot, value, name)?;
        self.codegen.load_refactor_gc_root_slot(slot, name)
    }

    fn emit_direct(
        mut self,
        entry_layout: &RefactorCallableEntryLayout<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        self.bind_direct_args(entry_layout).map_err(|err| {
            frontend_error(format!(
                "refactor direct entry `{}` bind args failed: {err}",
                entry_layout.symbol_name()
            ))
        })?;
        self.initialize_new_frame()?;
        let entry_state = self.callable.state_graph().entry_state();
        self.branch_to_state(entry_state)?;
        self.emit_states()
    }

    fn emit_plain_direct(
        mut self,
        hir_fun: &crate::hir::FunDecl,
        param_offset: u32,
        declared_return_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        self.return_mode = RefactorCallableReturnMode::Plain { declared_return_cg };
        self.codegen.bind_mir_params(
            hir_fun,
            self.mir_fun,
            self.function,
            param_offset,
            &mut self.slots,
        )?;
        self.initialize_new_frame()?;
        let entry_state = self.callable.state_graph().entry_state();
        self.branch_to_state(entry_state)?;
        self.emit_states()
    }

    fn emit_plain_direct_mir_params(
        mut self,
        param_offset: u32,
        declared_return_cg: CgTy,
    ) -> Result<(), LlvmEmitError> {
        self.return_mode = RefactorCallableReturnMode::Plain { declared_return_cg };
        self.codegen.bind_mir_params_without_hir(
            self.mir_fun,
            self.function,
            param_offset,
            &mut self.slots,
        )?;
        self.initialize_new_frame()?;
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
        let cont_ptr = self.root_gc_pointer(cont_ptr, "refactor_resume_cont_root")?;
        let captured_frame = self.load_frame_from_continuation(cont_ptr)?;
        self.store_frame_root(captured_frame)?;
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
        self.emit_double_resume_runtime_error(resume_state_tag)?;

        self.codegen.builder.position_at_end(first_resume_bb);
        self.store_continuation_one_shot(cont_ptr, true)?;
        let composed_callee = self.load_composed_callee_continuation(cont_ptr)?;
        let composed_is_null = self
            .codegen
            .builder
            .build_is_null(composed_callee, "refactor_composed_callee_is_null")?;
        let ordinary_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_plain_dispatch");
        let composed_resume_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_composed_dispatch");
        self.codegen.builder.build_conditional_branch(
            composed_is_null,
            ordinary_resume_bb,
            composed_resume_bb,
        )?;

        self.codegen.builder.position_at_end(composed_resume_bb);
        self.dispatch_composed_call_boundary_resume(
            resume_state_tag,
            composed_callee,
            resume_tuple_ty,
            payload,
        )?;

        self.codegen.builder.position_at_end(ordinary_resume_bb);
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
            let _ = self
                .abi
                .resume_payload_binding_for_state(self.abi_step_schema, binding.resume_state())?;
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

    fn emit_double_resume_runtime_error(
        &mut self,
        resume_state_tag: IntValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let (case_tag, payload_tuple_ty) = self.double_resume_runtime_error_case()?;
        let payload = self.lower_runtime_error_boundary_payload(payload_tuple_ty)?;
        let continuation =
            self.create_continuation_object_with_state_tag(None, resume_state_tag, None, None)?;
        let out_layout = self.step_layout.case_layout(case_tag).ok_or_else(|| {
            frontend_error(format!(
                "refactor callable `{}` step schema s{} 缺少 double resume runtime error case c{}",
                self.callable.root_fqn(),
                self.abi_step_schema.as_u32(),
                case_tag.as_u32(),
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

    fn double_resume_runtime_error_case(&self) -> Result<(CaseTag, TypeId), LlvmEmitError> {
        let mut selected = None::<(CaseTag, TypeId)>;
        for boundary in self.callable.boundary_map().entries() {
            let Some(LateLoweredBoundaryLowering::RuntimeError(lowering)) = boundary.lowering()
            else {
                continue;
            };
            let emission = lowering.emitted_step().clone();
            if self.step_layout.case_layout(emission.case_tag()).is_none() {
                continue;
            }
            let candidate = (emission.case_tag(), emission.payload_tuple_ty());
            match &selected {
                Some(existing) if existing != &candidate => {
                    return Err(frontend_error(format!(
                        "refactor callable `{}` 存在多义 double resume runtime error emission：{:?} 与 {:?}",
                        self.callable.root_fqn(),
                        existing,
                        candidate,
                    )));
                }
                Some(_) => {}
                None => selected = Some(candidate),
            }
        }
        if selected.is_none() {
            for (case_tag, case_layout) in self.step_layout.cases() {
                let payload_ty = case_layout.variant().payload_source_ty();
                if !self.source_ty_is_runtime_error(payload_ty) {
                    continue;
                }
                let candidate = (*case_tag, payload_ty);
                match &selected {
                    Some(existing) if existing != &candidate => {
                        return Err(frontend_error(format!(
                            "refactor callable `{}` 存在多义 double resume runtime error Step case：{:?} 与 {:?}",
                            self.callable.root_fqn(),
                            existing,
                            candidate,
                        )));
                    }
                    Some(_) => {}
                    None => selected = Some(candidate),
                }
            }
        }
        selected.ok_or_else(|| {
            frontend_error(format!(
                "refactor callable `{}` 缺少 double resume 可用的 ordinary runtime error boundary emission",
                self.callable.root_fqn(),
            ))
        })
    }

    fn dispatch_composed_call_boundary_resume(
        &mut self,
        resume_state_tag: IntValue<'ctx>,
        callee_continuation: PointerValue<'ctx>,
        resume_tuple_ty: TypeId,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        let mut composition_entries = Vec::new();
        for boundary in self.callable.boundary_map().entries() {
            match boundary.lowering() {
                Some(LateLoweredBoundaryLowering::Call(lowering)) => {
                    for composition in
                        lowering
                            .continuation_compositions()
                            .iter()
                            .filter(|composition| {
                                composition.caller_continuation_contract().resume_tuple_ty()
                                    == resume_tuple_ty
                            })
                    {
                        composition_entries.push((
                            boundary.clone(),
                            Some(lowering.clone()),
                            lowering.dispatch().clone(),
                            lowering.continuation_compositions().to_vec(),
                            composition.clone(),
                        ));
                    }
                }
                Some(LateLoweredBoundaryLowering::Resume(lowering)) => {
                    for composition in
                        lowering
                            .continuation_compositions()
                            .iter()
                            .filter(|composition| {
                                composition.caller_continuation_contract().resume_tuple_ty()
                                    == resume_tuple_ty
                            })
                    {
                        composition_entries.push((
                            boundary.clone(),
                            None,
                            lowering.dispatch().clone(),
                            lowering.continuation_compositions().to_vec(),
                            composition.clone(),
                        ));
                    }
                }
                _ => {}
            }
        }
        if composition_entries.is_empty() {
            self.codegen.builder.build_unreachable()?;
            return Ok(());
        }
        let invalid_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "resume_composed_invalid_state");
        let mut cases = Vec::with_capacity(composition_entries.len());
        let mut seen_states = BTreeMap::new();
        for (boundary, call_lowering, dispatch, compositions, composition) in composition_entries {
            let resume_state = composition.caller_resume_state();
            let candidate = (
                boundary.boundary_id(),
                composition.callee_continuation_schema(),
                composition.input_step_schema(),
            );
            if let Some(existing) = seen_states.get(&resume_state) {
                if *existing != candidate {
                    return Err(frontend_error(format!(
                        "refactor callable `{}` resume state st{} 存在多义 call-boundary continuation composition origin：{:?} 与 {:?}",
                        self.callable.root_fqn(),
                        resume_state.as_u32(),
                        existing,
                        candidate,
                    )));
                }
                continue;
            }
            seen_states.insert(resume_state, candidate);
            let bb = self.codegen.context.append_basic_block(
                self.function,
                &format!(
                    "resume_composed_bd{}_case{}",
                    boundary.boundary_id().as_u32(),
                    composition.input_case_tag().as_u32(),
                ),
            );
            cases.push((
                self.codegen
                    .context
                    .i32_type()
                    .const_int(composition.caller_resume_state().as_u32() as u64, false),
                bb,
                boundary,
                call_lowering,
                dispatch,
                compositions,
                composition,
            ));
        }
        let switch_cases = cases
            .iter()
            .map(|(tag, bb, _, _, _, _, _)| (*tag, *bb))
            .collect::<Vec<_>>();
        self.codegen
            .builder
            .build_switch(resume_state_tag, invalid_bb, &switch_cases)?;

        for (_, bb, boundary, call_lowering, dispatch, compositions, composition) in cases {
            self.codegen.builder.position_at_end(bb);
            let dispatch_context = ComposedBoundaryDispatchContext {
                call_lowering: call_lowering.as_ref(),
                dispatch: &dispatch,
                continuation_compositions: &compositions,
            };
            self.resume_composed_call_boundary_case(
                &boundary,
                dispatch_context,
                &composition,
                callee_continuation,
                resume_tuple_ty,
                payload,
            )?;
        }

        self.codegen.builder.position_at_end(invalid_bb);
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    fn resume_composed_call_boundary_case(
        &mut self,
        boundary: &LateLoweredBoundary,
        dispatch_context: ComposedBoundaryDispatchContext<'_>,
        composition: &LateLoweredCallBoundaryContinuationComposition,
        callee_continuation: PointerValue<'ctx>,
        resume_tuple_ty: TypeId,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        if composition.boundary_id() != boundary.boundary_id()
            || composition.caller_resume_state() != boundary.resume_state()
        {
            return Err(frontend_error(format!(
                "refactor composed call boundary bd{} continuation composition 与 boundary resume state 漂移：composition={:?}",
                boundary.boundary_id().as_u32(),
                composition,
            )));
        }
        let surface = self
            .abi
            .surface_resume_layout(composition.callee_continuation_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor composed call boundary bd{} 缺少 callee continuation schema k{} surface ABI",
                    boundary.boundary_id().as_u32(),
                    composition.callee_continuation_schema().as_u32(),
                ))
            })?;
        if surface.resume_tuple_ty() != resume_tuple_ty {
            return Err(frontend_error(format!(
                "refactor composed call boundary bd{} callee surface ABI 漂移：surface_resume=t{} surface_out=s{} composition_resume=t{} composition_out=s{}",
                boundary.boundary_id().as_u32(),
                surface.resume_tuple_ty().as_u32(),
                surface.return_step_schema().as_u32(),
                resume_tuple_ty.as_u32(),
                composition.input_step_schema().as_u32(),
            )));
        }
        if let Some(call_lowering) = dispatch_context.call_lowering
            && self
                .call_boundary_prefix_replay_matches_prior_resuming_route(boundary, call_lowering)?
            && !self.call_boundary_tail_has_later_resuming_boundary(boundary, call_lowering)?
        {
            self.replay_call_boundary_prefix(boundary, call_lowering)?;
        }
        let callee = self.codegen.refactor_function(surface.symbol_name())?;
        let mut args = vec![callee_continuation.into()];
        if surface.param_count() > 1 {
            args.push(
                payload
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor composed call boundary bd{} callee resume 需要 non-elided payload",
                            boundary.boundary_id().as_u32(),
                        ))
                    })?
                    .into(),
            );
        }
        let call = self.codegen.build_call_preserving_gc_local_roots(
            self.mir_fun.span,
            callee,
            &args,
            "refactor_composed_callee_resume",
        )?;
        let step = call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error(
                "refactor composed call boundary callee resume 未返回 Step_F".to_string(),
            )
        })?;
        self.dispatch_boundary_step(
            boundary,
            surface.return_step_schema(),
            step,
            dispatch_context.dispatch,
            dispatch_context.call_lowering,
            Some(dispatch_context.continuation_compositions),
        )
    }

    fn call_boundary_prefix_replay_matches_prior_resuming_route(
        &self,
        boundary: &LateLoweredBoundary,
        lowering: &LateLoweredCallBoundaryLowering,
    ) -> Result<bool, LlvmEmitError> {
        let output_cases = lowering
            .dispatch()
            .outward_cases()
            .iter()
            .map(|case| case.emission().case_tag())
            .collect::<BTreeSet<_>>();
        if output_cases.is_empty() {
            return Ok(false);
        }

        for state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch { contract, .. } = state.terminator()
            else {
                continue;
            };
            if contract.boundary_routing(boundary.boundary_id()).is_none() {
                continue;
            }

            let matched_cases = contract
                .boundary_routings()
                .iter()
                .filter(|routing| routing.boundary_id() != boundary.boundary_id())
                .filter(|routing| routing.resume_state() == boundary.owner_state())
                .flat_map(|routing| routing.case_routings())
                .filter(|case| output_cases.contains(&case.case_tag()))
                .filter(|case| {
                    matches!(
                        case.action(),
                        LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm { .. }
                    ) && contract
                        .handled_arm(case.case_tag())
                        .and_then(|arm| arm.continuation_binder())
                        .is_some()
                })
                .map(|case| case.case_tag())
                .collect::<BTreeSet<_>>();

            if output_cases.iter().all(|case| matched_cases.contains(case)) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn call_boundary_tail_has_later_resuming_boundary(
        &self,
        boundary: &LateLoweredBoundary,
        lowering: &LateLoweredCallBoundaryLowering,
    ) -> Result<bool, LlvmEmitError> {
        let LateLoweredBoundarySourceConsumption::Statement {
            source_slice,
            statement_index,
            ..
        } = lowering.operand_contract().source_consumption()
        else {
            return Ok(false);
        };
        for candidate in self.callable.boundary_map().entries() {
            if candidate.boundary_id() == boundary.boundary_id() {
                continue;
            }
            let Some(LateLoweredBoundaryLowering::Perform(perform)) = candidate.lowering() else {
                continue;
            };
            let candidate_consumption = perform.operand_contract().source_consumption();
            if candidate_consumption.source_slice().block_id() != source_slice.block_id() {
                continue;
            }
            let starts_after_call = match candidate_consumption {
                LateLoweredBoundarySourceConsumption::Statement {
                    statement_index: candidate_index,
                    ..
                } => candidate_index > statement_index,
                LateLoweredBoundarySourceConsumption::Terminator {
                    source_slice: candidate_slice,
                } => candidate_slice.start_statement_index() >= source_slice.end_statement_index(),
            };
            if !starts_after_call {
                continue;
            }
            let Some(RefactorHandleBoundaryRuntimeAction::ConsumeToArm(action)) = self
                .handle_boundary_action(
                    candidate.boundary_id(),
                    perform.emitted_step().case_tag(),
                )?
            else {
                continue;
            };
            if action.continuation_binder.is_some() {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn replay_call_boundary_prefix(
        &mut self,
        boundary: &LateLoweredBoundary,
        lowering: &LateLoweredCallBoundaryLowering,
    ) -> Result<(), LlvmEmitError> {
        let Some(owner_state) = self.callable.state_graph().state(boundary.owner_state()) else {
            return Err(frontend_error(format!(
                "refactor composed call replay bd{} 缺少 owner state st{}",
                boundary.boundary_id().as_u32(),
                boundary.owner_state().as_u32(),
            )));
        };
        if !matches!(owner_state.role(), LateLoweredStateRole::Resume) {
            return Ok(());
        }
        let LateLoweredBoundarySourceConsumption::Statement {
            source_slice,
            statement_index,
            ..
        } = lowering.operand_contract().source_consumption()
        else {
            return Ok(());
        };
        if source_slice.start_statement_index() != 0 {
            return Ok(());
        }
        let block = self
            .body
            .blocks
            .get(source_slice.block_id().as_u32() as usize)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor composed call replay block",
                at: self.mir_fun.span.into(),
            })?;
        for stmt_index in source_slice.start_statement_index()..statement_index {
            let stmt =
                block
                    .stmts
                    .get(stmt_index as usize)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor composed call replay statement",
                        at: self.mir_fun.span.into(),
                    })?;
            let classification = self
                .callable
                .source_statement_classification(source_slice, stmt_index)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor composed call replay bb{} stmt{} 缺少 published classification",
                        source_slice.block_id().as_u32(),
                        stmt_index,
                    ))
                })?;
            match classification.kind() {
                LateLoweredSourceStatementClassificationKind::EffectNeutralValue
                | LateLoweredSourceStatementClassificationKind::BoundaryResultInjection {
                    ..
                }
                | LateLoweredSourceStatementClassificationKind::CompletionPayloadInjection {
                    ..
                } => {
                    if !self.lower_published_call_statement(stmt)? {
                        self.lower_effect_neutral_statement(stmt)?;
                    }
                }
                LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor { .. }
                | LateLoweredSourceStatementClassificationKind::ResumePayloadInjection { .. }
                | LateLoweredSourceStatementClassificationKind::HandleSyntheticCarrierBinder {
                    ..
                }
                | LateLoweredSourceStatementClassificationKind::ElidedUnreachable => {}
                LateLoweredSourceStatementClassificationKind::Unsupported { reason } => {
                    return Err(frontend_error(format!(
                        "refactor composed call replay bb{} stmt{} classified unsupported: {reason}",
                        source_slice.block_id().as_u32(),
                        stmt_index,
                    )));
                }
            }
        }
        Ok(())
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
        Ok(slot.cg_ty == resume_cg || self.is_task_transport_tuple_ty(resume_tuple_ty)?)
    }

    fn is_task_transport_tuple_ty(&self, ty: TypeId) -> Result<bool, LlvmEmitError> {
        let Some(codegen_ty) = self
            .codegen
            .equivalent_codegen_type_id(self.source_types, ty)
        else {
            return Ok(false);
        };
        Ok(self.codegen.is_task_transport_tuple_ty(codegen_ty))
    }

    fn emit_states(&mut self) -> Result<(), LlvmEmitError> {
        for state in self.callable.state_graph().states() {
            let bb = self.state_block(state.state_id())?;
            self.codegen.builder.position_at_end(bb);
            self.lower_state_source_slices(state).map_err(|err| {
                frontend_error(format!(
                    "refactor callable `{}` state st{} source-slice lowering failed: {err}",
                    self.callable.root_fqn(),
                    state.state_id().as_u32()
                ))
            })?;
            self.lower_state_terminator(state).map_err(|err| {
                frontend_error(format!(
                    "refactor callable `{}` step schema s{} (ABI s{}) state st{} terminator lowering failed: {err}",
                    self.callable.root_fqn(),
                    self.callable.step_schema().as_u32(),
                    self.abi_step_schema.as_u32(),
                    state.state_id().as_u32()
                ))
            })?;
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
        let lambda_env_component_count = self.lambda_env_component_count(entry_layout)?;
        for (index, param) in self.mir_fun.params.iter().enumerate() {
            let param_cg = self
                .codegen
                .cg_ty_of_mir_type(self.source_types, param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor direct param type",
                    at: param.span.into(),
                })?;
            let value = if let Some(env_component_count) = lambda_env_component_count {
                if index == 0 {
                    if env_component_count == 0 {
                        self.codegen.default_value(param.span, param_cg)?
                    } else if self.mir_fun.params.len() == 1
                        && param.ty == entry_layout.invoke_args_tuple_ty()
                        && matches!(param_cg, CgTy::Tuple(_))
                    {
                        self.bind_direct_tuple_param_from_components(
                            entry_layout.symbol_name(),
                            param.span,
                            param.ty,
                            param_cg,
                            args_layout,
                            raw_arg,
                            0,
                        )?
                    } else {
                        self.bind_direct_param_from_component(
                            entry_layout.symbol_name(),
                            param.span,
                            param_cg,
                            args_layout,
                            raw_arg,
                            0,
                        )?
                    }
                } else {
                    self.bind_direct_param_from_component(
                        entry_layout.symbol_name(),
                        param.span,
                        param_cg,
                        args_layout,
                        raw_arg,
                        env_component_count + index - 1,
                    )?
                }
            } else if self.mir_fun.params.len() == 1
                && param.ty == entry_layout.invoke_args_tuple_ty()
                && matches!(param_cg, CgTy::Tuple(_))
            {
                self.bind_direct_tuple_param_from_components(
                    entry_layout.symbol_name(),
                    param.span,
                    param.ty,
                    param_cg,
                    args_layout,
                    raw_arg,
                    0,
                )?
            } else {
                self.bind_direct_param_from_component(
                    entry_layout.symbol_name(),
                    param.span,
                    param_cg,
                    args_layout,
                    raw_arg,
                    index,
                )?
            };
            let _ = self.store_local_value(param.span, param.local, value)?;
        }
        Ok(())
    }

    fn lambda_env_component_count(
        &self,
        entry_layout: &RefactorCallableEntryLayout<'ctx>,
    ) -> Result<Option<usize>, LlvmEmitError> {
        if !self.mir_fun.name.starts_with("$lambda") {
            return Ok(None);
        }
        let Some(env_param) = self.mir_fun.params.first() else {
            return Ok(None);
        };
        match self.source_types.kind(env_param.ty) {
            TypeKind::Value(ValueTypeKind::Unit) => Ok(Some(0)),
            TypeKind::Value(ValueTypeKind::Tuple(elements))
                if self.mir_fun.params.len() == 1
                    && env_param.ty == entry_layout.invoke_args_tuple_ty() =>
            {
                Ok(Some(elements.len()))
            }
            TypeKind::Value(ValueTypeKind::Tuple(_)) => Ok(Some(1)),
            _ => Err(frontend_error(format!(
                "refactor direct entry `{}` 的 lambda env 参数不是 Unit 或 tuple",
                self.mir_fun.fqn,
            ))),
        }
    }

    fn bind_direct_param_from_component(
        &mut self,
        entry_symbol: &str,
        span: crate::span::Span,
        param_cg: CgTy,
        args_layout: &RefactorSourceAbiLayout<'ctx>,
        raw_arg: Option<BasicValueEnum<'ctx>>,
        source_index: usize,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match self.extract_direct_arg_component(entry_symbol, args_layout, raw_arg, source_index)? {
            Some(raw) => self.codegen.cg_value_from_loaded(span, param_cg, raw),
            None => self.codegen.default_value(span, param_cg),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn bind_direct_tuple_param_from_components(
        &mut self,
        entry_symbol: &str,
        span: crate::span::Span,
        tuple_ty: TypeId,
        tuple_cg: CgTy,
        args_layout: &RefactorSourceAbiLayout<'ctx>,
        raw_arg: Option<BasicValueEnum<'ctx>>,
        source_start: usize,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.source_types.kind(tuple_ty)
        else {
            return Err(frontend_error(format!(
                "refactor direct entry `{entry_symbol}` 不能从 components 组装非 tuple 参数 t{}",
                tuple_ty.as_u32(),
            )));
        };
        let BasicTypeEnum::StructType(tuple_struct_ty) =
            self.codegen.llvm_basic_type_of(span, tuple_cg)?
        else {
            return Err(frontend_error(format!(
                "refactor direct entry `{entry_symbol}` tuple 参数 t{} 的 LLVM type 不是 struct",
                tuple_ty.as_u32(),
            )));
        };
        let mut aggregate = tuple_struct_ty.get_undef();
        for (offset, elem_ty) in elements.iter().enumerate() {
            let elem_cg = self
                .codegen
                .cg_ty_of_mir_type(self.source_types, *elem_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor direct tuple param element type",
                    at: span.into(),
                })?;
            let raw = match self.extract_direct_arg_component(
                entry_symbol,
                args_layout,
                raw_arg,
                source_start + offset,
            )? {
                Some(raw) => raw,
                None => {
                    let llvm_ty = self.codegen.llvm_basic_type_of(span, elem_cg)?;
                    self.codegen.zero_initializer_for_basic_type(llvm_ty)
                }
            };
            aggregate = self
                .codegen
                .builder
                .build_insert_value(
                    aggregate,
                    raw,
                    offset as u32,
                    &format!("refactor_direct_tuple_param{source_start}_{offset}"),
                )?
                .into_struct_value();
        }
        self.codegen
            .cg_value_from_loaded(span, tuple_cg, aggregate.into())
    }

    fn extract_direct_arg_component(
        &mut self,
        entry_symbol: &str,
        args_layout: &RefactorSourceAbiLayout<'ctx>,
        raw_arg: Option<BasicValueEnum<'ctx>>,
        source_index: usize,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        match args_layout.kind() {
            RefactorSourceAbiLayoutKind::Scalar if source_index == 0 => {
                if args_layout.abi().is_elided() {
                    Ok(None)
                } else {
                    raw_arg.map(Some).ok_or_else(|| {
                        frontend_error(format!(
                            "refactor direct entry `{entry_symbol}` scalar args ABI 缺少 raw 参数"
                        ))
                    })
                }
            }
            RefactorSourceAbiLayoutKind::Scalar => Err(frontend_error(format!(
                "refactor direct entry `{entry_symbol}` scalar args ABI 不能绑定 source component {source_index}；不能用默认值掩盖 contract 漂移"
            ))),
            RefactorSourceAbiLayoutKind::Tuple => {
                let field = args_layout.field(source_index).ok_or_else(|| {
                    frontend_error(format!(
                        "refactor direct entry `{entry_symbol}` args tuple ABI 缺少 source component {source_index} 的 field；不能用默认值掩盖 contract 漂移"
                    ))
                })?;
                if field.is_elided() {
                    return Ok(None);
                }
                let tuple = raw_arg.ok_or_else(|| {
                    frontend_error(format!(
                        "refactor direct entry `{entry_symbol}` args tuple ABI 缺少 raw 参数"
                    ))
                })?;
                let struct_value = tuple.into_struct_value();
                let raw = self.codegen.builder.build_extract_value(
                    struct_value,
                    field
                        .abi_field_index()
                        .expect("non-elided field has ABI index"),
                    &format!("refactor_arg_field{source_index}"),
                )?;
                Ok(Some(raw))
            }
        }
    }

    fn lower_state_source_slices(&mut self, state: &LateLoweredState) -> Result<(), LlvmEmitError> {
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
                let stmt = block.stmts.get(stmt_index as usize).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor source slice statement",
                        at: self.mir_fun.span.into(),
                    },
                )?;
                let classification = self
                    .callable
                    .source_statement_classification(*slice, stmt_index)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor source-slice statement bb{} stmt{} 缺少 published classification",
                            slice.block_id().as_u32(),
                            stmt_index,
                        ))
                    })?;
                match classification.kind() {
                    LateLoweredSourceStatementClassificationKind::EffectNeutralValue
                    | LateLoweredSourceStatementClassificationKind::BoundaryResultInjection { .. }
                    | LateLoweredSourceStatementClassificationKind::CompletionPayloadInjection { .. } => {
                        if !self.lower_published_call_statement(stmt)? {
                            self.lower_effect_neutral_statement(stmt)?;
                        }
                    }
                    LateLoweredSourceStatementClassificationKind::BoundaryConsumedAnchor { .. }
                    | LateLoweredSourceStatementClassificationKind::ResumePayloadInjection { .. }
                    | LateLoweredSourceStatementClassificationKind::HandleSyntheticCarrierBinder { .. }
                    | LateLoweredSourceStatementClassificationKind::ElidedUnreachable => {}
                    LateLoweredSourceStatementClassificationKind::Unsupported { reason } => {
                        return Err(frontend_error(format!(
                            "refactor source-slice statement bb{} stmt{} classified unsupported: {reason}",
                            slice.block_id().as_u32(),
                            stmt_index,
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    fn lower_state_terminator(&mut self, state: &LateLoweredState) -> Result<(), LlvmEmitError> {
        if self.current_block_is_terminated() {
            return Ok(());
        }
        match state.terminator() {
            LateLoweredStateTerminator::Goto { target } => {
                if self.try_return_handle_completion_from_resume_entry(state, *target)?
                    || self.try_return_wrapper_complete_from_handle_completion(state, *target)?
                    || self.try_route_handle_completion_goto(state, *target)?
                {
                    Ok(())
                } else {
                    self.branch_to_state(*target)
                }
            }
            LateLoweredStateTerminator::Branch {
                cond_local,
                then_state,
                else_state,
            } => {
                let cond = self
                    .load_local_value(self.mir_fun.span, *cond_local)?
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
                payload_source: _,
                complete_state: _,
            } => {
                let binding = self
                    .abi
                    .completion_payload_binding_for_state(self.abi_step_schema, state.state_id())?;
                let _ = self
                    .abi
                    .completion_payload_binding_layout(self.abi_step_schema, binding.binding())?;
                let payload_source = binding.payload_source();
                let payload = self
                    .lower_completion_payload_as(
                        payload_source,
                        self.step_layout.complete_variant().payload_source_ty(),
                    )
                    .map_err(|err| {
                    frontend_error(format!(
                        "refactor return state st{} completion payload {:?} lowering failed: {err}",
                        state.state_id().as_u32(),
                        payload_source,
                    ))
                })?;
                if payload.is_none() && !self.step_layout.complete_variant().payload_is_elided() {
                    return Err(frontend_error(format!(
                        "refactor return state st{} payload source {:?} produced no payload for non-elided Complete layout {}",
                        state.state_id().as_u32(),
                        payload_source,
                        self.step_layout.complete_variant().payload_anchor_name()
                    )));
                }
                match self.return_mode {
                    RefactorCallableReturnMode::Step => {
                        let step = self
                            .codegen
                            .refactor_build_step_complete(self.step_layout, payload)
                            .map_err(|err| {
                                frontend_error(format!(
                                    "refactor return state st{} build Complete step failed: {err}",
                                    state.state_id().as_u32(),
                                ))
                            })?;
                        self.return_step(step).map_err(|err| {
                            frontend_error(format!(
                                "refactor return state st{} return Step failed: {err}",
                                state.state_id().as_u32(),
                            ))
                        })
                    }
                    RefactorCallableReturnMode::Plain { declared_return_cg } => {
                        let value = match payload {
                            Some(raw) => self.codegen.cg_value_from_loaded(
                                self.mir_fun.span,
                                declared_return_cg,
                                raw,
                            )?,
                            None => self
                                .codegen
                                .default_value(self.mir_fun.span, declared_return_cg)?,
                        };
                        let value = self.codegen.coerce_value(
                            self.mir_fun.span,
                            value,
                            declared_return_cg,
                        )?;
                        self.codegen.finish_function_return_path(
                            self.mir_fun.span,
                            declared_return_cg,
                            value,
                        )
                    }
                }
            }
            LateLoweredStateTerminator::Suspend { boundary_ids, .. } => {
                self.lower_suspend(state, boundary_ids)
            }
            LateLoweredStateTerminator::HandleDispatch {
                site_id,
                contract,
                body_state,
                ..
            } => {
                let _ =
                    self.abi
                        .handle_dispatch_layout(self.abi_step_schema, *site_id, contract)?;
                self.branch_to_state(*body_state)
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
                let payload =
                    self.lower_runtime_error_boundary_payload(runtime.payload_tuple_ty)?;
                self.emit_local_runtime_error_terminal(&runtime, payload)
            }
            LateLoweredStateTerminator::Unreachable => {
                self.codegen.builder.build_unreachable()?;
                Ok(())
            }
            LateLoweredStateTerminator::ResumeUnwind => self.lower_resume_unwind_terminator(state),
            LateLoweredStateTerminator::Abandon => self.lower_abandon_terminator(state),
        }
    }

    fn lower_resume_unwind_terminator(
        &mut self,
        state: &LateLoweredState,
    ) -> Result<(), LlvmEmitError> {
        self.verify_resume_unwind_contract(state)?;
        // The verified cleanup route is consumed by the surrounding HandleDispatch
        // pending-completion contract; reaching the terminal directly would mean the
        // upstream handoff lost the unwind carrier/origin route.
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    fn lower_abandon_terminator(&mut self, state: &LateLoweredState) -> Result<(), LlvmEmitError> {
        self.verify_abandon_contract(state)?;
        // The drop state is entered by the continuation runtime/GC contract; no
        // remaining source-level computation is resumed from this block.
        self.codegen.builder.build_unreachable()?;
        Ok(())
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
            .or_else(|| {
                boundary_ids.iter().find_map(|id| {
                    self.callable
                        .boundary_map()
                        .boundary(*id)
                        .filter(|boundary| {
                            matches!(
                                boundary.lowering(),
                                Some(LateLoweredBoundaryLowering::RuntimeError(_))
                            )
                        })
                })
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
                    self.abi_step_schema,
                    source,
                    lowering.operand_contract(),
                )?;
                let args_payload = self.pack_sources(
                    lowering.facts().invoke_args_tuple_ty(),
                    lowering.operand_contract().arg_sources(),
                    "refactor_call_args",
                )?;
                let target =
                    self.abi
                        .call_target_layout(self.abi_step_schema, source, lowering.facts())?;
                let (step, callee_step_schema) = match target {
                    RefactorCallTargetQuery::KnownInstance(layout) => (
                        self.emit_known_instance_call_step(
                            source,
                            layout.direct_entry(),
                            args_payload,
                        )?,
                        layout.step_schema(),
                    ),
                    RefactorCallTargetQuery::DynamicInvoke(layout) => {
                        let carrier_source = lowering.operand_contract().carrier_source().ok_or_else(|| {
                            frontend_error(format!(
                                "refactor dynamic call boundary site {} 缺少 published carrier source",
                                source.as_u32()
                            ))
                        })?;
                        let carrier_value = self.lower_operand_source(carrier_source)?;
                        let Some(BasicValueEnum::PointerValue(carrier)) = carrier_value.value
                        else {
                            return Err(frontend_error(format!(
                                "refactor dynamic call boundary site {} carrier source 不是 pointer",
                                source.as_u32()
                            )));
                        };
                        (
                            self.emit_refactor_dynamic_invoke_step(layout, carrier, args_payload)?,
                            layout.return_step_schema(),
                        )
                    }
                };
                self.dispatch_boundary_step(
                    boundary,
                    callee_step_schema,
                    step,
                    lowering.dispatch(),
                    Some(lowering),
                    Some(lowering.continuation_compositions()),
                )
            }
            LateLoweredBoundaryLowering::ClassCtor(lowering) => {
                self.lower_class_ctor_boundary(boundary, lowering)
            }
            LateLoweredBoundaryLowering::Perform(lowering) => {
                let source = boundary_site(boundary, "Perform")?;
                let _ = self.abi.perform_boundary_operand_layout(
                    self.abi_step_schema,
                    source,
                    lowering.operand_contract(),
                )?;
                if self.perform_lowering_is_runtime_error_raise(lowering) {
                    let span = self.perform_site_span(source)?;
                    self.emit_effect_set_active_with_trace(
                        span,
                        "refactor_raise_set_active_with_trace",
                    )?;
                }
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
                    None,
                    None,
                )
            }
            LateLoweredBoundaryLowering::Resume(lowering) => {
                let source = boundary_site(boundary, "Resume")?;
                let _ = self.abi.resume_boundary_operand_layout(
                    self.abi_step_schema,
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
                self.sync_frame_slots_from_locals()?;
                if self.should_use_task_transport_dynamic_resume(source, surface, lowering)?
                    && self.lower_task_transport_dynamic_resume_boundary(
                        boundary,
                        lowering,
                        surface,
                        cont_ptr,
                        args_payload,
                    )?
                {
                    return Ok(());
                }
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
                let call = self.codegen.build_call_preserving_gc_local_roots(
                    self.mir_fun.span,
                    callee,
                    &args,
                    "refactor_resume_step",
                )?;
                let step = call.try_as_basic_value().basic().ok_or_else(|| {
                    frontend_error("refactor resume boundary callee 未返回 Step_F".to_string())
                })?;
                self.dispatch_boundary_step(
                    boundary,
                    lowering.facts().out_step_schema(),
                    step,
                    lowering.dispatch(),
                    None,
                    Some(lowering.continuation_compositions()),
                )
            }
            LateLoweredBoundaryLowering::RuntimeError(lowering) => {
                self.lower_runtime_error_boundary(boundary, lowering)
            }
            LateLoweredBoundaryLowering::Handle(lowering) => {
                self.lower_handle_boundary(boundary, lowering)
            }
        }
    }

    fn lower_class_ctor_boundary(
        &mut self,
        boundary: &LateLoweredBoundary,
        lowering: &crate::effect_lowered::ir::LateLoweredClassCtorBoundaryLowering,
    ) -> Result<(), LlvmEmitError> {
        let site_id = boundary_site(boundary, "ClassCtor")?;
        let source = self.class_ctor_boundary_statement(lowering, site_id)?;
        let class_layout_key = match &source {
            RefactorClassCtorBoundarySource::ClassCtor { .. } => {
                Some(self.class_ctor_layout_key(lowering.class_fqn(), lowering.result_local())?)
            }
            RefactorClassCtorBoundarySource::ObjectProperty { .. }
            | RefactorClassCtorBoundarySource::TopLevelRef { .. } => None,
        };
        let slots = self.slots.clone();
        let result = self
            .codegen
            .with_active_suspend_site_any_effect_outcome_capture(site_id.as_u32(), |cg| {
                cg.with_ordinary_effect_propagation_suppressed(|cg| match &source {
                    RefactorClassCtorBoundarySource::ClassCtor { span, ctor, args } => {
                        let args = args.to_vec();
                        let class_layout_key = class_layout_key.as_ref().ok_or_else(|| {
                            frontend_error(
                                "refactor class ctor boundary 缺少 class layout key".to_string(),
                            )
                        })?;
                        cg.codegen_mir_refactor_class_ctor_call(
                            *span,
                            class_layout_key,
                            ctor,
                            &args,
                            &slots,
                        )
                    }
                    RefactorClassCtorBoundarySource::ObjectProperty { span, fqn } => {
                        cg.codegen_object_property_access(*span, fqn)
                    }
                    RefactorClassCtorBoundarySource::TopLevelRef { span, fqn } => {
                        cg.codegen_top_level_value_ref(*span, fqn)
                    }
                })
            })?;
        let outcome_slot = self
            .codegen
            .take_suspend_site_explicit_effect_outcome(site_id.as_u32());
        let Some(outcome_slot) = outcome_slot else {
            let _ = self.store_local_value(source.span(), lowering.result_local(), result)?;
            return self.branch_to_state(boundary.resume_state());
        };

        let active_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "class_ctor_hidden_effect_active");
        let inactive_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "class_ctor_hidden_effect_inactive");
        let is_propagating = self.codegen.effect_outcome_is_propagating(
            source.span(),
            outcome_slot,
            "class_ctor_hidden_effect",
        )?;
        self.codegen
            .builder
            .build_conditional_branch(is_propagating, active_bb, inactive_bb)?;

        self.codegen.builder.position_at_end(active_bb);
        let emission = match lowering.emitted_steps() {
            [single] => single,
            [] => {
                return Err(frontend_error(format!(
                    "refactor class ctor boundary bd{} 缺少 hidden effect emission",
                    boundary.boundary_id().as_u32()
                )));
            }
            many => {
                return Err(frontend_error(format!(
                    "refactor class ctor boundary bd{} 发布了 {} 个 hidden effect emission；当前 runtime outcome lowering 需要唯一 ordinary effect case",
                    boundary.boundary_id().as_u32(),
                    many.len()
                )));
            }
        };
        let payload =
            self.lower_class_ctor_outcome_payload(outcome_slot, emission.payload_tuple_ty())?;
        self.emit_or_consume_outward_case(
            boundary,
            emission.case_tag(),
            payload,
            emission.payload_tuple_ty(),
            None,
            None,
        )?;

        self.codegen.builder.position_at_end(inactive_bb);
        let _ = self.store_local_value(source.span(), lowering.result_local(), result)?;
        self.branch_to_state(boundary.resume_state())
    }

    fn class_ctor_layout_key(
        &self,
        class_fqn: &str,
        result_local: LocalId,
    ) -> Result<String, LlvmEmitError> {
        let Some(target_ty) = self
            .body
            .locals
            .get(result_local.as_u32() as usize)
            .map(|local| local.ty)
        else {
            return Ok(class_fqn.to_string());
        };
        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.source_types.kind(target_ty) else {
            return Ok(class_fqn.to_string());
        };
        if nominal.fqn != class_fqn {
            return Ok(class_fqn.to_string());
        }
        let layout = self.abi.class_instance_layout(target_ty)?;
        if layout.base_fqn() != class_fqn {
            return Err(frontend_error(format!(
                "refactor class ctor boundary `{class_fqn}` result local{} resolved to mismatched layout `{}`",
                result_local.as_u32(),
                layout.base_fqn()
            )));
        }
        Ok(layout.class_key().to_string())
    }

    fn class_ctor_boundary_statement(
        &self,
        lowering: &crate::effect_lowered::ir::LateLoweredClassCtorBoundaryLowering,
        site_id: SiteId,
    ) -> Result<RefactorClassCtorBoundarySource<'a>, LlvmEmitError> {
        let Some(statement_index) = lowering.source_consumption().statement_index() else {
            return Err(frontend_error(format!(
                "refactor class ctor boundary site{} source consumption 不是 statement anchor",
                site_id.as_u32()
            )));
        };
        let source_slice = lowering.source_consumption().source_slice();
        let block = self
            .body
            .blocks
            .get(source_slice.block_id().as_u32() as usize)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor class ctor boundary site{} source block bb{} 不存在",
                    site_id.as_u32(),
                    source_slice.block_id().as_u32()
                ))
            })?;
        let stmt = block.stmts.get(statement_index as usize).ok_or_else(|| {
            frontend_error(format!(
                "refactor class ctor boundary site{} source statement {} 不存在",
                site_id.as_u32(),
                statement_index
            ))
        })?;
        match &stmt.kind {
            mir::StatementKind::Assign {
                value:
                    mir::Rvalue::ClassCtor {
                        site_id: stmt_site,
                        ctor,
                        args,
                        ..
                    },
                ..
            } if *stmt_site == site_id => Ok(RefactorClassCtorBoundarySource::ClassCtor {
                span: stmt.span,
                ctor,
                args,
            }),
            mir::StatementKind::Assign {
                value: mir::Rvalue::TopLevelRef(top_level),
                ..
            } if top_level.site_id == Some(site_id) && !top_level.hidden_effects.is_pure() => {
                Ok(RefactorClassCtorBoundarySource::TopLevelRef {
                    span: stmt.span,
                    fqn: &top_level.fqn,
                })
            }
            mir::StatementKind::Assign {
                value:
                    mir::Rvalue::MemberAccess {
                        site_id: Some(stmt_site),
                        member,
                        ..
                    },
                ..
            } if *stmt_site == site_id && !member.hidden_effects.is_pure() => {
                let Some(mir::MemberTarget::Value { fqn }) = member.resolved.as_ref() else {
                    return Err(frontend_error(format!(
                        "refactor class ctor boundary site{} hidden member source 不是 resolved value member",
                        site_id.as_u32()
                    )));
                };
                Ok(RefactorClassCtorBoundarySource::ObjectProperty {
                    span: stmt.span,
                    fqn,
                })
            }
            _ => Err(frontend_error(format!(
                "refactor class ctor boundary site{} source anchor 不是 ClassCtor/hidden member statement",
                site_id.as_u32()
            ))),
        }
    }

    fn lower_class_ctor_outcome_payload(
        &mut self,
        outcome_slot: PointerValue<'ctx>,
        payload_ty: TypeId,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(payload_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        let payload_cg = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, payload_ty)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor class ctor hidden effect payload t{} 缺少 codegen type",
                    payload_ty.as_u32()
                ))
            })?;
        let transport = self.codegen.effect_outcome_payload_transport(
            self.mir_fun.span,
            outcome_slot,
            "class_ctor_hidden_effect_payload",
        )?;
        let decoded = self.codegen.decode_effect_transport_value(
            self.mir_fun.span,
            transport.word,
            transport.gc_ref,
            payload_cg,
        )?;
        decoded
            .value
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor class ctor hidden effect payload t{} decoded to elided value despite non-elided ABI",
                    payload_ty.as_u32()
                ))
            })
            .map(Some)
    }

    fn should_use_task_transport_dynamic_resume(
        &mut self,
        site_id: SiteId,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        lowering: &crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering,
    ) -> Result<bool, LlvmEmitError> {
        // These continuations are stored in heap state and later resumed from helper paths, so
        // their concrete owner route is recovered from the continuation object descriptor.
        if !self.is_task_transport_tuple_ty(surface.resume_tuple_ty())? {
            return Ok(false);
        }
        let route = lowering.operand_contract().underlying_continuation_route();
        Ok(matches!(
            route.publication(),
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary {
                owner_version_key,
                site_id: route_site,
                ..
            } if owner_version_key == self.callable.body_version_key() && *route_site == site_id
        ))
    }

    fn lower_task_transport_dynamic_resume_boundary(
        &mut self,
        boundary: &LateLoweredBoundary,
        lowering: &crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering,
        surface: &RefactorContinuationSurfaceResumeLayout<'ctx>,
        cont_ptr: PointerValue<'ctx>,
        args_payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<bool, LlvmEmitError> {
        let payload = args_payload.ok_or_else(|| {
            frontend_error(format!(
                "refactor task transport resume bd{} 需要 non-elided payload",
                boundary.boundary_id().as_u32()
            ))
        })?;
        let candidates =
            self.task_transport_resume_candidates(lowering, surface.resume_tuple_ty())?;
        if candidates.is_empty() {
            return Ok(false);
        }

        let current_desc = self.load_gc_object_type_desc(cont_ptr, "task_resume_cont_desc")?;
        let word_ty = self.codegen.context.i64_type();
        let current_desc_int = self.codegen.builder.build_ptr_to_int(
            current_desc,
            word_ty,
            "task_resume_cont_desc_int",
        )?;
        let first_check = self
            .codegen
            .context
            .append_basic_block(self.function, "task_resume_check0");
        self.codegen
            .builder
            .build_unconditional_branch(first_check)?;

        let mut check_bb = first_check;
        for (index, candidate) in candidates.into_iter().enumerate() {
            let next_bb = self
                .codegen
                .context
                .append_basic_block(self.function, &format!("task_resume_check{}", index + 1));
            let hit_bb = self.codegen.context.append_basic_block(
                self.function,
                &format!(
                    "task_resume_hit_s{}",
                    candidate.callable.step_schema().as_u32()
                ),
            );
            self.codegen.builder.position_at_end(check_bb);
            let target_desc_int = self.codegen.builder.build_ptr_to_int(
                candidate.type_desc_i8,
                word_ty,
                "task_resume_target_desc_int",
            )?;
            let is_match = self.codegen.builder.build_int_compare(
                inkwell::IntPredicate::EQ,
                current_desc_int,
                target_desc_int,
                "task_resume_desc_match",
            )?;
            self.codegen
                .builder
                .build_conditional_branch(is_match, hit_bb, next_bb)?;

            self.codegen.builder.position_at_end(hit_bb);
            let args = vec![cont_ptr.into(), payload.into()];
            let call = self.codegen.build_call_preserving_gc_local_roots(
                self.mir_fun.span,
                candidate.adapter,
                &args,
                "refactor_task_transport_resume",
            )?;
            let owner_step = call.try_as_basic_value().basic().ok_or_else(|| {
                frontend_error(format!(
                    "refactor task transport resume adapter `{}` 未返回 Step_F",
                    candidate.adapter.get_name().to_str().unwrap_or("<invalid>")
                ))
            })?;
            self.dispatch_boundary_step(
                boundary,
                candidate.callable.step_schema(),
                owner_step,
                &candidate.dispatch_plan,
                None,
                None,
            )?;
            check_bb = next_bb;
        }

        self.codegen.builder.position_at_end(check_bb);
        self.codegen.builder.build_unreachable()?;
        Ok(true)
    }

    fn task_transport_resume_candidates(
        &mut self,
        lowering: &crate::effect_lowered::ir::LateLoweredResumeBoundaryLowering,
        transport_ty: TypeId,
    ) -> Result<Vec<TaskTransportResumeCandidate<'a, 'ctx>>, LlvmEmitError> {
        let mut candidates = Vec::new();
        for callable in self.program.callables() {
            if !callable.has_control_body()
                || callable.frame_schema().resume_payload_bindings().is_empty()
            {
                continue;
            }
            let Some(dispatch_plan) = self
                .task_transport_owner_dispatch_plan(callable.step_schema(), lowering.dispatch())?
            else {
                continue;
            };
            if !self.callable_accepts_task_transport_resume(callable, transport_ty)? {
                continue;
            }
            let continuation_layout = self
                .abi
                .continuation_layout(callable.continuation_object())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor task transport resume 缺少 callable `{}` continuation layout",
                        callable.root_fqn()
                    ))
                })?;
            let type_desc = self.codegen.get_or_create_refactor_gc_type_descriptor(
                self.mir_fun.span,
                continuation_layout.llvm_ty(),
                continuation_layout.layout_anchor_name(),
            )?;
            let type_desc_i8 = self.codegen.builder.build_pointer_cast(
                type_desc.as_pointer_value(),
                self.codegen.llvm_i8_ptr_type(),
                "task_resume_candidate_type_desc",
            )?;
            let adapter = self.ensure_task_transport_resume_adapter(callable, transport_ty)?;
            candidates.push(TaskTransportResumeCandidate {
                callable,
                adapter,
                type_desc_i8,
                dispatch_plan,
            });
        }
        Ok(candidates)
    }

    fn callable_accepts_task_transport_resume(
        &mut self,
        callable: &LateLoweredCallable,
        transport_ty: TypeId,
    ) -> Result<bool, LlvmEmitError> {
        for binding in callable.frame_schema().resume_payload_bindings() {
            let Some(mir_fun) = refactor_mir_callable(self.pass_view, callable.root_fqn()).ok()
            else {
                continue;
            };
            let Some(body) = mir_fun.body.as_ref() else {
                continue;
            };
            let local_ty = body.locals[binding.consumer_local().as_u32() as usize].ty;
            if local_ty != transport_ty || self.is_task_transport_tuple_ty(local_ty)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn task_transport_owner_dispatch_plan(
        &self,
        owner_step_schema: StepSchemaId,
        wrapper_dispatch: &LateLoweredStepDispatchPlan,
    ) -> Result<Option<LateLoweredStepDispatchPlan>, LlvmEmitError> {
        let owner_step = self.program.step_type(owner_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor task transport resume 缺少 owner step schema s{}",
                owner_step_schema.as_u32()
            ))
        })?;
        if owner_step.complete_ty() != wrapper_dispatch.complete().answer_ty() {
            return Ok(None);
        }
        let wrapper_step = self
            .program
            .step_type(wrapper_dispatch.input_step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor task transport resume 缺少 wrapper step schema s{}",
                    wrapper_dispatch.input_step_schema().as_u32()
                ))
            })?;
        let mut outward_cases = Vec::new();
        for wrapper_forwarding in wrapper_dispatch.outward_cases() {
            let Some(wrapper_case) = wrapper_step.case(wrapper_forwarding.input_case_tag()) else {
                return Ok(None);
            };
            let Some(owner_case) = owner_step.cases().iter().find(|candidate| {
                candidate.concrete_op_key() == wrapper_case.concrete_op_key()
                    && candidate.payload_tuple_ty() == wrapper_case.payload_tuple_ty()
            }) else {
                return Ok(None);
            };
            outward_cases.push(LateLoweredStepCaseForwarding::new(
                owner_case.case_tag(),
                owner_case.concrete_op_key().clone(),
                wrapper_forwarding.emission().clone(),
            ));
        }
        Ok(Some(LateLoweredStepDispatchPlan::new(
            owner_step_schema,
            wrapper_dispatch.complete().clone(),
            outward_cases,
        )))
    }

    fn ensure_task_transport_resume_adapter(
        &mut self,
        callable: &'a LateLoweredCallable,
        transport_ty: TypeId,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let step_layout = self
            .abi
            .step_layout(callable.step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor task transport resume 缺少 callable `{}` step layout s{}",
                    callable.root_fqn(),
                    callable.step_schema().as_u32()
                ))
            })?;
        let payload_layout = self.abi.source_value_layout(transport_ty)?;
        let payload_abi = *payload_layout.abi();
        let mut params: Vec<BasicMetadataTypeEnum<'ctx>> =
            vec![self.codegen.llvm_gc_i8_ptr_type().into()];
        if !payload_abi.is_elided() {
            params.push(payload_abi.llvm_ty().into());
        }
        let fn_ty = step_layout.llvm_ty().fn_type(&params, false);
        let symbol_name = format!(
            "__scoop_refactor_task_transport_resume__s{}",
            callable.step_schema().as_u32()
        );
        let function = self
            .codegen
            .module
            .get_function(&symbol_name)
            .unwrap_or_else(|| self.codegen.module.add_function(&symbol_name, fn_ty, None));
        if function.count_basic_blocks() > 0 {
            return Ok(function);
        }

        let saved_block = self.codegen.builder.get_insert_block();
        let mut child = self.codegen.fresh_child_codegen();
        let mir_fun = refactor_mir_callable(self.pass_view, callable.root_fqn())?;
        let body = mir_fun.body.as_ref().ok_or_else(|| {
            frontend_error(format!(
                "refactor task transport resume adapter `{}` owner `{}` 缺少 canonical MIR body",
                symbol_name,
                callable.root_fqn()
            ))
        })?;
        let entry = child.context.append_basic_block(function, "entry");
        child.builder.position_at_end(entry);
        child.begin_function_explicit_frame_layout(function)?;
        RefactorCallableEmitter::new(
            &mut child,
            self.program,
            self.source_types,
            self.pass_view,
            self.abi,
            callable,
            mir_fun,
            body,
            function,
            None,
            None,
            None,
            RefactorHandleCompletionMode::ReturnFromFunction,
        )?
        .emit_resume_entry(transport_ty)?;
        child.finish_function_explicit_frame_layout(mir_fun.span)?;
        if let Some(block) = saved_block {
            self.codegen.builder.position_at_end(block);
        }
        Ok(function)
    }

    fn load_gc_object_type_desc(
        &mut self,
        obj: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let header_ty = self.codegen.llvm_gc_object_header_type();
        let header_ptr_ty = self.codegen.llvm_ptr_type(self.codegen.gc_address_space());
        let header_ptr =
            self.codegen
                .builder
                .build_pointer_cast(obj, header_ptr_ty, &format!("{name}_hdr"))?;
        let type_desc_ptr = self.codegen.builder.build_struct_gep(
            header_ty,
            header_ptr,
            1,
            &format!("{name}_gep"),
        )?;
        Ok(self
            .codegen
            .builder
            .build_load(self.codegen.llvm_i8_ptr_type(), type_desc_ptr, name)?
            .into_pointer_value())
    }

    fn lower_runtime_error_boundary(
        &mut self,
        boundary: &LateLoweredBoundary,
        lowering: &crate::effect_lowered::ir::LateLoweredRuntimeErrorBoundaryLowering,
    ) -> Result<(), LlvmEmitError> {
        let payload =
            self.lower_runtime_error_boundary_payload(lowering.emitted_step().payload_tuple_ty())?;
        self.emit_or_consume_outward_case(
            boundary,
            lowering.emitted_step().case_tag(),
            payload,
            lowering.emitted_step().payload_tuple_ty(),
            None,
            None,
        )
    }

    fn perform_lowering_is_runtime_error_raise(
        &self,
        lowering: &crate::effect_lowered::ir::LateLoweredPerformBoundaryLowering,
    ) -> bool {
        lowering
            .emitted_step()
            .concrete_op_key()
            .instance_key()
            .template
            .fqn
            == "scoop.core.Raise.raise"
            && self.source_ty_is_runtime_error(lowering.emitted_step().payload_tuple_ty())
    }

    fn perform_site_span(&self, site_id: SiteId) -> Result<crate::span::Span, LlvmEmitError> {
        for block in &self.body.blocks {
            if let mir::TerminatorKind::Perform {
                site_id: terminator_site,
                ..
            } = &block.terminator.kind
                && *terminator_site == site_id
            {
                return Ok(block.terminator.span);
            }
        }
        Err(frontend_error(format!(
            "refactor perform site {} 缺少 canonical MIR terminator span",
            site_id.as_u32()
        )))
    }

    fn emit_effect_set_active_with_trace(
        &mut self,
        span: crate::span::Span,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let (line, col) = self
            .codegen
            .current_source()?
            .offset_to_line_col(span.start)
            .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                kind: "refactor effect trace source location",
                at: span.into(),
            })?;
        let line = u32::try_from(line).map_err(|_| LlvmEmitError::UnsupportedMainBody {
            kind: "refactor effect trace line overflow",
            at: span.into(),
        })?;
        let col = u32::try_from(col).map_err(|_| LlvmEmitError::UnsupportedMainBody {
            kind: "refactor effect trace column overflow",
            at: span.into(),
        })?;
        let i32_ty = self.codegen.context.i32_type();
        let set_active = self.codegen.declare_runtime_effect_set_active_with_trace();
        self.codegen.builder.build_call(
            set_active,
            &[
                i32_ty.const_int(line as u64, false).into(),
                i32_ty.const_int(col as u64, false).into(),
            ],
            name,
        )?;
        Ok(())
    }

    fn lower_runtime_error_boundary_payload(
        &mut self,
        payload_ty: TypeId,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let layout = self.abi.source_value_layout(payload_ty)?;
        if layout.abi().is_elided() {
            return Ok(None);
        }
        let value =
            self.runtime_error_unit_variant_payload(payload_ty, "ContinuationAlreadyResumed")?;
        match layout.kind() {
            RefactorSourceAbiLayoutKind::Scalar => Ok(value.value),
            RefactorSourceAbiLayoutKind::Tuple => Err(frontend_error(format!(
                "refactor runtime-error boundary payload t{} 需要 scalar ABI，当前 tuple ABI 尚未发布 payload field contract",
                payload_ty.as_u32()
            ))),
        }
    }

    fn runtime_error_unit_variant_payload(
        &mut self,
        payload_ty: TypeId,
        variant_name: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !self.source_ty_is_runtime_error(payload_ty) {
            return Err(frontend_error(format!(
                "refactor runtime-error payload t{} 不是 scoop.core.RuntimeError",
                payload_ty.as_u32()
            )));
        }
        let enum_layout = self
            .codegen
            .enum_layouts
            .get("scoop.core.RuntimeError")
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor RuntimeError enum layout",
                at: self.mir_fun.span.into(),
            })?;
        let variant = enum_layout
            .variants
            .iter()
            .find(|variant| variant.name == variant_name)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor RuntimeError variant layout",
                at: self.mir_fun.span.into(),
            })?;
        if !variant.fields.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor RuntimeError payload variant arity",
                at: self.mir_fun.span.into(),
            });
        }
        let abi = self.abi.source_value_layout(payload_ty)?.abi();
        let raw = match abi.llvm_ty() {
            BasicTypeEnum::IntType(int_ty) => int_ty.const_int(variant.tag, false).into(),
            BasicTypeEnum::StructType(struct_ty) => {
                let Some(BasicTypeEnum::IntType(tag_ty)) = struct_ty.get_field_type_at_index(0)
                else {
                    return Err(frontend_error(
                        "refactor RuntimeError tagged-union payload 缺少整数 tag field".to_string(),
                    ));
                };
                let mut aggregate = struct_ty.get_undef();
                aggregate = self
                    .codegen
                    .builder
                    .build_insert_value(
                        aggregate,
                        tag_ty.const_int(variant.tag, false),
                        0,
                        "refactor_runtime_error_tag",
                    )?
                    .into_struct_value();
                for field_index in 1..struct_ty.count_fields() {
                    let Some(field_ty) = struct_ty.get_field_type_at_index(field_index) else {
                        return Err(frontend_error(format!(
                            "refactor RuntimeError tagged-union payload 缺少 field {}",
                            field_index
                        )));
                    };
                    aggregate = self
                        .codegen
                        .builder
                        .build_insert_value(
                            aggregate,
                            field_ty.const_zero(),
                            field_index,
                            "refactor_runtime_error_payload_zero",
                        )?
                        .into_struct_value();
                }
                aggregate.into()
            }
            other => {
                return Err(frontend_error(format!(
                    "refactor RuntimeError payload ABI 不是 int/struct：{:?}",
                    other
                )));
            }
        };
        Ok(CgValue {
            ty: CgTy::Enum(payload_ty),
            value: Some(raw),
        })
    }

    fn emit_local_runtime_error_terminal(
        &mut self,
        runtime: &RefactorLocalRuntimeErrorRuntime,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        let payload = payload.ok_or_else(|| {
            frontend_error(format!(
                "refactor LocalRuntimeError st{} call site {} case c{} 需要 materialized payload t{}，但 lowering 产出了 elided payload",
                runtime.target_state.as_u32(),
                runtime.site_id.as_u32(),
                runtime.input_case_tag.as_u32(),
                runtime.payload_tuple_ty.as_u32()
            ))
        })?;
        let callee = self
            .codegen
            .module
            .get_function(&runtime.runtime_symbol)
            .unwrap_or_else(|| self.codegen.declare_runtime_error_fatal());
        if callee.count_params() as usize != runtime.runtime_param_count {
            return Err(frontend_error(format!(
                "refactor LocalRuntimeError runtime entry `{}` 参数数量漂移：module={} contract={}",
                runtime.runtime_symbol,
                callee.count_params(),
                runtime.runtime_param_count
            )));
        }
        let payload = self.materialize_runtime_error_fatal_payload(payload)?;
        self.codegen.builder.build_call(
            callee,
            &[payload.into()],
            "refactor_local_runtime_error_fatal",
        )?;
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    fn materialize_runtime_error_fatal_payload(
        &mut self,
        payload: BasicValueEnum<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let runtime_payload_ty = self.codegen.llvm_gc_i8_ptr_type();
        if let BasicValueEnum::PointerValue(ptr) = payload {
            return self.codegen.refactor_cast_ptr(
                ptr,
                runtime_payload_ty,
                "refactor_runtime_error_payload_ptr",
            );
        }

        let slot = self
            .codegen
            .builder
            .build_alloca(payload.get_type(), "refactor_runtime_error_payload_obj")?;
        self.codegen.builder.build_store(slot, payload)?;
        self.codegen.refactor_cast_ptr(
            slot,
            runtime_payload_ty,
            "refactor_runtime_error_payload_ptr",
        )
    }

    fn lower_handle_boundary(
        &mut self,
        boundary: &LateLoweredBoundary,
        lowering: &crate::effect_lowered::ir::LateLoweredHandleBoundaryLowering,
    ) -> Result<(), LlvmEmitError> {
        match lowering.outward_emissions() {
            [] => self.branch_to_state(boundary.resume_state()),
            [emission] => self.emit_or_consume_outward_case(
                boundary,
                emission.case_tag(),
                None,
                emission.payload_tuple_ty(),
                None,
                None,
            ),
            emissions => Err(frontend_error(format!(
                "refactor handle boundary bd{} 发布了 {} 个 outward emission；需要 HandleDispatch contract 选择具体 case",
                boundary.boundary_id().as_u32(),
                emissions.len()
            ))),
        }
    }

    fn dispatch_boundary_step(
        &mut self,
        boundary: &LateLoweredBoundary,
        input_step_schema: StepSchemaId,
        step: BasicValueEnum<'ctx>,
        dispatch: &crate::effect_lowered::ir::LateLoweredStepDispatchPlan,
        call_lowering: Option<&LateLoweredCallBoundaryLowering>,
        continuation_compositions: Option<&[LateLoweredCallBoundaryContinuationComposition]>,
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
        let local_runtime_error_case = match call_lowering
            .and_then(LateLoweredCallBoundaryLowering::consumed_runtime_error_case)
        {
            Some(contract) => {
                let case_layout = input_layout.case_layout(contract.input_case_tag()).ok_or_else(|| {
                    frontend_error(format!(
                        "refactor call boundary bd{} local runtime-error case c{} 缺少 input Step layout",
                        boundary.boundary_id().as_u32(),
                        contract.input_case_tag().as_u32()
                    ))
                })?;
                let source = boundary_site(boundary, "Call")?;
                let runtime = self.local_runtime_error_runtime_for_call(source, contract)?;
                let bb = self.codegen.context.append_basic_block(
                    function,
                    &format!(
                        "bd{}_local_runtime_error_case{}",
                        boundary.boundary_id().as_u32(),
                        contract.input_case_tag().as_u32()
                    ),
                );
                Some((
                    self.codegen
                        .context
                        .i32_type()
                        .const_int(case_layout.variant().tag_value() as u64, false),
                    bb,
                    contract.input_case_tag(),
                    runtime,
                ))
            }
            None => None,
        };
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
        let mut switch_cases = cases
            .iter()
            .map(|(tag, bb, _, _, _)| (*tag, *bb))
            .collect::<Vec<_>>();
        if let Some((tag, bb, _, _)) = &local_runtime_error_case {
            switch_cases.push((*tag, *bb));
        }
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
        if matches!(
            boundary.lowering(),
            Some(LateLoweredBoundaryLowering::Resume(_))
        ) {
            self.restore_frame_slots_to_locals()?;
        }
        if !self.try_route_boundary_complete_to_handle_completion(boundary)? {
            self.release_frame_root_for_frame_free_tail(boundary.resume_state())?;
            self.branch_to_state(boundary.resume_state())?;
        }

        for (_, bb, input_case, output_case, payload_ty) in cases {
            self.codegen.builder.position_at_end(bb);
            let case_layout = input_layout.case_layout(input_case).ok_or_else(|| {
                frontend_error(format!(
                    "refactor boundary dispatch 缺少 case c{}",
                    input_case.as_u32()
                ))
            })?;
            let (payload, callee_continuation) = self.codegen.refactor_extract_step_case_parts(
                input_layout,
                step,
                case_layout,
                "refactor_boundary_case_payload",
            )?;
            let composition = match continuation_compositions {
                Some(compositions) => {
                    let composition = compositions
                        .iter()
                        .find(|composition| composition.input_case_tag() == input_case)
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "refactor boundary bd{} case c{} 缺少 continuation composition contract",
                                boundary.boundary_id().as_u32(),
                                input_case.as_u32(),
                            ))
                        })?;
                    if composition.output_case_tag() != output_case {
                        return Err(frontend_error(format!(
                            "refactor boundary bd{} case c{} continuation composition 输出 case 漂移：composition=c{} dispatch=c{}",
                            boundary.boundary_id().as_u32(),
                            input_case.as_u32(),
                            composition.output_case_tag().as_u32(),
                            output_case.as_u32(),
                        )));
                    }
                    Some(composition)
                }
                None => None,
            };
            let continuation_for_binder = if continuation_compositions.is_some() {
                composition.map(|_| callee_continuation)
            } else {
                Some(callee_continuation)
            };
            self.emit_or_consume_outward_case(
                boundary,
                output_case,
                payload,
                payload_ty,
                continuation_for_binder,
                composition,
            )?;
        }

        if let Some((_, bb, input_case, runtime)) = local_runtime_error_case {
            self.codegen.builder.position_at_end(bb);
            let case_layout = input_layout.case_layout(input_case).ok_or_else(|| {
                frontend_error(format!(
                    "refactor boundary dispatch 缺少 local runtime-error case c{}",
                    input_case.as_u32()
                ))
            })?;
            let (payload, _continuation) = self.codegen.refactor_extract_step_case_parts(
                input_layout,
                step,
                case_layout,
                "refactor_local_runtime_error_payload",
            )?;
            self.emit_local_runtime_error_terminal(&runtime, payload)?;
        }

        self.codegen.builder.position_at_end(unmatched_bb);
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    fn try_route_boundary_complete_to_handle_completion(
        &mut self,
        boundary: &LateLoweredBoundary,
    ) -> Result<bool, LlvmEmitError> {
        let Some(result_local) = boundary_complete_result_local(boundary) else {
            return Ok(false);
        };
        let mut matched_target = None;
        for state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch { contract, .. } = state.terminator()
            else {
                continue;
            };
            if contract.needs_completion_state()
                || contract.boundary_routing(boundary.boundary_id()).is_none()
            {
                continue;
            }
            let target = match contract.state_region(boundary.owner_state()) {
                LateLoweredHandleStateRegion::Body => contract
                    .body_completion_payload_source()
                    .filter(|source| completion_payload_source_is_local(source, result_local))
                    .map(|_| contract.body_complete_target()),
                LateLoweredHandleStateRegion::Arm {
                    handled_case,
                    arm_ordinal,
                } => contract
                    .handled_arms()
                    .iter()
                    .find(|arm| {
                        arm.arm_ordinal() == arm_ordinal && arm.handled_case() == handled_case
                    })
                    .filter(|arm| {
                        completion_payload_source_is_local(
                            arm.completion_payload_source(),
                            result_local,
                        )
                    })
                    .map(|_| contract.arm_complete_target()),
                _ => None,
            };
            let Some(target) = target else {
                continue;
            };
            if matched_target.replace(target).is_some() {
                return Err(frontend_error(format!(
                    "refactor boundary bd{} complete 命中多个 handle completion target",
                    boundary.boundary_id().as_u32(),
                )));
            }
        }
        let Some(target) = matched_target else {
            return Ok(false);
        };
        self.copy_boundary_complete_to_handle_return_payload(result_local, target)?;
        self.branch_to_state(target)?;
        Ok(true)
    }

    fn copy_boundary_complete_to_handle_return_payload(
        &mut self,
        result_local: LocalId,
        target: StateId,
    ) -> Result<(), LlvmEmitError> {
        let Some(binding) = self
            .callable
            .frame_schema()
            .completion_payload_binding_for_state(target)
        else {
            return Ok(());
        };
        let Some(source) = binding.payload_source().operand_source() else {
            return Ok(());
        };
        let LateLoweredOperandValueSource::Local(target_local) = source.value() else {
            return Ok(());
        };
        if *target_local == result_local {
            return Ok(());
        }
        let value = self.load_local_value(self.mir_fun.span, result_local)?;
        let _ = self.store_local_value(self.mir_fun.span, *target_local, value)?;
        if let Some(frame_slot) = binding.payload_frame_slot() {
            self.store_local_to_frame_slot(*target_local, frame_slot)?;
        }
        Ok(())
    }

    fn emit_or_consume_outward_case(
        &mut self,
        boundary: &LateLoweredBoundary,
        case_tag: CaseTag,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
        callee_continuation: Option<PointerValue<'ctx>>,
        composition: Option<&LateLoweredCallBoundaryContinuationComposition>,
    ) -> Result<(), LlvmEmitError> {
        if composition.is_some() && callee_continuation.is_none() {
            return Err(frontend_error(format!(
                "refactor boundary bd{} case c{} 的 callee continuation 与 composition contract 不一致",
                boundary.boundary_id().as_u32(),
                case_tag.as_u32(),
            )));
        }
        self.sync_frame_slots_from_locals()?;
        let mut routed_action = self.handle_boundary_action(boundary.boundary_id(), case_tag)?;
        let skip_finalized_site =
            if let Some(RefactorHandleBoundaryRuntimeAction::PendingCompletion(action)) =
                &routed_action
            {
                self.composed_resume_already_ran_handle_finally(action, composition)?
                    .then_some(action.site_id)
            } else {
                None
            };
        if let Some(site_id) = skip_finalized_site {
            routed_action = self.handle_boundary_action_excluding(
                boundary.boundary_id(),
                case_tag,
                Some(site_id),
            )?;
        }
        match routed_action {
            Some(RefactorHandleBoundaryRuntimeAction::ConsumeToArm(action)) => {
                let deferred_payload = if action.continuation_binder.is_some() {
                    let payload_cg = self
                        .codegen
                        .cg_ty_of_mir_type(self.source_types, payload_ty)
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "refactor handle arm payload t{} 缺少 codegen type",
                                payload_ty.as_u32()
                            ))
                        })?;
                    payload
                        .map(|raw| {
                            self.codegen.defer_gc_sensitive_cg_value(
                                self.mir_fun.span,
                                "refactor_handle_arm_payload",
                                CgValue {
                                    ty: payload_cg,
                                    value: Some(raw),
                                },
                            )
                        })
                        .transpose()?
                } else {
                    None
                };
                if let Some(binder) = action.continuation_binder {
                    let continuation = if composition.is_some() {
                        self.create_continuation_object(
                            boundary.resume_state(),
                            callee_continuation,
                            composition,
                        )?
                    } else if let Some(callee_continuation) = callee_continuation {
                        callee_continuation
                    } else {
                        self.create_continuation_object(boundary.resume_state(), None, None)?
                    };
                    self.store_gc_ref_to_binder(binder, continuation)?;
                }
                let payload = if let Some(deferred_payload) = deferred_payload {
                    self.codegen
                        .materialize_deferred_cg_value(
                            self.mir_fun.span,
                            "refactor_handle_arm_payload_reload",
                            deferred_payload,
                        )?
                        .value
                } else {
                    payload
                };
                self.store_case_payload_to_arm_binders(
                    &action.payload_binders,
                    payload,
                    payload_ty,
                )?;
                return self.branch_to_state(action.arm_state);
            }
            Some(RefactorHandleBoundaryRuntimeAction::PendingCompletion(action)) => {
                self.begin_handle_pending_completion(action, Some((payload, payload_ty)))?;
                return Ok(());
            }
            Some(RefactorHandleBoundaryRuntimeAction::EmitOutward) | None => {}
        }
        let deferred_payload = payload
            .map(|raw| {
                let payload_cg = self
                    .codegen
                    .cg_ty_of_mir_type(self.source_types, payload_ty)
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor outward payload t{} 缺少 codegen type",
                            payload_ty.as_u32()
                        ))
                    })?;
                self.codegen.defer_gc_sensitive_cg_value(
                    self.mir_fun.span,
                    "refactor_outward_payload",
                    CgValue {
                        ty: payload_cg,
                        value: Some(raw),
                    },
                )
            })
            .transpose()?;
        let continuation = self.create_continuation_object(
            boundary.resume_state(),
            callee_continuation,
            composition,
        )?;
        let out_layout = self.step_layout.case_layout(case_tag).ok_or_else(|| {
            frontend_error(format!(
                "refactor callable `{}` step schema s{} 缺少 outward case c{}",
                self.callable.root_fqn(),
                self.abi_step_schema.as_u32(),
                case_tag.as_u32()
            ))
        })?;
        let payload = if let Some(deferred_payload) = deferred_payload {
            self.codegen
                .materialize_deferred_cg_value(
                    self.mir_fun.span,
                    "refactor_outward_payload_reload",
                    deferred_payload,
                )?
                .value
        } else {
            payload
        };
        let step = self.codegen.refactor_build_step_case(
            self.step_layout,
            out_layout,
            payload,
            continuation,
        )?;
        self.return_step(step)
    }

    fn composed_resume_already_ran_handle_finally(
        &self,
        action: &RefactorHandlePendingCompletionRuntime,
        composition: Option<&LateLoweredCallBoundaryContinuationComposition>,
    ) -> Result<bool, LlvmEmitError> {
        let Some(composition) = composition else {
            return Ok(false);
        };
        let dispatch = self
            .abi
            .surface_resume_dispatch_layout(composition.callee_continuation_schema())?;
        for target in dispatch.target().owner_trampolines() {
            if target
                .handle_binder_routes()
                .iter()
                .any(|route| route.site_id() == action.site_id)
            {
                return Ok(true);
            }
            if let Some(projection) = target.wrapper_projection()
                && matches!(
                    projection.underlying_route().publication(),
                    LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                        site_id,
                        ..
                    } if *site_id == action.site_id
                )
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn return_step(&mut self, step: BasicValueEnum<'ctx>) -> Result<(), LlvmEmitError> {
        if let RefactorCallableReturnMode::Plain { .. } = self.return_mode {
            return Err(frontend_error(format!(
                "refactor plain callable `{}` 的本地 effect/control path 尝试向外返回 Step_F；P5 handoff 应保证 NoOutward body 的 case 被本地 handle/catch 消费",
                self.callable.root_fqn()
            )));
        }
        self.sync_frame_slots_from_locals()?;
        if let Some(projection) = self.return_projection {
            self.project_owner_step_to_wrapper(projection, step)
        } else if let Some(return_step_schema) = self.return_step_schema {
            let projected = if return_step_schema == self.abi_step_schema {
                step
            } else {
                self.codegen.project_refactor_step_to_schema(
                    self.abi,
                    step,
                    self.abi_step_schema,
                    return_step_schema,
                )?
            };
            self.codegen.builder.build_return(Some(&projected))?;
            Ok(())
        } else {
            self.codegen.builder.build_return(Some(&step))?;
            Ok(())
        }
    }

    fn project_owner_step_to_wrapper(
        &mut self,
        projection: &crate::effect_lowered::ir::LateLoweredSurfaceResumeWrapperProjection,
        owner_step: BasicValueEnum<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let owner_step_schema = projection.owner_step_schema();
        let wrapper_step_schema = projection.wrapper_step_schema();
        let owner_layout = self.abi.step_layout(owner_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor wrapper projection 缺少 owner step schema s{} layout",
                owner_step_schema.as_u32()
            ))
        })?;
        let wrapper_layout = self.abi.step_layout(wrapper_step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor wrapper projection 缺少 wrapper step schema s{} layout",
                wrapper_step_schema.as_u32()
            ))
        })?;
        let tag = self
            .codegen
            .refactor_extract_step_tag(owner_layout, owner_step)?;
        let complete_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "wrapper_project_complete");
        let unmatched_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "wrapper_project_unmatched");
        let cases = projection
            .outward_cases()
            .iter()
            .map(|case| {
                let owner_case_tag = case.owner_case_tag();
                let wrapper_case_tag = case.wrapper_case_tag();
                let owner_case = owner_layout
                    .case_layout(owner_case_tag)
                    .expect("projection case was validated by helper");
                (
                    self.codegen
                        .context
                        .i32_type()
                        .const_int(owner_case.variant().tag_value() as u64, false),
                    self.codegen.context.append_basic_block(
                        self.function,
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
        let complete_tag = self
            .codegen
            .context
            .i32_type()
            .const_int(STEP_TAG_COMPLETE, false);
        let is_complete = self.codegen.builder.build_int_compare(
            inkwell::IntPredicate::EQ,
            tag,
            complete_tag,
            "wrapper_project_is_complete",
        )?;
        let dispatch_bb = self
            .codegen
            .context
            .append_basic_block(self.function, "wrapper_project_dispatch");
        self.codegen
            .builder
            .build_conditional_branch(is_complete, complete_bb, dispatch_bb)?;

        self.codegen.builder.position_at_end(dispatch_bb);
        self.codegen
            .builder
            .build_switch(tag, unmatched_bb, &switch_cases)?;

        self.codegen.builder.position_at_end(complete_bb);
        let payload = self.lower_wrapper_complete_payload(
            projection.complete().payload_source(),
            owner_layout,
            owner_step,
        )?;
        let projected = self
            .codegen
            .refactor_build_step_complete(wrapper_layout, payload)?;
        self.codegen.builder.build_return(Some(&projected))?;

        for (_, bb, owner_case, wrapper_case) in cases {
            self.codegen.builder.position_at_end(bb);
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
            let (payload, continuation) = self.codegen.refactor_extract_step_case_parts(
                owner_layout,
                owner_step,
                owner_case_layout,
                "wrapper_project_case_payload",
            )?;
            let projected = self.codegen.refactor_build_step_case(
                wrapper_layout,
                wrapper_case_layout,
                payload,
                continuation,
            )?;
            self.codegen.builder.build_return(Some(&projected))?;
        }

        self.codegen.builder.position_at_end(unmatched_bb);
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    fn lower_wrapper_complete_payload(
        &mut self,
        source: &LateLoweredSurfaceResumeWrapperCompletePayloadSource,
        owner_layout: &RefactorStepLayout<'ctx>,
        owner_step: BasicValueEnum<'ctx>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        match source {
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { .. } => {
                self.codegen.refactor_extract_step_payload(
                    owner_layout,
                    owner_step,
                    owner_layout.complete_variant(),
                    "wrapper_project_complete_payload",
                )
            }
            LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(source) => {
                self.lower_completion_payload(source)
            }
        }
    }

    fn try_return_wrapper_complete_from_handle_completion(
        &mut self,
        state: &LateLoweredState,
        target: StateId,
    ) -> Result<bool, LlvmEmitError> {
        let Some(projection) = self.return_projection else {
            return Ok(false);
        };
        let LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
            site_id, ..
        } = projection.underlying_route().publication()
        else {
            return Ok(false);
        };

        let mut matched_payload_source = None;
        for candidate in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id: state_site,
                contract,
                ..
            } = candidate.terminator()
            else {
                continue;
            };
            if state_site != site_id {
                continue;
            }
            match contract.state_region(state.state_id()) {
                LateLoweredHandleStateRegion::Body if target == contract.body_complete_target() => {
                    let source = contract.body_completion_payload_source().ok_or_else(|| {
                        frontend_error(format!(
                            "refactor wrapper completion projection 找不到 site{} 的 published body completion payload source",
                            site_id.as_u32()
                        ))
                    })?;
                    matched_payload_source = Some(source);
                    break;
                }
                LateLoweredHandleStateRegion::Arm {
                    handled_case: region_case,
                    arm_ordinal: region_ordinal,
                } if target == contract.arm_complete_target() => {
                    let LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(
                        payload_source,
                    ) = projection.complete().payload_source()
                    else {
                        return Ok(false);
                    };
                    let arm = contract
                        .handled_arms()
                        .iter()
                        .find(|arm| {
                            arm.arm_ordinal() == region_ordinal
                                && arm.handled_case() == region_case
                        })
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "refactor wrapper completion projection 找不到 site{} arm#{} case c{} 的 published arm contract",
                                site_id.as_u32(),
                                region_ordinal,
                                region_case.as_u32()
                            ))
                        })?;
                    if !same_completion_payload_source_ignoring_span(
                        arm.completion_payload_source(),
                        payload_source,
                    ) {
                        return Err(frontend_error(format!(
                            "refactor wrapper completion projection payload source drift: published={payload_source:?}, arm={:?}",
                            arm.completion_payload_source()
                        )));
                    }
                    matched_payload_source = Some(arm.completion_payload_source());
                    break;
                }
                _ => continue,
            }
        }

        let Some(payload_source) = matched_payload_source else {
            return Ok(false);
        };
        let wrapper_layout = self
            .abi
            .step_layout(projection.wrapper_step_schema())
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor wrapper projection 缺少 wrapper step schema s{} layout",
                    projection.wrapper_step_schema().as_u32()
                ))
            })?;
        let payload = self.lower_completion_payload(payload_source)?;
        let projected = self
            .codegen
            .refactor_build_step_complete(wrapper_layout, payload)?;
        self.sync_frame_slots_from_locals()?;
        self.codegen.builder.build_return(Some(&projected))?;
        Ok(true)
    }

    fn try_return_handle_completion_from_resume_entry(
        &mut self,
        state: &LateLoweredState,
        target: StateId,
    ) -> Result<bool, LlvmEmitError> {
        if self.handle_completion_mode != RefactorHandleCompletionMode::ReturnFromFunction {
            return Ok(false);
        }
        let mut matched_payload_source = None;
        for candidate in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } = candidate.terminator()
            else {
                continue;
            };
            let is_surface_resume_handle = self
                .surface_resume_handle_sites
                .as_ref()
                .is_some_and(|sites| sites.contains(site_id));
            if let Some(surface_handle_sites) = &self.surface_resume_handle_sites
                && !surface_handle_sites.contains(site_id)
            {
                continue;
            }
            if contract.needs_completion_state() && !is_surface_resume_handle {
                continue;
            }
            let payload_source = match contract.state_region(state.state_id()) {
                LateLoweredHandleStateRegion::Body if target == contract.body_complete_target() => {
                    contract.body_completion_payload_source().ok_or_else(|| {
                        frontend_error(format!(
                            "refactor resume entry handle body st{} 缺少 body completion payload source",
                            state.state_id().as_u32()
                        ))
                    })?
                }
                LateLoweredHandleStateRegion::Arm {
                    handled_case,
                    arm_ordinal,
                } if target == contract.arm_complete_target() => contract
                    .handled_arms()
                    .iter()
                    .find(|arm| {
                        arm.arm_ordinal() == arm_ordinal && arm.handled_case() == handled_case
                    })
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor resume entry handle arm st{} 缺少 arm#{} case c{} completion payload source",
                            state.state_id().as_u32(),
                            arm_ordinal,
                            handled_case.as_u32()
                        ))
                    })?
                    .completion_payload_source(),
                _ => continue,
            };
            if matched_payload_source
                .replace(payload_source.clone())
                .is_some()
            {
                return Err(frontend_error(format!(
                    "refactor resume entry state st{} -> st{} 命中多个 handle completion return contract",
                    state.state_id().as_u32(),
                    target.as_u32(),
                )));
            }
        }
        let Some(payload_source) = matched_payload_source else {
            return Ok(false);
        };
        self.return_handle_completion_payload(payload_source)
    }

    fn return_handle_completion_payload(
        &mut self,
        owner_payload_source: LateLoweredCompletionPayloadSource,
    ) -> Result<bool, LlvmEmitError> {
        if let Some(projection) = self.return_projection {
            let wrapper_layout = self
                .abi
                .step_layout(projection.wrapper_step_schema())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor wrapper projection 缺少 wrapper step schema s{} layout",
                        projection.wrapper_step_schema().as_u32()
                    ))
                })?;
            let payload = match projection.complete().payload_source() {
                LateLoweredSurfaceResumeWrapperCompletePayloadSource::OwnerComplete { .. } => self
                    .lower_completion_payload_as(
                        &owner_payload_source,
                        wrapper_layout.complete_variant().payload_source_ty(),
                    )?,
                LateLoweredSurfaceResumeWrapperCompletePayloadSource::WrapperPayload(source) => {
                    self.lower_completion_payload_as(
                        source,
                        wrapper_layout.complete_variant().payload_source_ty(),
                    )?
                }
            };
            let payload = self.complete_payload_or_default(wrapper_layout, payload)?;
            let projected = self
                .codegen
                .refactor_build_step_complete(wrapper_layout, payload)?;
            self.sync_frame_slots_from_locals()?;
            self.codegen.builder.build_return(Some(&projected))?;
        } else {
            let payload = self.lower_completion_payload_as(
                &owner_payload_source,
                self.step_layout.complete_variant().payload_source_ty(),
            )?;
            let payload = self.complete_payload_or_default(self.step_layout, payload)?;
            let step = self
                .codegen
                .refactor_build_step_complete(self.step_layout, payload)?;
            self.return_step(step)?;
        }
        Ok(true)
    }

    fn complete_payload_or_default(
        &mut self,
        step_layout: &RefactorStepLayout<'ctx>,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        if payload.is_some() || step_layout.complete_variant().payload_is_elided() {
            return Ok(payload);
        }
        let payload_ty = step_layout.complete_variant().payload_source_ty();
        let payload_cg =
            self.codegen
                .cg_ty_of(payload_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "refactor default Complete payload type",
                    at: self.mir_fun.span.into(),
                })?;
        Ok(self
            .codegen
            .default_value(self.mir_fun.span, payload_cg)?
            .value)
    }

    fn try_route_handle_completion_goto(
        &mut self,
        state: &LateLoweredState,
        target: StateId,
    ) -> Result<bool, LlvmEmitError> {
        let Some(action) = self.handle_goto_action(state.state_id(), target)? else {
            return Ok(false);
        };
        match action {
            RefactorHandleGotoRuntimeAction::BeginCompletion(action) => {
                self.begin_handle_pending_completion(action, None)?;
            }
            RefactorHandleGotoRuntimeAction::FinishFinally(finally) => {
                self.finish_handle_finally_completion(finally)?;
            }
        }
        Ok(true)
    }

    fn handle_goto_action(
        &self,
        state_id: StateId,
        target: StateId,
    ) -> Result<Option<RefactorHandleGotoRuntimeAction>, LlvmEmitError> {
        let mut matched = None;
        for candidate in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } = candidate.terminator()
            else {
                continue;
            };
            let layout =
                self.abi
                    .handle_dispatch_layout(self.abi_step_schema, *site_id, contract)?;
            let action = match contract.state_region(state_id) {
                LateLoweredHandleStateRegion::Body if target == contract.body_complete_target() => {
                    self.handle_begin_completion_action(layout, *site_id)?
                        .map(RefactorHandleGotoRuntimeAction::BeginCompletion)
                }
                LateLoweredHandleStateRegion::Arm { .. }
                    if target == contract.arm_complete_target() =>
                {
                    self.handle_begin_completion_action(layout, *site_id)?
                        .map(RefactorHandleGotoRuntimeAction::BeginCompletion)
                }
                LateLoweredHandleStateRegion::Finally
                    if Some(target) == contract.finally_complete_target() =>
                {
                    Some(RefactorHandleGotoRuntimeAction::FinishFinally(
                        self.handle_finally_runtime(layout, *site_id)?,
                    ))
                }
                _ => None,
            };
            let Some(action) = action else {
                continue;
            };
            if matched.replace(action).is_some() {
                return Err(frontend_error(format!(
                    "refactor state st{} -> st{} 命中多个 HandleDispatch completion contract",
                    state_id.as_u32(),
                    target.as_u32()
                )));
            }
        }
        Ok(matched)
    }

    fn handle_begin_completion_action(
        &self,
        layout: &super::types::RefactorHandleDispatchLayout,
        site_id: SiteId,
    ) -> Result<Option<RefactorHandlePendingCompletionRuntime>, LlvmEmitError> {
        let contract = layout.lowered_contract();
        if !contract.needs_completion_state() {
            return Ok(None);
        }
        let completion = self.handle_completion_mode.pending_completion();
        let completion_tag_value = layout.completion_tag_value(completion).ok_or_else(|| {
            frontend_error(format!(
                "refactor HandleDispatch site{} 缺少 completion tag {:?}",
                site_id.as_u32(),
                completion
            ))
        })?;
        let finally_state = handle_finally_state(contract).ok_or_else(|| {
            frontend_error(format!(
                "refactor HandleDispatch site{} 需要 completion state 但缺少 finally region",
                site_id.as_u32()
            ))
        })?;
        Ok(Some(RefactorHandlePendingCompletionRuntime {
            site_id,
            completion,
            completion_tag_value,
            completion_tag_field_index: layout.completion_tag_field_index(),
            finally_state,
            payload_transport: None,
        }))
    }

    fn handle_boundary_action(
        &self,
        boundary_id: BoundaryId,
        case_tag: CaseTag,
    ) -> Result<Option<RefactorHandleBoundaryRuntimeAction>, LlvmEmitError> {
        self.handle_boundary_action_excluding(boundary_id, case_tag, None)
    }

    fn handle_boundary_action_excluding(
        &self,
        boundary_id: BoundaryId,
        case_tag: CaseTag,
        excluded_site: Option<SiteId>,
    ) -> Result<Option<RefactorHandleBoundaryRuntimeAction>, LlvmEmitError> {
        let mut matched = None::<(usize, RefactorHandleBoundaryRuntimeAction)>;
        for state in self.callable.state_graph().states() {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id, contract, ..
            } = state.terminator()
            else {
                continue;
            };
            if excluded_site.is_some_and(|excluded| excluded == *site_id) {
                continue;
            }
            let Some(routing) = contract.boundary_routing(boundary_id) else {
                continue;
            };
            let Some(case) = routing.case_routing(case_tag) else {
                continue;
            };
            let layout =
                self.abi
                    .handle_dispatch_layout(self.abi_step_schema, *site_id, contract)?;
            let action = match case.action() {
                LateLoweredHandleBoundaryCaseRoutingAction::ConsumeToArm {
                    arm_state,
                    arm_ordinal,
                    ..
                } => {
                    let arm = layout
                        .handled_arms()
                        .iter()
                        .find(|arm| arm.arm_ordinal() == arm_ordinal)
                        .ok_or_else(|| {
                            frontend_error(format!(
                                "refactor HandleDispatch site{} boundary bd{} case c{} 缺少 arm#{} layout",
                                site_id.as_u32(),
                                boundary_id.as_u32(),
                                case_tag.as_u32(),
                                arm_ordinal
                            ))
                        })?;
                    if arm.arm_state() != arm_state {
                        return Err(frontend_error(format!(
                            "refactor HandleDispatch site{} boundary bd{} case c{} arm state 漂移：routing=st{} layout=st{}",
                            site_id.as_u32(),
                            boundary_id.as_u32(),
                            case_tag.as_u32(),
                            arm_state.as_u32(),
                            arm.arm_state().as_u32()
                        )));
                    }
                    RefactorHandleBoundaryRuntimeAction::ConsumeToArm(
                        RefactorHandleConsumeArmRuntime {
                            arm_state,
                            payload_binders: arm.payload_binders().to_vec(),
                            continuation_binder: arm.continuation_binder(),
                        },
                    )
                }
                LateLoweredHandleBoundaryCaseRoutingAction::PendingCompletion { completion } => {
                    let origin = LateLoweredHandlePendingCompletionOrigin::new(
                        completion,
                        routing.boundary_id(),
                        routing.owner_state(),
                        routing.resume_state(),
                    );
                    let completion_tag_value = layout.pending_completion_origin_tag_value(origin).ok_or_else(|| {
                        frontend_error(format!(
                            "refactor HandleDispatch site{} boundary bd{} case c{} 缺少 pending completion origin tag {:?}",
                            site_id.as_u32(),
                            boundary_id.as_u32(),
                            case_tag.as_u32(),
                            origin
                        ))
                    })?;
                    let finally_state = handle_finally_state(contract).ok_or_else(|| {
                        frontend_error(format!(
                            "refactor HandleDispatch site{} boundary bd{} pending completion 缺少 finally region",
                            site_id.as_u32(),
                            boundary_id.as_u32()
                        ))
                    })?;
                    RefactorHandleBoundaryRuntimeAction::PendingCompletion(
                        RefactorHandlePendingCompletionRuntime {
                            site_id: *site_id,
                            completion,
                            completion_tag_value,
                            completion_tag_field_index: layout.completion_tag_field_index(),
                            finally_state,
                            payload_transport: layout
                                .pending_payload_transport_layout(completion)
                                .map(|transport| RefactorHandlePendingPayloadRuntime {
                                    completion: transport.completion(),
                                    payload_tuple_ty: transport.payload_tuple_ty(),
                                    frame_field_index: transport.frame_field_index(),
                                }),
                        },
                    )
                }
                LateLoweredHandleBoundaryCaseRoutingAction::EmitOutward => {
                    RefactorHandleBoundaryRuntimeAction::EmitOutward
                }
            };
            let action = if self.surface_resume_handle_sites.is_some()
                && !self.surface_resume_allows_handle_dispatch(*site_id, state.state_id())
                && !matches!(action, RefactorHandleBoundaryRuntimeAction::EmitOutward)
            {
                RefactorHandleBoundaryRuntimeAction::EmitOutward
            } else {
                action
            };
            let depth = self.handle_dispatch_nesting_depth(state.state_id());
            match (&matched, &action) {
                (None, _) => matched = Some((depth, action)),
                (Some((_, RefactorHandleBoundaryRuntimeAction::EmitOutward)), _)
                    if !matches!(action, RefactorHandleBoundaryRuntimeAction::EmitOutward) =>
                {
                    matched = Some((depth, action))
                }
                (_, RefactorHandleBoundaryRuntimeAction::EmitOutward) => {}
                (Some((matched_depth, _)), _) if depth > *matched_depth => {
                    matched = Some((depth, action))
                }
                (Some((matched_depth, _)), _) if depth < *matched_depth => {}
                (Some(_), _) => {
                    return Err(frontend_error(format!(
                        "refactor boundary bd{} case c{} 命中多个 HandleDispatch routing contract",
                        boundary_id.as_u32(),
                        case_tag.as_u32()
                    )));
                }
            }
        }
        Ok(matched.map(|(_, action)| action))
    }

    fn surface_resume_allows_handle_dispatch(
        &self,
        site_id: SiteId,
        dispatch_state: StateId,
    ) -> bool {
        let Some(surface_sites) = self.surface_resume_handle_sites.as_ref() else {
            return true;
        };
        if surface_sites.contains(&site_id) {
            return true;
        }

        self.callable.state_graph().states().iter().any(|state| {
            let LateLoweredStateTerminator::HandleDispatch {
                site_id: parent_site,
                contract,
                ..
            } = state.terminator()
            else {
                return false;
            };
            surface_sites.contains(parent_site)
                && handle_dispatch_region_implies_runtime_nesting(
                    contract.state_region(dispatch_state),
                )
        })
    }

    fn handle_dispatch_nesting_depth(&self, dispatch_state: StateId) -> usize {
        self.callable
            .state_graph()
            .states()
            .iter()
            .filter(|state| state.state_id() != dispatch_state)
            .filter_map(|state| match state.terminator() {
                LateLoweredStateTerminator::HandleDispatch { contract, .. } => Some(contract),
                _ => None,
            })
            .filter(|contract| {
                handle_dispatch_region_implies_runtime_nesting(
                    contract.state_region(dispatch_state),
                )
            })
            .count()
    }

    fn handle_finally_runtime(
        &self,
        layout: &super::types::RefactorHandleDispatchLayout,
        site_id: SiteId,
    ) -> Result<RefactorHandleFinallyRuntime, LlvmEmitError> {
        let contract = layout.lowered_contract();
        let exit_state = contract.finally_complete_target().ok_or_else(|| {
            frontend_error(format!(
                "refactor HandleDispatch site{} finally region 缺少 complete target",
                site_id.as_u32()
            ))
        })?;
        let continue_to_exit_tag = layout
            .completion_tag_value(LateLoweredHandlePendingCompletion::ContinueToExit)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor HandleDispatch site{} 缺少 ContinueToExit completion tag",
                    site_id.as_u32()
                ))
            })?;
        let return_from_function_tag = layout
            .completion_tag_value(LateLoweredHandlePendingCompletion::ReturnFromFunction)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor HandleDispatch site{} 缺少 ReturnFromFunction completion tag",
                    site_id.as_u32()
                ))
            })?;
        let return_payload_source = handle_finally_return_payload_source(contract)?;
        let mut propagate_outward = Vec::new();
        for origin in contract.pending_completion_origins() {
            let LateLoweredHandlePendingCompletion::PropagateOutward(case_tag) =
                origin.completion()
            else {
                continue;
            };
            let emission = contract.outward_emission(case_tag).ok_or_else(|| {
                frontend_error(format!(
                    "refactor HandleDispatch site{} pending outward c{} 缺少 outward emission",
                    site_id.as_u32(),
                    case_tag.as_u32()
                ))
            })?;
            let completion_tag_value = layout
                .pending_completion_origin_tag_value(*origin)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor HandleDispatch site{} 缺少 pending outward origin tag {:?}",
                        site_id.as_u32(),
                        origin
                    ))
                })?;
            let boundary_id = self.handle_boundary_for_site(site_id)?.boundary_id();
            propagate_outward.push(RefactorHandleOutwardCompletionRuntime {
                boundary_id,
                completion_tag_value,
                case_tag,
                payload_tuple_ty: emission.payload_tuple_ty(),
                resume_state: origin.resume_state(),
                payload_transport: layout
                    .pending_payload_transport_layout(origin.completion())
                    .map(|transport| RefactorHandlePendingPayloadRuntime {
                        completion: transport.completion(),
                        payload_tuple_ty: transport.payload_tuple_ty(),
                        frame_field_index: transport.frame_field_index(),
                    }),
            });
        }
        Ok(RefactorHandleFinallyRuntime {
            site_id,
            completion_tag_field_index: layout.completion_tag_field_index(),
            exit_state,
            continue_to_exit_tag,
            return_from_function_tag,
            return_payload_source,
            propagate_outward,
        })
    }

    fn begin_handle_pending_completion(
        &mut self,
        action: RefactorHandlePendingCompletionRuntime,
        payload: Option<(Option<BasicValueEnum<'ctx>>, TypeId)>,
    ) -> Result<(), LlvmEmitError> {
        if let Some(transport) = action.payload_transport {
            let Some((payload, payload_ty)) = payload else {
                return Err(frontend_error(format!(
                    "refactor HandleDispatch pending completion {:?} 需要 payload transport，但当前 completion 没有 payload",
                    action.completion
                )));
            };
            self.store_handle_pending_payload(transport, payload, payload_ty)?;
        } else if let Some((payload, payload_ty)) = payload {
            let payload_layout = self.abi.source_value_layout(payload_ty)?;
            if payload.is_some() || !payload_layout.abi().is_elided() {
                return Err(frontend_error(format!(
                    "refactor HandleDispatch pending completion {:?} 缺少 published payload transport for t{}",
                    action.completion,
                    payload_ty.as_u32()
                )));
            }
        }
        self.store_handle_completion_tag(
            action.completion_tag_field_index,
            action.completion_tag_value,
        )?;
        self.branch_to_state(action.finally_state)
    }

    fn finish_handle_finally_completion(
        &mut self,
        finally: RefactorHandleFinallyRuntime,
    ) -> Result<(), LlvmEmitError> {
        let tag = self.load_handle_completion_tag(finally.completion_tag_field_index)?;
        let function = self.function;
        let invalid_bb = self.codegen.context.append_basic_block(
            function,
            &format!("handle{}_invalid_completion", finally.site_id.as_u32()),
        );
        let (normal_tag, normal_bb) = match self.handle_completion_mode {
            RefactorHandleCompletionMode::ContinueToExit => (
                finally.continue_to_exit_tag,
                self.codegen.context.append_basic_block(
                    function,
                    &format!("handle{}_continue_exit", finally.site_id.as_u32()),
                ),
            ),
            RefactorHandleCompletionMode::ReturnFromFunction => (
                finally.return_from_function_tag,
                self.codegen.context.append_basic_block(
                    function,
                    &format!("handle{}_return_function", finally.site_id.as_u32()),
                ),
            ),
        };
        let mut cases = vec![(
            tag.get_type().const_int(u64::from(normal_tag), false),
            normal_bb,
        )];
        let mut outward_blocks = Vec::new();
        for outward in &finally.propagate_outward {
            let bb = self.codegen.context.append_basic_block(
                function,
                &format!(
                    "handle{}_propagate_c{}_st{}",
                    finally.site_id.as_u32(),
                    outward.case_tag.as_u32(),
                    outward.resume_state.as_u32()
                ),
            );
            cases.push((
                tag.get_type()
                    .const_int(u64::from(outward.completion_tag_value), false),
                bb,
            ));
            outward_blocks.push((*outward, bb));
        }
        self.codegen.builder.build_switch(tag, invalid_bb, &cases)?;

        self.codegen.builder.position_at_end(normal_bb);
        match self.handle_completion_mode {
            RefactorHandleCompletionMode::ContinueToExit => {
                self.branch_to_state(finally.exit_state)?;
            }
            RefactorHandleCompletionMode::ReturnFromFunction => {
                let payload_source = finally.return_payload_source.as_ref().ok_or_else(|| {
                    frontend_error(format!(
                        "refactor HandleDispatch site{} ReturnFromFunction 缺少 finally completion payload source",
                        finally.site_id.as_u32(),
                    ))
                })?;
                let step = self.build_complete_step_from_payload_source(payload_source)?;
                self.return_step(step)?;
            }
        }

        for (outward, bb) in outward_blocks {
            self.codegen.builder.position_at_end(bb);
            let payload = self.load_handle_pending_payload(outward.payload_transport)?;
            let boundary = self
                .callable
                .boundary_map()
                .boundary(outward.boundary_id)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor HandleDispatch site{} pending outward c{} 引用了不存在的 handle boundary bd{}",
                        finally.site_id.as_u32(),
                        outward.case_tag.as_u32(),
                        outward.boundary_id.as_u32(),
                    ))
                })?;
            let Some(LateLoweredBoundaryLowering::Handle(_)) = boundary.lowering() else {
                return Err(frontend_error(format!(
                    "refactor HandleDispatch site{} pending outward c{} 的 boundary bd{} 不是 Handle lowering",
                    finally.site_id.as_u32(),
                    outward.case_tag.as_u32(),
                    outward.boundary_id.as_u32(),
                )));
            };
            let continuation = self.create_continuation_object(outward.resume_state, None, None)?;
            self.emit_or_consume_outward_case(
                boundary,
                outward.case_tag,
                payload,
                outward.payload_tuple_ty,
                Some(continuation),
                None,
            )?;
        }

        self.codegen.builder.position_at_end(invalid_bb);
        self.codegen.builder.build_unreachable()?;
        Ok(())
    }

    fn handle_boundary_for_site(
        &self,
        site_id: SiteId,
    ) -> Result<&'a LateLoweredBoundary, LlvmEmitError> {
        let mut found = None;
        for boundary in self.callable.boundary_map().entries() {
            let LateLoweredBoundarySource::Site {
                site_id: boundary_site_id,
                kind: BoundarySiteKind::Handle,
            } = boundary.source()
            else {
                continue;
            };
            if boundary_site_id != site_id {
                continue;
            }
            let Some(LateLoweredBoundaryLowering::Handle(_)) = boundary.lowering() else {
                return Err(frontend_error(format!(
                    "refactor HandleDispatch site{} 对应 boundary bd{} 不是 Handle lowering",
                    site_id.as_u32(),
                    boundary.boundary_id().as_u32(),
                )));
            };
            if found.replace(boundary).is_some() {
                return Err(frontend_error(format!(
                    "refactor HandleDispatch site{} 命中多个 Handle boundary",
                    site_id.as_u32(),
                )));
            }
        }
        found.ok_or_else(|| {
            frontend_error(format!(
                "refactor HandleDispatch site{} 缺少对应 Handle boundary",
                site_id.as_u32(),
            ))
        })
    }

    fn build_complete_step_from_payload_source(
        &mut self,
        payload_source: &LateLoweredCompletionPayloadSource,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let payload = self.lower_completion_payload_as(
            payload_source,
            self.step_layout.complete_variant().payload_source_ty(),
        )?;
        if payload.is_none() && !self.step_layout.complete_variant().payload_is_elided() {
            return Err(frontend_error(format!(
                "refactor HandleDispatch completion payload {:?} produced no payload for non-elided Complete layout {}",
                payload_source,
                self.step_layout.complete_variant().payload_anchor_name()
            )));
        }
        self.codegen
            .refactor_build_step_complete(self.step_layout, payload)
    }

    fn store_case_payload_to_arm_binders(
        &mut self,
        binders: &[RefactorHandlePayloadBinderLayout],
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        if let [binder] = binders
            && self
                .body
                .locals
                .get(binder.local().as_u32() as usize)
                .is_some_and(|local| local.ty == payload_ty)
        {
            if let Some(raw) = payload {
                let _ = self.store_loaded_raw_local(self.mir_fun.span, binder.local(), raw)?;
                if let Some(frame_slot) = binder.frame_slot() {
                    self.store_local_to_frame_slot(binder.local(), frame_slot)?;
                }
                return Ok(());
            }
            if !self.abi.source_value_layout(payload_ty)?.abi().is_elided() {
                return Err(frontend_error(format!(
                    "refactor handle arm payload binder local{} 需要完整 non-elided payload t{}，但 boundary lowering 未提供 payload",
                    binder.local().as_u32(),
                    payload_ty.as_u32(),
                )));
            }
            return Ok(());
        }
        for binder in binders {
            let value = self.unpack_payload_field(payload, payload_ty, binder.ordinal())?;
            if let Some(raw) = value {
                let _ = self.store_loaded_raw_local(self.mir_fun.span, binder.local(), raw)?;
                if let Some(frame_slot) = binder.frame_slot() {
                    self.store_local_to_frame_slot(binder.local(), frame_slot)?;
                }
            } else if !self.payload_field_is_elided(payload_ty, binder.ordinal())? {
                return Err(frontend_error(format!(
                    "refactor handle arm payload binder local{} ordinal {} 需要 non-elided payload t{}，但 boundary lowering 未提供 payload",
                    binder.local().as_u32(),
                    binder.ordinal(),
                    payload_ty.as_u32()
                )));
            }
        }
        Ok(())
    }

    fn payload_field_is_elided(
        &self,
        payload_ty: TypeId,
        ordinal: u32,
    ) -> Result<bool, LlvmEmitError> {
        let layout = self.abi.source_value_layout(payload_ty)?;
        if layout.abi().is_elided() {
            return Ok(true);
        }
        match layout.kind() {
            RefactorSourceAbiLayoutKind::Scalar => Ok(false),
            RefactorSourceAbiLayoutKind::Tuple => layout
                .field(ordinal as usize)
                .map(|field| field.is_elided())
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor payload tuple t{} 缺少 ordinal {}",
                        payload_ty.as_u32(),
                        ordinal
                    ))
                }),
        }
    }

    fn store_gc_ref_to_binder(
        &mut self,
        binder: RefactorHandleContinuationBinderLayout,
        value: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        self.store_gc_ref_to_local(binder.local(), value)?;
        if let Some(frame_slot) = binder.frame_slot() {
            self.store_local_to_frame_slot(binder.local(), frame_slot)?;
        }
        Ok(())
    }

    fn store_handle_pending_payload(
        &mut self,
        transport: RefactorHandlePendingPayloadRuntime,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
    ) -> Result<(), LlvmEmitError> {
        if transport.payload_tuple_ty != payload_ty {
            return Err(frontend_error(format!(
                "refactor HandleDispatch pending payload transport {:?} 类型漂移：transport=t{} payload=t{}",
                transport.completion,
                transport.payload_tuple_ty.as_u32(),
                payload_ty.as_u32()
            )));
        }
        let Some(payload) = payload else {
            let payload_layout = self.abi.source_value_layout(payload_ty)?;
            if payload_layout.abi().is_elided() {
                return Ok(());
            }
            return Err(frontend_error(format!(
                "refactor HandleDispatch pending payload transport {:?} 需要 non-elided payload t{}",
                transport.completion,
                payload_ty.as_u32()
            )));
        };
        let field_ptr = self.frame_field_ptr(
            transport.frame_field_index,
            "refactor_handle_pending_payload_store_gep",
        )?;
        self.codegen.refactor_store_gc_aware_value(
            self.mir_fun.span,
            field_ptr,
            payload,
            "refactor_handle_pending_payload_store",
        )?;
        Ok(())
    }

    fn load_handle_pending_payload(
        &mut self,
        transport: Option<RefactorHandlePendingPayloadRuntime>,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let Some(transport) = transport else {
            return Ok(None);
        };
        let field_ty = self.frame_field_type(transport.frame_field_index)?;
        let field_ptr = self.frame_field_ptr(
            transport.frame_field_index,
            "refactor_handle_pending_payload_load_gep",
        )?;
        Ok(Some(self.codegen.builder.build_load(
            field_ty,
            field_ptr,
            "refactor_handle_pending_payload",
        )?))
    }

    fn store_handle_completion_tag(
        &mut self,
        field_index: u32,
        tag_value: u32,
    ) -> Result<(), LlvmEmitError> {
        let field_ty = self.frame_field_type(field_index)?;
        let BasicTypeEnum::IntType(int_ty) = field_ty else {
            return Err(frontend_error(format!(
                "refactor HandleDispatch completion tag field {field_index} 不是 integer"
            )));
        };
        let field_ptr = self.frame_field_ptr(field_index, "refactor_handle_completion_tag_gep")?;
        self.codegen
            .builder
            .build_store(field_ptr, int_ty.const_int(u64::from(tag_value), false))?;
        Ok(())
    }

    fn load_handle_completion_tag(
        &mut self,
        field_index: u32,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let field_ty = self.frame_field_type(field_index)?;
        let BasicTypeEnum::IntType(int_ty) = field_ty else {
            return Err(frontend_error(format!(
                "refactor HandleDispatch completion tag field {field_index} 不是 integer"
            )));
        };
        let field_ptr = self.frame_field_ptr(field_index, "refactor_handle_completion_tag_gep")?;
        Ok(self
            .codegen
            .builder
            .build_load(int_ty, field_ptr, "refactor_handle_completion_tag")?
            .into_int_value())
    }

    fn frame_field_type(&self, field_index: u32) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        self.frame_layout
            .llvm_ty()
            .get_field_type_at_index(field_index)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor frame layout 缺少 field index {field_index}"
                ))
            })
    }

    fn frame_field_ptr(
        &mut self,
        field_index: u32,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let frame_ptr = self.current_frame_ptr()?;
        self.codegen
            .builder
            .build_struct_gep(self.frame_layout.llvm_ty(), frame_ptr, field_index, name)
            .map_err(Into::into)
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
            .resume_payload_binding_layout(self.abi_step_schema, binding)?;
        self.store_payload_to_binding(binding, payload)
    }

    fn inject_resume_payload(
        &mut self,
        binding: LateLoweredResumePayloadBinding,
        resume_tuple_ty: TypeId,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        self.store_resume_payload_to_binding(&binding, resume_tuple_ty, payload)?;
        Ok(())
    }

    fn store_resume_payload_to_binding(
        &mut self,
        binding: &LateLoweredResumePayloadBinding,
        resume_tuple_ty: TypeId,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        if let Some(raw) = payload {
            if self.is_task_transport_tuple_ty(resume_tuple_ty)? {
                let resume_cg = self
                    .codegen
                    .cg_ty_of_mir_type(self.source_types, resume_tuple_ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor task transport resume payload type",
                        at: self.mir_fun.span.into(),
                    })?;
                let slot = self.codegen.mir_local_slot(
                    self.mir_fun.span,
                    &self.slots,
                    binding.consumer_local(),
                )?;
                if slot.cg_ty == resume_cg {
                    let value =
                        self.codegen
                            .cg_value_from_loaded(self.mir_fun.span, slot.cg_ty, raw)?;
                    self.codegen.store_local_value(
                        self.mir_fun.span,
                        slot.ptr,
                        slot.cg_ty,
                        value,
                    )?;
                } else {
                    let transport =
                        self.codegen
                            .cg_value_from_loaded(self.mir_fun.span, resume_cg, raw)?;
                    let (word, gc_ref) = self
                        .codegen
                        .split_task_transport_tuple_value(self.mir_fun.span, transport)?;
                    let decoded = self.codegen.decode_effect_transport_value(
                        self.mir_fun.span,
                        word,
                        gc_ref,
                        slot.cg_ty,
                    )?;
                    self.codegen.store_local_value(
                        self.mir_fun.span,
                        slot.ptr,
                        slot.cg_ty,
                        decoded,
                    )?;
                }
            } else {
                let _ =
                    self.store_loaded_raw_local(self.mir_fun.span, binding.consumer_local(), raw)?;
            }
        }
        if let Some(frame_slot) = binding.consumer_frame_slot() {
            self.store_local_to_frame_slot(binding.consumer_local(), frame_slot)?;
        }
        Ok(())
    }

    fn store_payload_to_binding(
        &mut self,
        binding: &LateLoweredResumePayloadBinding,
        payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<(), LlvmEmitError> {
        if let Some(raw) = payload {
            let _ =
                self.store_loaded_raw_local(self.mir_fun.span, binding.consumer_local(), raw)?;
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
                if value.value.is_none() {
                    return Err(frontend_error(format!(
                        "refactor completion payload source {:?} lowered to no runtime value",
                        source
                    )));
                }
                Ok(value.value)
            }
        }
    }

    fn lower_completion_payload_as(
        &mut self,
        payload_source: &LateLoweredCompletionPayloadSource,
        target_ty: TypeId,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        let expected = self
            .codegen
            .cg_ty_of_mir_type(self.source_types, target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "refactor completion payload target type",
                at: self.mir_fun.span.into(),
            })?;
        if expected == CgTy::Unit {
            return Ok(None);
        }
        match payload_source {
            LateLoweredCompletionPayloadSource::Unit { .. } => Ok(None),
            LateLoweredCompletionPayloadSource::Operand(source) => {
                let value = self.lower_operand_source(source)?;
                let value = self.codegen.coerce_value(
                    source.span().unwrap_or(self.mir_fun.span),
                    value,
                    expected,
                )?;
                if value.value.is_none() {
                    return Err(frontend_error(format!(
                        "refactor completion payload source {:?} coerced to no runtime value",
                        source
                    )));
                }
                Ok(value.value)
            }
        }
    }

    fn lower_operand_source(
        &mut self,
        source: &LateLoweredOperandSource,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.value_primitives().lower_operand_source(source)
    }

    fn pack_sources(
        &mut self,
        source_ty: TypeId,
        sources: &[LateLoweredOperandSource],
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        self.value_primitives()
            .pack_sources(source_ty, sources, name)
    }

    fn pack_call_args_for_invoke(
        &mut self,
        span: crate::span::Span,
        invoke_args_tuple_ty: TypeId,
        args: &[mir::CallArg],
        name: &str,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        self.value_primitives()
            .pack_call_args_for_invoke_args_tuple(span, invoke_args_tuple_ty, args, name)
    }

    fn emit_known_instance_call_step(
        &mut self,
        site_id: SiteId,
        entry: &RefactorCallableEntryLayout<'ctx>,
        args_payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let callee = self.codegen.refactor_function(entry.symbol_name())?;
        let mut args = Vec::new();
        if !entry.args_abi().is_elided() {
            args.push(
                args_payload
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor call site {} 需要 non-elided args payload",
                            site_id.as_u32()
                        ))
                    })?
                    .into(),
            );
        }
        let call = self.codegen.build_call_preserving_gc_local_roots(
            self.mir_fun.span,
            callee,
            &args,
            "refactor_call_step",
        )?;
        call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error("refactor call boundary callee 未返回 Step_F".to_string())
        })
    }

    fn lower_dynamic_call_carrier(
        &mut self,
        span: crate::span::Span,
        kind: &mir::CallKind,
        layout: &RefactorDynamicInvokeLayout<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let (operand, expected_ty) = match (kind, layout.carrier()) {
            (
                mir::CallKind::Closure { callee, .. } | mir::CallKind::FunValue { callee },
                RefactorDynamicInvokeCarrierLayout::ClosureObject(_),
            ) => (callee, CgTy::Ref),
            (
                mir::CallKind::Virtual { receiver, .. },
                RefactorDynamicInvokeCarrierLayout::VirtualReceiver(dispatch),
            ) => {
                let expected = self
                    .codegen
                    .cg_ty_of_mir_type(self.source_types, dispatch.receiver_ty())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor virtual receiver type",
                        at: span.into(),
                    })?;
                (receiver, expected)
            }
            (
                mir::CallKind::Interface { receiver, .. },
                RefactorDynamicInvokeCarrierLayout::InterfaceReceiver(dispatch),
            ) => {
                let expected = self
                    .codegen
                    .cg_ty_of_mir_type(self.source_types, dispatch.receiver_ty())
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "refactor interface receiver type",
                        at: span.into(),
                    })?;
                (receiver, expected)
            }
            _ => {
                return Err(frontend_error(format!(
                    "refactor dynamic call site {} 的 CallKind 与 published carrier layout 漂移",
                    layout.site_id().as_u32()
                )));
            }
        };
        let value = self.codegen.codegen_mir_operand_expected(
            span,
            operand,
            &self.slots,
            Some(expected_ty),
        )?;
        let value = self.codegen.coerce_value(span, value, expected_ty)?;
        let Some(BasicValueEnum::PointerValue(ptr)) = value.value else {
            return Err(frontend_error(format!(
                "refactor dynamic call site {} carrier source 不是 pointer",
                layout.site_id().as_u32()
            )));
        };
        Ok(ptr)
    }

    fn emit_refactor_dynamic_invoke_step(
        &mut self,
        layout: &RefactorDynamicInvokeLayout<'ctx>,
        carrier: PointerValue<'ctx>,
        args_payload: Option<BasicValueEnum<'ctx>>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let fn_i8 = self.load_dynamic_invoke_fn_ptr(layout, carrier)?;
        let typed_fn = self.codegen.refactor_cast_ptr(
            fn_i8,
            self.codegen.context.ptr_type(AddressSpace::default()),
            "refactor_dynamic_fn",
        )?;
        let mut args = Vec::<BasicMetadataValueEnum<'ctx>>::new();
        args.push(carrier.into());
        if !layout.args_abi().is_elided() {
            args.push(
                args_payload
                    .ok_or_else(|| {
                        frontend_error(format!(
                            "refactor dynamic call site {} 需要 non-elided args payload",
                            layout.site_id().as_u32()
                        ))
                    })?
                    .into(),
            );
        }
        let call =
            self.codegen
                .with_conservative_gc_local_root_spills(self.mir_fun.span, |codegen| {
                    Ok(codegen.builder.build_indirect_call(
                        layout.llvm_ty(),
                        typed_fn,
                        &args,
                        "refactor_dynamic_call_step",
                    )?)
                })?;
        call.try_as_basic_value().basic().ok_or_else(|| {
            frontend_error(format!(
                "refactor dynamic call site {} 未返回 Step_F",
                layout.site_id().as_u32()
            ))
        })
    }

    fn load_dynamic_invoke_fn_ptr(
        &mut self,
        layout: &RefactorDynamicInvokeLayout<'ctx>,
        carrier: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        match layout.carrier() {
            RefactorDynamicInvokeCarrierLayout::ClosureObject(closure) => {
                if closure.fn_field_index() >= closure.object_ty().count_fields() {
                    return Err(frontend_error(format!(
                        "refactor dynamic closure carrier site {} fn field {} 越界（field_count={}）",
                        layout.site_id().as_u32(),
                        closure.fn_field_index(),
                        closure.object_ty().count_fields(),
                    )));
                }
                let obj_ptr = self.codegen.refactor_cast_ptr(
                    carrier,
                    self.codegen
                        .context
                        .ptr_type(self.codegen.gc_address_space()),
                    "refactor_dynamic_closure_obj",
                )?;
                let fn_gep = self.codegen.builder.build_struct_gep(
                    closure.object_ty(),
                    obj_ptr,
                    closure.fn_field_index(),
                    "refactor_dynamic_closure_fn_gep",
                )?;
                Ok(self
                    .codegen
                    .builder
                    .build_load(
                        self.codegen.llvm_i8_ptr_type(),
                        fn_gep,
                        "refactor_dynamic_closure_fn",
                    )?
                    .into_pointer_value())
            }
            RefactorDynamicInvokeCarrierLayout::VirtualReceiver(dispatch) => {
                self.codegen.load_class_vtable_slot_fn_ptr_i8(
                    self.mir_fun.span,
                    carrier,
                    dispatch.method_slot(),
                )
            }
            RefactorDynamicInvokeCarrierLayout::InterfaceReceiver(dispatch) => {
                let interface_id = dispatch.interface_id().ok_or_else(|| {
                    frontend_error(format!(
                        "refactor dynamic interface call site {} 缺少 published interface id",
                        layout.site_id().as_u32()
                    ))
                })?;
                let fn_i8 = self.codegen.load_interface_itable_slot_fn_ptr_i8(
                    self.mir_fun.span,
                    carrier,
                    interface_id,
                    dispatch.method_slot(),
                )?;
                let is_null = self
                    .codegen
                    .builder
                    .build_is_null(fn_i8, "refactor_dynamic_itable_fn_is_null")?;
                let function = self.function;
                let ok_bb = self
                    .codegen
                    .context
                    .append_basic_block(function, "refactor_dynamic_itable_ok");
                let bad_bb = self
                    .codegen
                    .context
                    .append_basic_block(function, "refactor_dynamic_itable_null");
                self.codegen
                    .builder
                    .build_conditional_branch(is_null, bad_bb, ok_bb)?;
                self.codegen.builder.position_at_end(bad_bb);
                let exit = self.codegen.declare_libc_exit();
                let code = self.codegen.context.i32_type().const_int(7, false);
                self.codegen.builder.build_call(
                    exit,
                    &[code.into()],
                    "refactor_dynamic_itable_null_exit",
                )?;
                self.codegen.builder.build_unreachable()?;
                self.codegen.builder.position_at_end(ok_bb);
                Ok(fn_i8)
            }
        }
    }

    fn store_no_outward_call_complete(
        &mut self,
        span: crate::span::Span,
        site_id: SiteId,
        step_schema: StepSchemaId,
        step: BasicValueEnum<'ctx>,
        target: LocalId,
    ) -> Result<(), LlvmEmitError> {
        let step_layout = self.abi.step_layout(step_schema).ok_or_else(|| {
            frontend_error(format!(
                "refactor dynamic call site {} 缺少 return step schema s{} layout",
                site_id.as_u32(),
                step_schema.as_u32()
            ))
        })?;
        if !step_layout.cases().is_empty() {
            return Err(frontend_error(format!(
                "refactor source-slice dynamic call site {} return step schema s{} 含 outward case，必须走 boundary lowering",
                site_id.as_u32(),
                step_schema.as_u32()
            )));
        }
        let payload = self.codegen.refactor_extract_step_payload(
            step_layout,
            step,
            step_layout.complete_variant(),
            "refactor_dynamic_complete_payload",
        )?;
        match payload {
            Some(raw) => {
                let _ = self.store_loaded_raw_local(span, target, raw)?;
            }
            None => {
                let slot = self.codegen.mir_local_slot(span, &self.slots, target)?;
                if slot.cg_ty != CgTy::Unit && slot.cg_ty != CgTy::Never {
                    return Err(frontend_error(format!(
                        "refactor dynamic call site {} non-Unit target 缺少 Complete payload",
                        site_id.as_u32()
                    )));
                }
                let _ = self.store_local_value(span, target, CgValue::unit())?;
            }
        }
        Ok(())
    }

    fn create_continuation_object(
        &mut self,
        resume_state: StateId,
        callee_continuation: Option<PointerValue<'ctx>>,
        composition: Option<&LateLoweredCallBoundaryContinuationComposition>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let resume_state_tag = self
            .codegen
            .context
            .i32_type()
            .const_int(resume_state.as_u32() as u64, false);
        self.create_continuation_object_with_state_tag(
            Some(resume_state),
            resume_state_tag,
            callee_continuation,
            composition,
        )
    }

    fn create_continuation_object_with_state_tag(
        &mut self,
        resume_state: Option<StateId>,
        resume_state_tag: IntValue<'ctx>,
        callee_continuation: Option<PointerValue<'ctx>>,
        composition: Option<&LateLoweredCallBoundaryContinuationComposition>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if let Some(composition) = composition
            && Some(composition.caller_resume_state()) != resume_state
        {
            return Err(frontend_error(format!(
                "refactor continuation composition resume_state 漂移：object={:?} contract=st{}",
                resume_state,
                composition.caller_resume_state().as_u32(),
            )));
        }
        // A continuation extracted from a Step case may only exist as an SSA value here.
        // Root it in an explicit-frame slot before allocating the wrapper continuation, then
        // reload from that slot after the allocation safepoint before writing the composition
        // edge. Otherwise moving GC can relocate the callee continuation while the stale SSA
        // value still gets written into the wrapper.
        let callee_continuation_root = match callee_continuation {
            Some(callee_continuation) => {
                let slot = self.codegen.create_refactor_gc_root_slot(
                    self.mir_fun.span,
                    "refactor_composed_callee_root",
                )?;
                self.codegen.store_refactor_gc_root_slot(
                    self.mir_fun.span,
                    slot,
                    callee_continuation,
                    "refactor_composed_callee_root",
                )?;
                Some(slot)
            }
            None => None,
        };
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
        let cont_ptr = self.codegen.refactor_alloc_gc_struct(
            self.mir_fun.span,
            cont_layout.llvm_ty(),
            cont_layout.layout_anchor_name(),
            "refactor_cont",
        )?;
        let cont_ptr = self.root_gc_pointer(cont_ptr, "refactor_cont_root")?;
        let current_frame = self.current_frame_ptr()?;
        let frame_gc = self.codegen.refactor_cast_ptr(
            current_frame,
            self.codegen.llvm_gc_i8_ptr_type(),
            "refactor_frame_gc",
        )?;
        let frame_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_CAPTURED_FRAME,
            "refactor_cont_frame_gep",
        )?;
        self.codegen.store_gc_pointer_slot_with_write_barrier(
            self.mir_fun.span,
            frame_gep,
            frame_gc,
        )?;
        let state_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_RESUME_STATE,
            "refactor_cont_state_gep",
        )?;
        self.codegen
            .builder
            .build_store(state_gep, resume_state_tag)?;
        let one_shot_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_ONE_SHOT,
            "refactor_cont_one_shot_gep",
        )?;
        self.codegen
            .builder
            .build_store(one_shot_gep, self.codegen.context.bool_type().const_zero())?;
        let composed_gep = self.codegen.builder.build_struct_gep(
            cont_layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_COMPOSED_CALLEE,
            "refactor_cont_composed_callee_gep",
        )?;
        let composed_callee = match callee_continuation_root {
            Some(slot) => self
                .codegen
                .load_refactor_gc_root_slot(slot, "refactor_composed_callee_root")?,
            None => self.codegen.llvm_gc_i8_ptr_type().const_null(),
        };
        self.codegen.store_gc_pointer_slot_with_write_barrier(
            self.mir_fun.span,
            composed_gep,
            composed_callee,
        )?;
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
        let target_ty = self.codegen.llvm_ptr_type(self.codegen.gc_address_space());
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
            self.codegen.llvm_ptr_type(self.codegen.gc_address_space()),
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

    fn load_composed_callee_continuation(
        &mut self,
        cont_ptr: PointerValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let layout = self
            .abi
            .continuation_layout(self.callable.continuation_object())
            .unwrap();
        let gep = self.codegen.builder.build_struct_gep(
            layout.llvm_ty(),
            cont_ptr,
            CONT_FIELD_COMPOSED_CALLEE,
            "refactor_composed_callee_gep",
        )?;
        Ok(self
            .codegen
            .builder
            .build_load(
                self.codegen.llvm_gc_i8_ptr_type(),
                gep,
                "refactor_composed_callee",
            )?
            .into_pointer_value())
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
            let field_ptr = self.frame_field_ptr(field_index, "refactor_frame_slot_load_gep")?;
            let loaded = self.codegen.builder.build_load(
                self.codegen
                    .llvm_basic_type_of(self.mir_fun.span, local_slot.cg_ty)?,
                field_ptr,
                "refactor_frame_slot_load",
            )?;
            let _ = self.store_loaded_raw_local(self.mir_fun.span, local, loaded)?;
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
        let field_ptr = self.frame_field_ptr(field_index, "refactor_frame_slot_store_gep")?;
        let value = self.load_local_value(self.mir_fun.span, local)?;
        if let Some(raw) = value.value {
            self.codegen.refactor_store_gc_aware_value(
                self.mir_fun.span,
                field_ptr,
                raw,
                "refactor_frame_slot_store",
            )?;
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
        let _ = self.store_local_value(self.mir_fun.span, local, cg)?;
        Ok(())
    }

    fn unpack_payload_field(
        &mut self,
        payload: Option<BasicValueEnum<'ctx>>,
        payload_ty: TypeId,
        ordinal: u32,
    ) -> Result<Option<BasicValueEnum<'ctx>>, LlvmEmitError> {
        self.value_primitives()
            .unpack_payload_field(payload, payload_ty, ordinal)
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
    fn create_refactor_gc_root_slot(
        &mut self,
        at: crate::span::Span,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let gc_ptr_ty = self.llvm_gc_i8_ptr_type();
        let slot = self.create_entry_alloca_raw(at, name, gc_ptr_ty.into())?;
        self.builder.build_store(slot, gc_ptr_ty.const_null())?;
        self.sync_storage_slot_into_explicit_frame(at, slot, gc_ptr_ty.into(), name)?;
        self.track_gc_root_slots_for_spill_slot(at, slot, gc_ptr_ty.into(), name)?;
        Ok(slot)
    }

    fn store_refactor_gc_root_slot(
        &mut self,
        at: crate::span::Span,
        slot: PointerValue<'ctx>,
        value: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        let value =
            self.refactor_cast_ptr(value, self.llvm_gc_i8_ptr_type(), &format!("{name}_gc"))?;
        self.builder.build_store(slot, value)?;
        self.sync_storage_slot_into_explicit_frame(
            at,
            slot,
            self.llvm_gc_i8_ptr_type().into(),
            name,
        )
    }

    fn load_refactor_gc_root_slot(
        &mut self,
        slot: PointerValue<'ctx>,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        Ok(self
            .builder
            .build_load(self.llvm_gc_i8_ptr_type(), slot, &format!("{name}_reload"))?
            .into_pointer_value())
    }

    fn refactor_alloc_gc_struct(
        &mut self,
        at: crate::span::Span,
        struct_ty: StructType<'ctx>,
        layout_anchor_name: &str,
        name: &str,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let desc =
            self.get_or_create_refactor_gc_type_descriptor(at, struct_ty, layout_anchor_name)?;
        let desc_i8 = self.builder.build_pointer_cast(
            desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            &format!("{name}_type_desc"),
        )?;
        let alloc = self.declare_runtime_alloc_typed();
        let size = self.target_data.get_store_size(&struct_ty);
        let call = self.build_call_preserving_gc_local_roots(
            at,
            alloc,
            &[
                desc_i8.into(),
                self.context.i64_type().const_int(size, false).into(),
            ],
            &format!("rt_alloc_{name}"),
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or_else(|| frontend_error("scoop_alloc_typed 未返回 pointer".to_string()))?
            .into_pointer_value();
        let ptr = self.refactor_cast_ptr(raw, self.llvm_ptr_type(self.gc_address_space()), name)?;
        self.refactor_zero_gc_object_payload(struct_ty, ptr, name)?;
        Ok(ptr)
    }

    fn get_or_create_refactor_gc_type_descriptor(
        &mut self,
        at: crate::span::Span,
        struct_ty: StructType<'ctx>,
        layout_anchor_name: &str,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let global_name = format!("{layout_anchor_name}__type_desc");
        let trace_start_offset_bytes = if struct_ty.count_fields() > 1 {
            self.target_data
                .offset_of_element(&struct_ty, 1)
                .unwrap_or(0)
        } else {
            self.target_data.get_store_size(&struct_ty)
        };
        self.get_or_create_type_descriptor_global(TypeDescriptorSpec {
            at,
            global_name: &global_name,
            canonical_name: layout_anchor_name,
            obj_ty: struct_ty,
            trace_start_offset_bytes,
            parent: None,
            itable: None,
            vtable: None,
        })
    }

    fn refactor_zero_gc_object_payload(
        &mut self,
        struct_ty: StructType<'ctx>,
        ptr: PointerValue<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        for field_index in 1..struct_ty.count_fields() {
            let Some(field_ty) = struct_ty.get_field_type_at_index(field_index) else {
                return Err(frontend_error(format!(
                    "refactor GC object `{name}` 缺少 field {}",
                    field_index
                )));
            };
            let field_ptr = self.builder.build_struct_gep(
                struct_ty,
                ptr,
                field_index,
                &format!("{name}_zero_field_{field_index}"),
            )?;
            self.builder.build_store(field_ptr, field_ty.const_zero())?;
        }
        Ok(())
    }

    fn refactor_store_gc_aware_value(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        value: BasicValueEnum<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        self.refactor_store_gc_aware_basic_value(at, ptr, value.get_type(), value, name)
    }

    fn refactor_store_gc_aware_basic_value(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        value_ty: BasicTypeEnum<'ctx>,
        value: BasicValueEnum<'ctx>,
        name: &str,
    ) -> Result<(), LlvmEmitError> {
        match value_ty {
            BasicTypeEnum::PointerType(ptr_ty)
                if ptr_ty.get_address_space() == self.gc_address_space()
                    && ptr.get_type().get_address_space() == self.gc_address_space() =>
            {
                let BasicValueEnum::PointerValue(value_ptr) = value else {
                    return Err(frontend_error(format!(
                        "refactor GC-aware store `{name}` 的值不是 pointer"
                    )));
                };
                self.store_gc_pointer_slot_with_write_barrier(at, ptr, value_ptr)
            }
            BasicTypeEnum::StructType(struct_ty)
                if ptr.get_type().get_address_space() == self.gc_address_space()
                    && self.basic_type_contains_gc_ptrs(at, value_ty)? =>
            {
                let BasicValueEnum::StructValue(struct_value) = value else {
                    return Err(frontend_error(format!(
                        "refactor GC-aware store `{name}` 的值不是 struct"
                    )));
                };
                for field_index in 0..struct_ty.count_fields() {
                    let Some(field_ty) = struct_ty.get_field_type_at_index(field_index) else {
                        return Err(frontend_error(format!(
                            "refactor GC-aware store `{name}` 缺少 field {}",
                            field_index
                        )));
                    };
                    let field_ptr = self.builder.build_struct_gep(
                        struct_ty,
                        ptr,
                        field_index,
                        &format!("{name}_field_{field_index}"),
                    )?;
                    let field_value = self.builder.build_extract_value(
                        struct_value,
                        field_index,
                        &format!("{name}_field_value_{field_index}"),
                    )?;
                    self.refactor_store_gc_aware_basic_value(
                        at,
                        field_ptr,
                        field_ty,
                        field_value,
                        name,
                    )?;
                }
                Ok(())
            }
            BasicTypeEnum::ArrayType(_) if self.basic_type_contains_gc_ptrs(at, value_ty)? => {
                Err(frontend_error(format!(
                    "refactor GC-aware store `{name}` 尚未发布 array payload root/write-barrier contract"
                )))
            }
            _ => {
                self.builder.build_store(ptr, value)?;
                Ok(())
            }
        }
    }

    pub(super) fn refactor_cast_ptr(
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

    pub(super) fn refactor_build_step_complete(
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
                    "refactor Step variant tag {} ({}) 需要 payload，但 lowering 未提供",
                    tag,
                    variant.payload_anchor_name()
                ))
            })?;
            let expected_payload_ty = variant
                .payload_ty()
                .get_field_type_at_index(next_field)
                .ok_or_else(|| {
                    frontend_error(format!(
                        "refactor Step variant tag {} ({}) 缺少 payload field#{} layout",
                        tag,
                        variant.payload_anchor_name(),
                        next_field
                    ))
                })?;
            if payload.get_type() != expected_payload_ty {
                return Err(frontend_error(format!(
                    "refactor Step variant tag {} ({}) payload field#{} type drift: expected {:?}, got {:?}",
                    tag,
                    variant.payload_anchor_name(),
                    next_field,
                    expected_payload_ty,
                    payload.get_type()
                )));
            }
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

    pub(super) fn refactor_extract_step_payload(
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

fn resume_packing_method_is_reachable(
    program: &LateLoweredProgram,
    interface_id: ResumeInterfaceId,
    case_tag: CaseTag,
) -> bool {
    program.continuation_objects().iter().any(|object| {
        object.implemented_packings().contains(&interface_id)
            && object.methods().iter().any(|method| {
                method.packing_interface_id() == interface_id
                    && method.case_tag() == case_tag
                    && matches!(
                        method.body(),
                        LateLoweredContinuationResumeBody::ResumeCapturedState { .. }
                    )
            })
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

fn boundary_source_consumption(
    boundary: &LateLoweredBoundary,
) -> Option<LateLoweredBoundarySourceConsumption> {
    match boundary.lowering()? {
        LateLoweredBoundaryLowering::Call(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::ClassCtor(lowering) => Some(lowering.source_consumption()),
        LateLoweredBoundaryLowering::Perform(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::Resume(lowering) => {
            Some(lowering.operand_contract().source_consumption())
        }
        LateLoweredBoundaryLowering::RuntimeError(_) => None,
        LateLoweredBoundaryLowering::Handle(_) => None,
    }
}

fn handle_finally_state(
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
) -> Option<StateId> {
    let mut states = contract.state_regions().iter().filter_map(|entry| {
        matches!(entry.region(), LateLoweredHandleStateRegion::Finally).then_some(entry.state_id())
    });
    let first = states.next()?;
    if states.next().is_some() {
        None
    } else {
        Some(first)
    }
}

fn handle_finally_return_payload_source(
    contract: &crate::effect_lowered::ir::LateLoweredHandleDispatchContract,
) -> Result<Option<LateLoweredCompletionPayloadSource>, LlvmEmitError> {
    if !contract.needs_completion_state() {
        return Ok(None);
    }
    if let Some(source) = contract.body_completion_payload_source().cloned() {
        return Ok(Some(source));
    }
    let mut published = None;
    for arm in contract.handled_arms() {
        let candidate = arm.completion_payload_source();
        match &published {
            Some(existing) if same_completion_payload_source_ignoring_span(existing, candidate) => {
            }
            Some(existing) => {
                return Err(frontend_error(format!(
                    "refactor HandleDispatch finally ReturnFromFunction completion payload source 歧义：body/previous={existing:?} arm={candidate:?}"
                )));
            }
            None => published = Some(candidate.clone()),
        }
    }
    Ok(published)
}

fn boundary_complete_result_local(boundary: &LateLoweredBoundary) -> Option<LocalId> {
    match boundary.lowering()? {
        LateLoweredBoundaryLowering::Call(lowering) => Some(lowering.result_local()),
        LateLoweredBoundaryLowering::ClassCtor(lowering) => Some(lowering.result_local()),
        LateLoweredBoundaryLowering::Resume(lowering) => Some(lowering.result_local()),
        LateLoweredBoundaryLowering::Perform(_)
        | LateLoweredBoundaryLowering::RuntimeError(_)
        | LateLoweredBoundaryLowering::Handle(_) => None,
    }
}

fn completion_payload_source_is_local(
    source: &LateLoweredCompletionPayloadSource,
    local: LocalId,
) -> bool {
    matches!(
        source,
        LateLoweredCompletionPayloadSource::Operand(operand)
            if matches!(operand.value(), LateLoweredOperandValueSource::Local(source_local) if *source_local == local)
    )
}

fn completion_payload_local_pair(
    binding_source: &LateLoweredCompletionPayloadSource,
    return_source: &LateLoweredCompletionPayloadSource,
) -> Option<(LocalId, LocalId)> {
    let LateLoweredCompletionPayloadSource::Operand(binding_operand) = binding_source else {
        return None;
    };
    let LateLoweredCompletionPayloadSource::Operand(return_operand) = return_source else {
        return None;
    };
    if binding_operand.source_ty() != return_operand.source_ty() {
        return None;
    }
    let LateLoweredOperandValueSource::Local(binding_local) = binding_operand.value() else {
        return None;
    };
    let LateLoweredOperandValueSource::Local(return_local) = return_operand.value() else {
        return None;
    };
    Some((*binding_local, *return_local))
}

fn validate_callable_entry_layout(
    layout: &RefactorCallableLayout<'_>,
) -> Result<(), LlvmEmitError> {
    let direct = layout.direct_entry();
    let dynamic = layout.dynamic_entry();
    if direct.invoke_args_tuple_ty() != dynamic.invoke_args_tuple_ty()
        || direct.param_count() != dynamic.param_count()
        || direct.args_abi().is_elided() != dynamic.args_abi().is_elided()
        || direct.return_step_schema() != dynamic.return_step_schema()
        || direct.return_step_schema() != layout.step_schema()
    {
        return Err(frontend_error(format!(
            "refactor callable `{}` entry ABI contract 漂移：direct=(args=t{}, params={}, elided={}, return=s{}) dynamic=(args=t{}, params={}, elided={}, return=s{}) layout_step=s{}",
            layout.root_fqn(),
            direct.invoke_args_tuple_ty().as_u32(),
            direct.param_count(),
            direct.args_abi().is_elided(),
            direct.return_step_schema().as_u32(),
            dynamic.invoke_args_tuple_ty().as_u32(),
            dynamic.param_count(),
            dynamic.args_abi().is_elided(),
            dynamic.return_step_schema().as_u32(),
            layout.step_schema().as_u32(),
        )));
    }
    Ok(())
}

fn validate_plain_callable_layout(
    callable: &LateLoweredCallable,
    layout: &RefactorPlainCallableLayout<'_>,
) -> Result<(), LlvmEmitError> {
    let plain = callable.plain_abi().ok_or_else(|| {
        frontend_error(format!(
            "refactor plain callable `{}` 缺少 plain ABI handoff",
            callable.root_fqn()
        ))
    })?;
    let entry = layout.direct_entry();
    if layout.root_fqn() != callable.root_fqn()
        || entry.function_ty() != plain.function_ty()
        || entry.param_tys() != plain.param_tys()
        || entry.return_ty() != plain.return_ty()
    {
        return Err(frontend_error(format!(
            "refactor plain callable `{}` ABI contract 漂移：layout_root=`{}` function_ty=t{} return=t{} params={:?} handoff_function=t{} handoff_return=t{} handoff_params={:?}",
            callable.root_fqn(),
            layout.root_fqn(),
            entry.function_ty().as_u32(),
            entry.return_ty().as_u32(),
            entry.param_tys(),
            plain.function_ty().as_u32(),
            plain.return_ty().as_u32(),
            plain.param_tys(),
        )));
    }
    Ok(())
}

fn validate_plain_body_slices(
    root_fqn: &str,
    plain: &LateLoweredPlainCallable,
    body: &mir::Body,
) -> Result<BTreeMap<mir::BasicBlockId, LateLoweredPlainBodySlice>, LlvmEmitError> {
    if plain.body_slices().len() != body.blocks.len() {
        return Err(frontend_error(format!(
            "refactor plain callable `{root_fqn}` 的 body_slices 数量({}) 与 MIR block 数量({}) 不一致",
            plain.body_slices().len(),
            body.blocks.len(),
        )));
    }
    let mut slices = BTreeMap::new();
    for slice in plain.body_slices() {
        let block = body
            .blocks
            .get(slice.block_id().as_u32() as usize)
            .ok_or_else(|| {
                frontend_error(format!(
                    "refactor plain callable `{root_fqn}` 的 source slice 指向缺失 bb{}",
                    slice.block_id().as_u32()
                ))
            })?;
        if slice.start_statement_index() != 0
            || slice.end_statement_index() as usize != block.stmts.len()
            || !slice.includes_terminator()
        {
            return Err(frontend_error(format!(
                "refactor plain callable `{root_fqn}` 的 bb{} source slice 不是完整 ordinary block：slice=[{}..{}) includes_terminator={} stmt_count={}",
                slice.block_id().as_u32(),
                slice.start_statement_index(),
                slice.end_statement_index(),
                slice.includes_terminator(),
                block.stmts.len(),
            )));
        }
        if slices.insert(slice.block_id(), *slice).is_some() {
            return Err(frontend_error(format!(
                "refactor plain callable `{root_fqn}` 重复发布 bb{} source slice",
                slice.block_id().as_u32()
            )));
        }
    }
    Ok(slices)
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

fn same_completion_payload_source_ignoring_span(
    left: &LateLoweredCompletionPayloadSource,
    right: &LateLoweredCompletionPayloadSource,
) -> bool {
    match (left, right) {
        (
            LateLoweredCompletionPayloadSource::Unit {
                complete_ty: left_ty,
            },
            LateLoweredCompletionPayloadSource::Unit {
                complete_ty: right_ty,
            },
        ) => left_ty == right_ty,
        (
            LateLoweredCompletionPayloadSource::Operand(left),
            LateLoweredCompletionPayloadSource::Operand(right),
        ) => left.source_ty() == right.source_ty() && left.value() == right.value(),
        _ => false,
    }
}

fn refactor_source_layout_component_count(layout: &RefactorSourceAbiLayout<'_>) -> usize {
    if layout.abi().is_elided() {
        return 0;
    }
    match layout.kind() {
        RefactorSourceAbiLayoutKind::Scalar => 1,
        RefactorSourceAbiLayoutKind::Tuple => layout
            .fields()
            .iter()
            .map(|field| field.source_index() as usize + 1)
            .max()
            .unwrap_or(0),
    }
}

fn handle_dispatch_region_implies_runtime_nesting(region: LateLoweredHandleStateRegion) -> bool {
    // `Exit` 只是前一个 handle 的收尾落点，不代表当前 dispatch 仍在它的动态作用域内。
    !matches!(
        region,
        LateLoweredHandleStateRegion::OutsideHandle | LateLoweredHandleStateRegion::Exit
    )
}

fn frontend_error(message: String) -> LlvmEmitError {
    LlvmEmitError::Frontend { message }
}

#[cfg(test)]
mod tests {
    #[test]
    fn refactor_llvm_call_lowering_uses_published_call_targets() {
        let body = include_str!("body.rs");

        assert!(body.contains("RefactorCallTargetQuery::KnownInstance"));
        assert!(body.contains("RefactorCallTargetQuery::DynamicInvoke"));
        assert!(body.contains("emit_known_instance_call_step"));
        assert!(body.contains("emit_refactor_dynamic_invoke_step"));
        assert!(!body.contains(concat!("尚未支持 dynamic ", "invoke ", "body call site")));
    }

    #[test]
    fn refactor_llvm_dynamic_invoke_lowering_uses_carrier_layouts() {
        let body = include_str!("body.rs");

        assert!(body.contains("RefactorDynamicInvokeCarrierLayout::ClosureObject"));
        assert!(body.contains("RefactorDynamicInvokeCarrierLayout::VirtualReceiver"));
        assert!(body.contains("RefactorDynamicInvokeCarrierLayout::InterfaceReceiver"));
        assert!(body.contains("load_dynamic_invoke_fn_ptr"));
        assert!(body.contains("build_indirect_call"));
        assert!(!body.contains(concat!("codegen_mir_", "direct_call(")));
    }

    #[test]
    fn refactor_llvm_boundary_lowering_covers_published_boundary_categories() {
        let body = include_str!("body.rs");

        assert!(body.contains("LateLoweredBoundaryLowering::Call(lowering)"));
        assert!(body.contains("LateLoweredBoundaryLowering::Perform(lowering)"));
        assert!(body.contains("LateLoweredBoundaryLowering::Resume(lowering)"));
        assert!(body.contains("LateLoweredBoundaryLowering::RuntimeError(lowering)"));
        assert!(body.contains("LateLoweredBoundaryLowering::Handle(lowering)"));
        assert!(body.contains("lower_runtime_error_boundary"));
        assert!(body.contains("lower_handle_boundary"));
        assert!(!body.contains(concat!(
            "primary boundary bd{} 不是 ",
            "Call/Perform/Resume"
        )));
    }

    #[test]
    fn refactor_llvm_runtime_error_case_uses_ordinary_step_payload() {
        let body = include_str!("body.rs");

        assert!(body.contains("scoop.core.RuntimeError.ContinuationAlreadyResumed"));
        assert!(body.contains("emit_or_consume_outward_case"));
        assert!(body.contains("lower_runtime_error_boundary_payload"));
        assert!(!body.contains(concat!(
            "RuntimeError(_) | ",
            "LateLoweredBoundaryLowering::Handle(_)"
        )));
    }

    #[test]
    fn refactor_llvm_handle_dispatch_lowering_uses_published_protocol() {
        let body = include_str!("body.rs");

        assert!(body.contains("handle_dispatch_layout"));
        assert!(body.contains("handle_boundary_action"));
        assert!(body.contains("try_route_handle_completion_goto"));
        assert!(body.contains("try_return_wrapper_complete_from_handle_completion"));
        assert!(body.contains("body_completion_payload_source"));
        assert!(!body.contains(concat!("local_handle_", "consumption")));
    }

    #[test]
    fn refactor_llvm_handle_pending_completion_uses_published_transport() {
        let body = include_str!("body.rs");

        assert!(body.contains("LateLoweredHandlePendingCompletion::ContinueToExit"));
        assert!(body.contains("LateLoweredHandlePendingCompletion::ReturnFromFunction"));
        assert!(body.contains("LateLoweredHandlePendingCompletion::PropagateOutward"));
        assert!(body.contains("pending_payload_transport_layout"));
        assert!(body.contains("store_handle_pending_payload"));
        assert!(body.contains("load_handle_pending_payload"));
        assert!(body.contains("store_handle_completion_tag"));
        assert!(body.contains("load_handle_completion_tag"));
    }

    #[test]
    fn refactor_llvm_continuation_protocol_uses_published_resume_contracts() {
        let body = include_str!("body.rs");

        assert!(body.contains("load_frame_from_continuation"));
        assert!(body.contains("restore_frame_slots_to_locals"));
        assert!(body.contains("load_continuation_resume_state"));
        assert!(body.contains("store_continuation_one_shot"));
        assert!(body.contains("project_owner_step_to_wrapper"));
        assert!(body.contains("lower_abandon_terminator"));
        assert!(body.contains("lower_resume_unwind_terminator"));
    }

    #[test]
    fn refactor_llvm_double_resume_runtime_error_uses_ordinary_step() {
        let body = include_str!("body.rs");

        assert!(body.contains("emit_double_resume_runtime_error"));
        assert!(body.contains("double_resume_runtime_error_case"));
        assert!(body.contains("lower_runtime_error_boundary_payload"));
        assert!(body.contains("scoop.core.RuntimeError.ContinuationAlreadyResumed"));
        assert!(body.contains("create_continuation_object_with_state_tag"));
        assert!(!body.contains("self.codegen.builder.position_at_end(double_resume_bb);\n        self.codegen.builder.build_unreachable()?;"));
    }

    #[test]
    fn refactor_llvm_body_verifier_checks_published_contracts() {
        let body = include_str!("body.rs");

        assert!(body.contains("verify_body_contract"));
        assert!(body.contains("verify_state_graph_contract"));
        assert!(body.contains("verify_frame_contract"));
        assert!(body.contains("verify_boundary_contracts"));
        assert!(body.contains("verify_boundary_source_consumption"));
        assert!(body.contains("call_local_runtime_error_contract"));
    }

    #[test]
    fn refactor_llvm_source_classification_verifier_rejects_unsupported() {
        let body = include_str!("body.rs");

        assert!(body.contains("verify_source_statement_classification"));
        assert!(body.contains("source statement classified unsupported"));
        assert!(body.contains("explicit elide/skip contract"));
        assert!(!body.contains(concat!("Unsupported { .. } => ", "Ok(())")));
    }

    #[test]
    fn refactor_llvm_resume_unwind_lowering_consumes_published_contract() {
        let body = include_str!("body.rs");

        assert!(body.contains("verify_resume_unwind_contract"));
        assert!(body.contains("verify_resume_unwind_source"));
        assert!(body.contains("resume_unwind_cleanup_origin"));
        assert!(body.contains("verify_resume_unwind_handle_contract"));
        assert!(body.contains("pending-completion contract"));
        assert!(!body.contains(concat!("placeholder ", "unreachable")));
    }

    #[test]
    fn refactor_llvm_runtime_error_lowering_materializes_payload() {
        let body = include_str!("body.rs");

        assert!(body.contains("emit_local_runtime_error_terminal"));
        assert!(body.contains("materialize_runtime_error_fatal_payload"));
        assert!(body.contains("refactor_local_runtime_error_payload"));
        assert!(body.contains("consumed_runtime_error_case"));
        assert!(body.contains("source_ty_is_runtime_error"));
        assert!(body.contains("copy_boundary_complete_to_handle_return_payload"));
        assert!(!body.contains(concat!("null_", "payload")));
    }

    #[test]
    fn refactor_llvm_gc_roots_allocates_refactor_objects_with_typed_gc() {
        let body = include_str!("body.rs");

        assert!(body.contains("refactor_alloc_gc_struct"));
        assert!(body.contains("declare_runtime_alloc_typed"));
        assert!(body.contains("get_or_create_refactor_gc_type_descriptor"));
        assert!(body.contains("create_refactor_gc_root_slot"));
        assert!(body.contains("store_refactor_gc_root_slot"));
        assert!(body.contains("store_gc_pointer_slot_with_write_barrier"));
        assert!(!body.contains(concat!("declare_libc_", "malloc")));
    }

    #[test]
    fn refactor_llvm_stackmap_keeps_refactor_path_on_explicit_roots() {
        let body = include_str!("body.rs");

        assert!(body.contains("build_call_preserving_gc_local_roots"));
        assert!(body.contains("with_conservative_gc_local_root_spills"));
        assert!(body.contains("current_frame_ptr"));
        assert!(body.contains("load_refactor_gc_root_slot"));
        assert!(!body.contains(concat!("statepoint", "-example")));
        assert!(!body.contains(concat!("llvm.experimental.", "stackmap")));
    }

    #[test]
    fn refactor_llvm_dropped_continuation_keeps_drop_state_terminal() {
        let body = include_str!("body.rs");

        assert!(body.contains("verify_abandon_contract"));
        assert!(body.contains("lower_abandon_terminator"));
        assert!(body.contains("published drop_state"));
        assert!(body.contains("no\n        // remaining source-level computation is resumed"));
        assert!(!body.contains(concat!("cleanup ", "hook")));
        assert!(!body.contains(concat!("finish finally", " from drop")));
    }

    #[test]
    fn refactor_llvm_managed_abi_boundary_rejects_refactor_effect_carriers() {
        let body = include_str!("body.rs");
        let value = include_str!("value.rs");

        assert!(value.contains("extern_funs.contains_key"));
        assert!(value.contains("codegen_mir_direct_call"));
        assert!(body.contains("RefactorContinuationSurfaceResumeLayout"));
        assert!(body.contains("RefactorDynamicInvokeLayout"));
        assert!(!body.contains(concat!("emit_extern_", "native_call")));
        assert!(!body.contains(concat!("scoop_effect_", "handler_stack")));
        assert!(!body.contains(concat!("scoop_effect_", "outcome")));
    }

    #[test]
    fn refactor_llvm_plain_local_effect_control_uses_published_handoff() {
        let body = include_str!("body.rs");
        let layout = include_str!("layout.rs");
        let value = include_str!("value.rs");

        assert!(body.contains("plain.local_effect_control().is_some()"));
        assert!(body.contains("emit_plain_direct"));
        assert!(body.contains("RefactorCallableReturnMode::Plain"));
        assert!(body.contains("P5 handoff 应保证 NoOutward body 的 case 被本地 handle/catch 消费"));
        assert!(layout.contains("has_control_body()"));
        assert!(layout.contains("__scoop_refactor_plain_source_main"));
        assert!(value.contains("scoop.core.ToString"));
        assert!(!body.contains(concat!("codegen_mir_", "statement")));
    }
}
