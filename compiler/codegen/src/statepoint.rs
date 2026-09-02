//! LLVM statepoint rewrite policy and defensive verification.
//!
//! Typed LIR is the sole source of safepoint identity and live-root shape.
//! This module records that complete manifest, configures LLVM 22.1, and
//! rejects rewritten IR that does not match it exactly.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CStr, CString};
use std::slice;

use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::basic_block::BasicBlock;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::llvm_sys::core::{
    LLVMBuildCall2, LLVMBuildInvoke2, LLVMConstIntGetZExtValue, LLVMGetCalledValue, LLVMGetGC,
    LLVMGetIntrinsicDeclaration, LLVMGetNumArgOperands, LLVMGetOperand, LLVMGetValueName2,
    LLVMGlobalGetValueType, LLVMIsAAllocaInst, LLVMIsAConstantInt, LLVMLookupIntrinsicID,
};
use inkwell::module::Module as LlvmModule;
use inkwell::passes::PassBuilderOptions;
use inkwell::targets::TargetMachine;
use inkwell::types::{AnyType, AsTypeRef, BasicTypeEnum, FunctionType};
use inkwell::values::{
    AsValueRef, BasicMetadataValueEnum, BasicValue, BasicValueEnum, CallSiteValue, FunctionValue,
    InstructionOpcode, InstructionValue, PointerValue,
};
use la_arena::RawIdx;
use scoop_lir::GcEffect;

use crate::CodegenError;
use crate::TargetProfile;
use crate::target::ManagedAddressSpace;

mod provenance;

const GC_STRATEGY: &str = "statepoint-example";
pub(crate) const TYPED_MANAGED_POINTER_BOUNDARY_METADATA: &str =
    "scoop.typed-managed-pointer-boundary";
pub(crate) const STATEPOINT_ROOT_IDENTITY_METADATA: &str = "scoop.statepoint-root-identity";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TypedManagedPointerBoundary {
    AllocationResult,
    CardAddress,
}

impl TypedManagedPointerBoundary {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::AllocationResult => "allocation-result",
            Self::CardAddress => "card-address",
        }
    }
}

pub(crate) fn mark_root_identity(
    context: &Context,
    instruction: InstructionValue<'_>,
    safepoint: scoop_lir::SafepointId,
    source: scoop_lir::CallerRootSource,
    byte_offset: u64,
) -> Result<(), CodegenError> {
    let (source_kind, source_index) = match source {
        scoop_lir::CallerRootSource::Param(index) => (0, index),
        scoop_lir::CallerRootSource::Local(id) => (1, id.into_raw().into_u32()),
        scoop_lir::CallerRootSource::Temp(id) => (2, id.into_raw().into_u32()),
    };
    let i64_type = context.i64_type();
    instruction
        .set_metadata(
            context.metadata_node(&[
                i64_type.const_int(safepoint.get(), false).into(),
                i64_type.const_int(source_kind, false).into(),
                i64_type.const_int(source_index.into(), false).into(),
                i64_type.const_int(byte_offset, false).into(),
            ]),
            context.get_kind_id(STATEPOINT_ROOT_IDENTITY_METADATA),
        )
        .map_err(|error| {
            CodegenError(format!(
                "mark statepoint {} root identity: {error}",
                safepoint.get()
            ))
        })
}

/// Build the explicit zero-`gc-live` statepoint form required for a managed
/// invoke. RS4GC cannot represent relocation on Scoop's Itanium exceptional
/// edge, so invoke roots live in the compiler-root frame instead.
pub(crate) struct ManagedInvoke<'a, 'ctx> {
    pub(crate) callee: PointerValue<'ctx>,
    pub(crate) callee_type: FunctionType<'ctx>,
    pub(crate) call_args: &'a [BasicValueEnum<'ctx>],
    pub(crate) safepoint: scoop_lir::SafepointId,
    pub(crate) normal: BasicBlock<'ctx>,
    pub(crate) unwind: BasicBlock<'ctx>,
    pub(crate) result_type: Option<BasicTypeEnum<'ctx>>,
}

pub(crate) struct ZeroLiveCall<'a, 'ctx> {
    pub(crate) callee: PointerValue<'ctx>,
    pub(crate) callee_type: FunctionType<'ctx>,
    pub(crate) call_args: &'a [BasicValueEnum<'ctx>],
    pub(crate) safepoint: scoop_lir::SafepointId,
    pub(crate) result_type: Option<BasicTypeEnum<'ctx>>,
}

/// Build a native transition entry's statepoint directly. The entry captures
/// the frozen managed-segment anchor; current-frame roots are already
/// published in addressable caller-root storage, so allowing RS4GC to infer
/// roots would create a second competing update mechanism.
pub(crate) fn build_zero_live_call<'ctx>(
    context: &'ctx Context,
    module: &LlvmModule<'ctx>,
    builder: &Builder<'ctx>,
    call: ZeroLiveCall<'_, 'ctx>,
) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
    let ZeroLiveCall {
        callee,
        callee_type,
        call_args,
        safepoint,
        result_type,
    } = call;
    let statepoint = intrinsic_declaration(module, "llvm.experimental.gc.statepoint", &[callee])?;
    let mut arguments = zero_live_arguments(context, callee, call_args, safepoint)?;
    let operand_count = u32::try_from(arguments.len()).map_err(|_| {
        CodegenError(format!(
            "statepoint {} operand count exceeds u32::MAX",
            safepoint.get()
        ))
    })?;
    let name = CString::new("statepoint_token").expect("static name has no NUL");
    // SAFETY: the declaration is LLVM's overloaded statepoint intrinsic and
    // `zero_live_arguments` constructs its complete fixed/variable operands.
    let token = unsafe {
        LLVMBuildCall2(
            builder.as_mut_ptr(),
            LLVMGlobalGetValueType(statepoint),
            statepoint,
            arguments.as_mut_ptr(),
            operand_count,
            name.as_ptr(),
        )
    };
    let statepoint_call = unsafe { CallSiteValue::new(token) };
    configure_statepoint_call(context, statepoint_call, callee_type);
    statepoint_call.add_attribute(
        AttributeLoc::Function,
        context.create_enum_attribute(Attribute::get_named_enum_kind_id("nounwind"), 0),
    );
    build_gc_result(module, builder, token, result_type, "native_result")
}

pub(crate) fn build_managed_invoke<'ctx>(
    context: &'ctx Context,
    module: &LlvmModule<'ctx>,
    builder: &Builder<'ctx>,
    invoke: ManagedInvoke<'_, 'ctx>,
) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
    let ManagedInvoke {
        callee,
        callee_type,
        call_args,
        safepoint,
        normal,
        unwind,
        result_type,
    } = invoke;
    let statepoint = intrinsic_declaration(module, "llvm.experimental.gc.statepoint", &[callee])?;
    let mut arguments = zero_live_arguments(context, callee, call_args, safepoint)?;
    let name = CString::new("statepoint_token").expect("static name has no NUL");
    let operand_count = u32::try_from(arguments.len()).map_err(|_| {
        CodegenError(format!(
            "statepoint {} operand count exceeds u32::MAX",
            safepoint.get()
        ))
    })?;
    // SAFETY: the declaration is LLVM's overloaded statepoint intrinsic; the
    // fixed header, actual-call argument count and two zero variable sections
    // are constructed above, and both destinations belong to this function.
    let token = unsafe {
        LLVMBuildInvoke2(
            builder.as_mut_ptr(),
            LLVMGlobalGetValueType(statepoint),
            statepoint,
            arguments.as_mut_ptr(),
            operand_count,
            normal.as_mut_ptr(),
            unwind.as_mut_ptr(),
            name.as_ptr(),
        )
    };
    // Opaque pointers require the actual callee signature on operand 2.
    let statepoint_call = unsafe { CallSiteValue::new(token) };
    configure_statepoint_call(context, statepoint_call, callee_type);

    builder.position_at_end(normal);
    build_gc_result(module, builder, token, result_type, "invoke_result")
}

fn zero_live_arguments<'ctx>(
    context: &'ctx Context,
    callee: PointerValue<'ctx>,
    call_args: &[BasicValueEnum<'ctx>],
    safepoint: scoop_lir::SafepointId,
) -> Result<Vec<inkwell::llvm_sys::prelude::LLVMValueRef>, CodegenError> {
    let i32_type = context.i32_type();
    let call_argument_count = u32::try_from(call_args.len()).map_err(|_| {
        CodegenError(format!(
            "statepoint {} has more than u32::MAX call arguments",
            safepoint.get()
        ))
    })?;
    let mut arguments = vec![
        context
            .i64_type()
            .const_int(safepoint.get(), false)
            .as_value_ref(),
        i32_type.const_zero().as_value_ref(),
        callee.as_value_ref(),
        i32_type
            .const_int(u64::from(call_argument_count), false)
            .as_value_ref(),
        i32_type.const_zero().as_value_ref(),
    ];
    arguments.extend(call_args.iter().map(AsValueRef::as_value_ref));
    arguments.push(i32_type.const_zero().as_value_ref());
    arguments.push(i32_type.const_zero().as_value_ref());
    Ok(arguments)
}

fn configure_statepoint_call(
    context: &Context,
    statepoint_call: CallSiteValue<'_>,
    callee_type: FunctionType<'_>,
) {
    // Opaque pointers require the actual callee signature on operand 2.
    statepoint_call.add_attribute(
        AttributeLoc::Param(2),
        context.create_type_attribute(
            Attribute::get_named_enum_kind_id("elementtype"),
            callee_type.as_any_type_enum(),
        ),
    );
}

fn build_gc_result<'ctx>(
    module: &LlvmModule<'ctx>,
    builder: &Builder<'ctx>,
    token: inkwell::llvm_sys::prelude::LLVMValueRef,
    result_type: Option<BasicTypeEnum<'ctx>>,
    name: &str,
) -> Result<Option<BasicValueEnum<'ctx>>, CodegenError> {
    let Some(result_type) = result_type else {
        return Ok(None);
    };
    let result = intrinsic_declaration_for_type(
        module,
        "llvm.experimental.gc.result",
        result_type.as_type_ref(),
    )?;
    let mut result_arguments = [token];
    let result_name = CString::new(name).map_err(|_| {
        CodegenError("internal gc.result instruction name contains a NUL byte".to_string())
    })?;
    // SAFETY: `token` is the immediately dominating statepoint invoke and the
    // overloaded intrinsic was declared for the original call's result type.
    let value = unsafe {
        LLVMBuildCall2(
            builder.as_mut_ptr(),
            LLVMGlobalGetValueType(result),
            result,
            result_arguments.as_mut_ptr(),
            1,
            result_name.as_ptr(),
        )
    };
    Ok(Some(unsafe { BasicValueEnum::new(value) }))
}

fn intrinsic_declaration(
    module: &LlvmModule<'_>,
    name: &str,
    pointer_overloads: &[PointerValue<'_>],
) -> Result<inkwell::llvm_sys::prelude::LLVMValueRef, CodegenError> {
    let mut types = pointer_overloads
        .iter()
        .map(|value| value.get_type().as_type_ref())
        .collect::<Vec<_>>();
    intrinsic_declaration_raw(module, name, &mut types)
}

fn intrinsic_declaration_for_type(
    module: &LlvmModule<'_>,
    name: &str,
    ty: inkwell::llvm_sys::prelude::LLVMTypeRef,
) -> Result<inkwell::llvm_sys::prelude::LLVMValueRef, CodegenError> {
    intrinsic_declaration_raw(module, name, &mut [ty])
}

fn intrinsic_declaration_raw(
    module: &LlvmModule<'_>,
    name: &str,
    types: &mut [inkwell::llvm_sys::prelude::LLVMTypeRef],
) -> Result<inkwell::llvm_sys::prelude::LLVMValueRef, CodegenError> {
    // SAFETY: LLVM only reads the name bytes and overload type array during
    // this call; both slices remain alive for its duration.
    let id = unsafe { LLVMLookupIntrinsicID(name.as_ptr().cast(), name.len()) };
    if id == 0 {
        return Err(CodegenError(format!("unknown LLVM intrinsic `{name}`")));
    }
    let function = unsafe {
        LLVMGetIntrinsicDeclaration(module.as_mut_ptr(), id, types.as_mut_ptr(), types.len())
    };
    if function.is_null() {
        return Err(CodegenError(format!(
            "failed to declare LLVM intrinsic `{name}`"
        )));
    }
    Ok(function)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExpectedRoot {
    source: scoop_lir::CallerRootSource,
    byte_offset: u64,
}

impl ExpectedRoot {
    fn key(self) -> (u8, u32, u64) {
        let (kind, index) = match self.source {
            scoop_lir::CallerRootSource::Param(index) => (0, index),
            scoop_lir::CallerRootSource::Local(id) => (1, id.into_raw().into_u32()),
            scoop_lir::CallerRootSource::Temp(id) => (2, id.into_raw().into_u32()),
        };
        (kind, index, self.byte_offset)
    }
}

#[derive(Debug, PartialEq, Eq)]
enum ExpectedStatepoint {
    Relocating(Vec<ExpectedRoot>),
    NativeTransition(&'static str),
    ZeroLiveInvoke,
}

#[derive(Debug, PartialEq, Eq)]
struct ExpectedSite {
    function: String,
    block: String,
    statepoint: ExpectedStatepoint,
}

#[derive(Debug)]
pub(crate) struct ExpectedSafepoints {
    sites: BTreeMap<u64, ExpectedSite>,
    functions: BTreeMap<String, GcEffect>,
}

impl ExpectedSafepoints {
    pub(crate) fn site_count(&self) -> usize {
        self.sites.len()
    }

    pub(crate) fn root_count(&self, safepoint: u64) -> Option<usize> {
        self.sites
            .get(&safepoint)
            .map(|site| match &site.statepoint {
                ExpectedStatepoint::Relocating(roots) => roots.len(),
                ExpectedStatepoint::NativeTransition(_) | ExpectedStatepoint::ZeroLiveInvoke => 0,
            })
    }
}

pub(crate) fn expectations(module: &scoop_lir::Module) -> Result<ExpectedSafepoints, CodegenError> {
    let mut sites = BTreeMap::new();
    let mut functions = BTreeMap::new();
    for function in &module.functions {
        if functions
            .insert(function.symbol.clone(), function.gc_effect)
            .is_some()
        {
            return Err(CodegenError(format!(
                "duplicate LIR function symbol `{}` in statepoint manifest",
                function.symbol
            )));
        }
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                let expectation = match instruction {
                    scoop_lir::Instruction::ManagedPoll { site } => {
                        Some((site.safepoint, relocating_roots(&site.live)))
                    }
                    scoop_lir::Instruction::Call { site } => match site {
                        scoop_lir::CallSite::Managed(site) => {
                            Some((site.safepoint, relocating_roots(&site.live)))
                        }
                        scoop_lir::CallSite::NativeSafe(site) => Some((
                            site.safepoint,
                            ExpectedStatepoint::NativeTransition("scoop_rt_enter_native_safe"),
                        )),
                        scoop_lir::CallSite::NativeBorrowed(site) => Some((
                            site.safepoint,
                            ExpectedStatepoint::NativeTransition("scoop_rt_enter_native_borrowed"),
                        )),
                        scoop_lir::CallSite::NoGc(_) => None,
                    },
                    scoop_lir::Instruction::Invoke { site } => match site {
                        scoop_lir::InvokeSite::Managed(site) => {
                            Some((site.safepoint, ExpectedStatepoint::ZeroLiveInvoke))
                        }
                        scoop_lir::InvokeSite::NoGc(_) => None,
                    },
                    scoop_lir::Instruction::NativeGlobalLoad { safepoint, .. }
                    | scoop_lir::Instruction::NativeGlobalStore { safepoint, .. }
                    | scoop_lir::Instruction::NativeGlobalAddress { safepoint, .. } => Some((
                        *safepoint,
                        ExpectedStatepoint::NativeTransition("scoop_rt_enter_native_safe"),
                    )),
                    scoop_lir::Instruction::ArrayAlloc {
                        safepoint, live, ..
                    }
                    | scoop_lir::Instruction::ArrayClone {
                        safepoint, live, ..
                    } => Some((*safepoint, relocating_roots(live))),
                    _ => None,
                };
                let Some((safepoint, expectation)) = expectation else {
                    continue;
                };
                if function.gc_effect == GcEffect::NoGc {
                    return Err(CodegenError(format!(
                        "NoGc LIR function `{}` contains safepoint {}",
                        function.symbol,
                        safepoint.get()
                    )));
                }
                insert_expectation(
                    &mut sites,
                    &function.symbol,
                    &block.name,
                    safepoint,
                    expectation,
                )?;
            }
        }
    }
    Ok(ExpectedSafepoints { sites, functions })
}

fn relocating_roots(live: &scoop_lir::StatepointLiveSet) -> ExpectedStatepoint {
    ExpectedStatepoint::Relocating(
        live.as_slice()
            .iter()
            .flat_map(|value| {
                value.leaves.as_slice().iter().map(|leaf| ExpectedRoot {
                    source: value.source,
                    byte_offset: leaf.byte_offset,
                })
            })
            .collect(),
    )
}

fn insert_expectation(
    expected: &mut BTreeMap<u64, ExpectedSite>,
    function: &str,
    block: &str,
    id: scoop_lir::SafepointId,
    statepoint: ExpectedStatepoint,
) -> Result<(), CodegenError> {
    if expected
        .insert(
            id.get(),
            ExpectedSite {
                function: function.to_string(),
                block: block.to_string(),
                statepoint,
            },
        )
        .is_some()
    {
        return Err(CodegenError(format!(
            "duplicate image SafepointId {} in complete LIR",
            id.get()
        )));
    }
    Ok(())
}

/// Apply the LLVM GC strategy from typed LIR and the frame policy from the
/// complete target profile.
pub(crate) fn configure_function(
    context: &Context,
    function: FunctionValue<'_>,
    effect: GcEffect,
    profile: TargetProfile,
) {
    if effect == GcEffect::Managed {
        function.set_gc(GC_STRATEGY);
        function.add_attribute(
            AttributeLoc::Function,
            context.create_string_attribute("frame-pointer", profile.frame_pointer_attribute()),
        );
        function.add_attribute(
            AttributeLoc::Function,
            context.create_string_attribute(
                "disable-tail-calls",
                profile.disable_tail_calls_attribute(),
            ),
        );
    }
}

/// Scalarize aggregate root storage, build SSA values, then run RS4GC.
pub(crate) fn rewrite(
    module: &LlvmModule<'_>,
    machine: &TargetMachine,
) -> Result<(), CodegenError> {
    module
        .run_passes(
            "function(sroa,mem2reg),rewrite-statepoints-for-gc",
            machine,
            PassBuilderOptions::create(),
        )
        .map_err(|error| CodegenError(format!("rewrite-statepoints-for-gc failed: {error}")))
}

/// Verify RS4GC output against the exact LIR manifest. This is a defensive
/// compiler/toolchain check, never a source-language fallback.
pub(crate) fn verify_rewritten(
    module: &LlvmModule<'_>,
    expected: &ExpectedSafepoints,
    profile: TargetProfile,
) -> Result<(), CodegenError> {
    verify_function_policies(module, expected, profile)?;
    let managed_address_space = profile.managed_address_space_contract();

    let cast_metadata = module
        .get_context()
        .get_kind_id(TYPED_MANAGED_POINTER_BOUNDARY_METADATA);
    let root_identity_metadata = module
        .get_context()
        .get_kind_id(STATEPOINT_ROOT_IDENTITY_METADATA);
    let mut observed = BTreeMap::<u64, ObservedStatepoint<'_>>::new();
    let mut token_ids = BTreeMap::<usize, u64>::new();
    let mut relocations = Vec::<(InstructionValue<'_>, usize, u64, u64)>::new();

    for function in module.get_functions() {
        let function_name = llvm_value_name(function.as_value_ref())?;
        for block in function.get_basic_blocks() {
            for instruction in block.get_instructions() {
                provenance::verify_pointer_instruction(
                    instruction,
                    cast_metadata,
                    managed_address_space,
                    &function_name,
                )?;
                if !matches!(
                    instruction.get_opcode(),
                    InstructionOpcode::Call | InstructionOpcode::Invoke | InstructionOpcode::CallBr
                ) {
                    continue;
                }
                let raw = instruction.as_value_ref();
                // SAFETY: the opcode check above proves this is a CallBase.
                let callee = unsafe { LLVMGetCalledValue(raw) };
                if callee.is_null() {
                    continue;
                }
                let name = llvm_value_name(callee)?;
                if name.starts_with("llvm.experimental.gc.statepoint") {
                    let id = call_constant(raw, 0, "statepoint id")?;
                    let expected_site = expected.sites.get(&id).ok_or_else(|| {
                        CodegenError(format!(
                            "post-RS4GC function `{function_name}` contains unexpected SafepointId {id}"
                        ))
                    })?;
                    if expected_site.function != function_name {
                        return Err(CodegenError(format!(
                            "SafepointId {id} belongs to LIR function `{}`, but was emitted in `{function_name}`",
                            expected_site.function
                        )));
                    }
                    if expected.functions.get(&function_name) != Some(&GcEffect::Managed) {
                        return Err(CodegenError(format!(
                            "statepoint {id} is emitted outside a managed LIR function"
                        )));
                    }
                    let call_args =
                        u32::try_from(call_constant(raw, 3, "statepoint call-argument count")?)
                            .map_err(|_| {
                                CodegenError(format!(
                                    "statepoint {id} call arguments exceed u32::MAX"
                                ))
                            })?;
                    let flags = call_constant(raw, 4, "statepoint flags")?;
                    if flags != 0 {
                        return Err(CodegenError(format!(
                            "statepoint {id} carries unsupported flags {flags}"
                        )));
                    }
                    let transition_index = 5u32.checked_add(call_args).ok_or_else(|| {
                        CodegenError(format!("statepoint {id} call arguments overflow"))
                    })?;
                    let transition_count = call_constant(
                        raw,
                        transition_index,
                        "statepoint transition-argument count",
                    )?;
                    let deopt_index = transition_index
                        .checked_add(1)
                        .and_then(|index| {
                            u32::try_from(transition_count)
                                .ok()
                                .and_then(|count| index.checked_add(count))
                        })
                        .ok_or_else(|| {
                            CodegenError(format!("statepoint {id} transition arguments overflow"))
                        })?;
                    let deopt_count =
                        call_constant(raw, deopt_index, "statepoint deopt-argument count")?;
                    if transition_count != 0 || deopt_count != 0 {
                        return Err(CodegenError(format!(
                            "statepoint {id} carries transition/deopt inputs"
                        )));
                    }
                    let expected_argument_count = deopt_index.checked_add(1).ok_or_else(|| {
                        CodegenError(format!("statepoint {id} argument count overflows u32"))
                    })?;
                    let argument_count = unsafe { LLVMGetNumArgOperands(raw) };
                    if argument_count != expected_argument_count {
                        return Err(CodegenError(format!(
                            "statepoint {id} has {argument_count} intrinsic arguments, expected {expected_argument_count}"
                        )));
                    }
                    let roots = gc_live_roots(raw, id, managed_address_space)?;
                    let identities = gc_live_root_identities(&roots, root_identity_metadata, id)?;
                    verify_statepoint_shape(
                        id,
                        &expected_site.statepoint,
                        instruction,
                        &roots,
                        &identities,
                        &function_name,
                    )?;
                    if observed
                        .insert(
                            id,
                            ObservedStatepoint {
                                instruction,
                                function,
                                roots,
                                identities,
                                relocations: BTreeMap::new(),
                            },
                        )
                        .is_some()
                    {
                        return Err(CodegenError(format!(
                            "SafepointId {id} produced more than one statepoint"
                        )));
                    }
                    token_ids.insert(raw as usize, id);
                } else if name.starts_with("llvm.experimental.gc.relocate") {
                    // SAFETY: relocate has three required operands; the helper
                    // validates both index operands before extraction.
                    let token = unsafe { LLVMGetOperand(raw, 0) } as usize;
                    let base = call_constant(raw, 1, "gc.relocate base index")?;
                    let derived = call_constant(raw, 2, "gc.relocate derived index")?;
                    relocations.push((instruction, token, base, derived));
                }
            }
        }
    }

    for (instruction, token, base, derived) in relocations {
        if base != derived {
            return Err(CodegenError(format!(
                "gc.relocate uses distinct base/derived indices {base}/{derived}"
            )));
        }
        let id = token_ids.get(&token).ok_or_else(|| {
            CodegenError("gc.relocate references an unknown statepoint token".to_string())
        })?;
        let site = observed
            .get_mut(id)
            .expect("statepoint token was inserted with its id");
        let root_count = u64::try_from(site.roots.len())
            .map_err(|_| CodegenError(format!("statepoint {id} root count exceeds u64::MAX")))?;
        if base >= root_count {
            return Err(CodegenError(format!(
                "gc.relocate for statepoint {id} references root index {base}, but gc-live has {root_count} roots"
            )));
        }
        if site.relocations.insert(base, instruction).is_some() {
            return Err(CodegenError(format!(
                "statepoint {id} produced more than one gc.relocate for root index {base}"
            )));
        }
    }

    let observed_ids = observed.keys().copied().collect::<BTreeSet<_>>();
    let expected_ids = expected.sites.keys().copied().collect::<BTreeSet<_>>();
    if observed_ids != expected_ids {
        let missing = expected_ids
            .difference(&observed_ids)
            .map(|id| {
                let site = &expected.sites[id];
                format!(
                    "{id}:{}:{}:{:?}",
                    site.function, site.block, site.statepoint
                )
            })
            .collect::<Vec<_>>();
        let unexpected = observed_ids
            .difference(&expected_ids)
            .copied()
            .collect::<Vec<_>>();
        return Err(CodegenError(format!(
            "post-RS4GC statepoint ids disagree with complete LIR: missing {missing:?}, unexpected {unexpected:?}"
        )));
    }
    provenance::verify_no_derived_live_through(&observed, managed_address_space)?;
    for (id, site) in &observed {
        match &expected.sites[id].statepoint {
            ExpectedStatepoint::Relocating(_) => {}
            ExpectedStatepoint::NativeTransition(_) | ExpectedStatepoint::ZeroLiveInvoke => {
                if !site.relocations.is_empty() {
                    return Err(CodegenError(format!(
                        "zero-live statepoint {id} produced gc.relocate instructions"
                    )));
                }
                continue;
            }
        }
        for (index, identity) in site.identities.iter().enumerate() {
            let index = u64::try_from(index)
                .map_err(|_| CodegenError(format!("statepoint {id} root index overflow")))?;
            if !site.relocations.contains_key(&index) {
                return Err(CodegenError(format!(
                    "statepoint {id} root {index} ({:?}+{}) has no gc.relocate",
                    identity.source, identity.byte_offset
                )));
            }
        }
        provenance::verify_no_stale_root_uses(*id, site)?;
    }
    Ok(())
}

struct ObservedStatepoint<'ctx> {
    instruction: InstructionValue<'ctx>,
    function: FunctionValue<'ctx>,
    roots: Vec<BasicValueEnum<'ctx>>,
    identities: Vec<ExpectedRoot>,
    relocations: BTreeMap<u64, InstructionValue<'ctx>>,
}

fn verify_function_policies(
    module: &LlvmModule<'_>,
    expected: &ExpectedSafepoints,
    profile: TargetProfile,
) -> Result<(), CodegenError> {
    for (symbol, effect) in &expected.functions {
        let function = module.get_function(symbol).ok_or_else(|| {
            CodegenError(format!(
                "LIR function `{symbol}` is absent from the LLVM module"
            ))
        })?;
        let gc = gc_strategy(function)?;
        match effect {
            GcEffect::Managed => {
                if gc != GC_STRATEGY {
                    return Err(CodegenError(format!(
                        "managed function `{symbol}` has GC strategy `{gc}`, expected `{GC_STRATEGY}`"
                    )));
                }
                verify_string_attribute(
                    function,
                    symbol,
                    "frame-pointer",
                    profile.frame_pointer_attribute(),
                )?;
                verify_string_attribute(
                    function,
                    symbol,
                    "disable-tail-calls",
                    profile.disable_tail_calls_attribute(),
                )?;
            }
            GcEffect::NoGc if !gc.is_empty() => {
                return Err(CodegenError(format!(
                    "NoGc function `{symbol}` unexpectedly has GC strategy `{gc}`"
                )));
            }
            GcEffect::NoGc => {}
        }
    }
    for function in module.get_functions() {
        let gc = gc_strategy(function)?;
        if gc.is_empty() {
            continue;
        }
        let symbol = llvm_value_name(function.as_value_ref())?;
        if !expected.functions.contains_key(&symbol) {
            return Err(CodegenError(format!(
                "LLVM function `{symbol}` has an unmanifested GC strategy `{gc}`"
            )));
        }
    }
    Ok(())
}

fn gc_strategy(function: FunctionValue<'_>) -> Result<String, CodegenError> {
    // LLVMGetGC returns null when no strategy is attached. Inkwell's
    // FunctionValue::get_gc assumes a non-null pointer, so preserve the
    // absent case explicitly instead of constructing a CStr from null.
    let pointer = unsafe { LLVMGetGC(function.as_value_ref()) };
    if pointer.is_null() {
        return Ok(String::new());
    }
    unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .map(str::to_owned)
        .map_err(|error| CodegenError(format!("LLVM GC strategy is not UTF-8: {error}")))
}

fn verify_string_attribute(
    function: FunctionValue<'_>,
    symbol: &str,
    key: &str,
    expected: &str,
) -> Result<(), CodegenError> {
    let attribute = function
        .get_string_attribute(AttributeLoc::Function, key)
        .ok_or_else(|| {
            CodegenError(format!(
                "managed function `{symbol}` lacks required `{key}` attribute"
            ))
        })?;
    let actual = attribute.get_string_value().to_str().map_err(|error| {
        CodegenError(format!(
            "attribute `{key}` on `{symbol}` is not UTF-8: {error}"
        ))
    })?;
    if actual != expected {
        return Err(CodegenError(format!(
            "managed function `{symbol}` has `{key}`=`{actual}`, expected `{expected}`"
        )));
    }
    Ok(())
}

fn gc_live_roots<'ctx>(
    raw: inkwell::llvm_sys::prelude::LLVMValueRef,
    id: u64,
    managed_address_space: ManagedAddressSpace,
) -> Result<Vec<BasicValueEnum<'ctx>>, CodegenError> {
    // SAFETY: the caller only passes a statepoint CallBase instruction.
    let call = unsafe { CallSiteValue::new(raw) };
    let mut roots = None;
    for bundle in call.get_operand_bundles() {
        let tag = bundle.get_tag().map_err(|error| {
            CodegenError(format!(
                "statepoint {id} operand-bundle tag is not UTF-8: {error}"
            ))
        })?;
        if tag != "gc-live" {
            return Err(CodegenError(format!(
                "statepoint {id} carries unsupported `{tag}` operand bundle"
            )));
        }
        if roots.is_some() {
            return Err(CodegenError(format!(
                "statepoint {id} carries more than one gc-live bundle"
            )));
        }
        roots = Some(bundle.get_args().collect::<Vec<_>>());
    }
    let roots = roots.unwrap_or_default();
    let mut identities = BTreeSet::new();
    for (index, root) in roots.iter().enumerate() {
        if !provenance::is_managed_pointer(root.as_value_ref(), managed_address_space) {
            return Err(CodegenError(format!(
                "statepoint {id} gc-live root {index} is not an AS{} managed pointer",
                managed_address_space.llvm()
            )));
        }
        if root
            .as_instruction_value()
            .is_some_and(|instruction| instruction.get_opcode() == InstructionOpcode::GetElementPtr)
        {
            return Err(CodegenError(format!(
                "statepoint {id} gc-live root {index} is a derived AS{} address",
                managed_address_space.llvm()
            )));
        }
        if !identities.insert(root.as_value_ref() as usize) {
            return Err(CodegenError(format!(
                "statepoint {id} repeats one LLVM root value in gc-live"
            )));
        }
    }
    Ok(roots)
}

fn gc_live_root_identities(
    roots: &[BasicValueEnum<'_>],
    metadata_kind: u32,
    statepoint: u64,
) -> Result<Vec<ExpectedRoot>, CodegenError> {
    let mut identities = Vec::with_capacity(roots.len());
    let mut unique = BTreeSet::new();
    for (index, root) in roots.iter().enumerate() {
        let instruction = root.as_instruction_value().ok_or_else(|| {
            CodegenError(format!(
                "statepoint {statepoint} gc-live root {index} has no typed LIR identity: {root:?}"
            ))
        })?;
        if instruction.get_opcode() != InstructionOpcode::Load {
            return Err(CodegenError(format!(
                "statepoint {statepoint} gc-live root {index} is not loaded from typed root storage: {root:?}"
            )));
        }
        let storage = unsafe { LLVMGetOperand(instruction.as_value_ref(), 0) };
        if unsafe { LLVMIsAAllocaInst(storage) }.is_null() {
            return Err(CodegenError(format!(
                "statepoint {statepoint} gc-live root {index} does not use dedicated typed root storage: {root:?}"
            )));
        }
        // SAFETY: LLVMIsAAllocaInst above proves the operand is an instruction.
        let storage = unsafe { InstructionValue::new(storage) };
        let metadata = storage.get_metadata(metadata_kind).ok_or_else(|| {
            CodegenError(format!(
                "statepoint {statepoint} gc-live root {index} lacks typed LIR identity metadata: {root:?}"
            ))
        })?;
        let values = metadata.get_node_values().ok_or_else(|| {
            CodegenError(format!(
                "statepoint {statepoint} root identity metadata is not a node"
            ))
        })?;
        let [
            BasicMetadataValueEnum::IntValue(metadata_statepoint),
            BasicMetadataValueEnum::IntValue(source_kind),
            BasicMetadataValueEnum::IntValue(source_index),
            BasicMetadataValueEnum::IntValue(byte_offset),
        ] = values.as_slice()
        else {
            return Err(CodegenError(format!(
                "statepoint {statepoint} root identity metadata has an invalid shape"
            )));
        };
        let metadata_statepoint = constant_metadata_int(*metadata_statepoint, "safepoint id")?;
        if metadata_statepoint != statepoint {
            return Err(CodegenError(format!(
                "statepoint {statepoint} root {index} carries identity for statepoint {metadata_statepoint}"
            )));
        }
        let source_index =
            u32::try_from(constant_metadata_int(*source_index, "root source index")?).map_err(
                |_| {
                    CodegenError(format!(
                        "statepoint {statepoint} root source index exceeds u32::MAX"
                    ))
                },
            )?;
        let source = match constant_metadata_int(*source_kind, "root source kind")? {
            0 => scoop_lir::CallerRootSource::Param(source_index),
            1 => scoop_lir::CallerRootSource::Local(scoop_lir::LocalId::from_raw(
                RawIdx::from_u32(source_index),
            )),
            2 => scoop_lir::CallerRootSource::Temp(scoop_lir::TempId::from_raw(RawIdx::from_u32(
                source_index,
            ))),
            kind => {
                return Err(CodegenError(format!(
                    "statepoint {statepoint} root {index} carries unknown source kind {kind}"
                )));
            }
        };
        let identity = ExpectedRoot {
            source,
            byte_offset: constant_metadata_int(*byte_offset, "root byte offset")?,
        };
        if !unique.insert(identity.key()) {
            return Err(CodegenError(format!(
                "statepoint {statepoint} repeats typed root identity {identity:?}"
            )));
        }
        identities.push(identity);
    }
    Ok(identities)
}

fn constant_metadata_int(
    value: inkwell::values::IntValue<'_>,
    what: &str,
) -> Result<u64, CodegenError> {
    if unsafe { LLVMIsAConstantInt(value.as_value_ref()) }.is_null() {
        return Err(CodegenError(format!(
            "statepoint {what} metadata is not a constant integer"
        )));
    }
    Ok(unsafe { LLVMConstIntGetZExtValue(value.as_value_ref()) })
}

fn verify_statepoint_shape(
    id: u64,
    expected: &ExpectedStatepoint,
    instruction: InstructionValue<'_>,
    roots: &[BasicValueEnum<'_>],
    identities: &[ExpectedRoot],
    function: &str,
) -> Result<(), CodegenError> {
    let expected_count = match expected {
        ExpectedStatepoint::Relocating(expected) => expected.len(),
        ExpectedStatepoint::NativeTransition(_) | ExpectedStatepoint::ZeroLiveInvoke => 0,
    };
    if roots.len() != expected_count {
        // SAFETY: the statepoint shape check above has already validated the
        // fixed header, whose operand 2 is the actual callee pointer.
        let actual_callee =
            llvm_value_name(unsafe { LLVMGetOperand(instruction.as_value_ref(), 2) })?;
        return Err(CodegenError(format!(
            "statepoint {id} for `{actual_callee}` in `{function}` has a gc-live count that disagrees with complete LIR: expected {expected_count} from {expected:?}, observed {} ({roots:?})",
            roots.len(),
        )));
    }
    if let ExpectedStatepoint::NativeTransition(expected_callee) = expected {
        // SAFETY: the statepoint shape check has validated operand 2 as the
        // actual callee pointer.
        let actual_callee =
            llvm_value_name(unsafe { LLVMGetOperand(instruction.as_value_ref(), 2) })?;
        if actual_callee != *expected_callee {
            return Err(CodegenError(format!(
                "native transition statepoint {id} in `{function}` targets `{actual_callee}`, expected `{expected_callee}`"
            )));
        }
    }
    if let ExpectedStatepoint::Relocating(expected_roots) = expected {
        let expected_identities = expected_roots
            .iter()
            .copied()
            .map(ExpectedRoot::key)
            .collect::<BTreeSet<_>>();
        let observed_identities = identities
            .iter()
            .copied()
            .map(ExpectedRoot::key)
            .collect::<BTreeSet<_>>();
        if observed_identities != expected_identities {
            return Err(CodegenError(format!(
                "statepoint {id} in `{function}` has gc-live identities that disagree with complete LIR: expected {expected_roots:?}, observed {identities:?}"
            )));
        }
    } else if !identities.is_empty() {
        return Err(CodegenError(format!(
            "zero-live statepoint {id} unexpectedly carries typed root identities"
        )));
    }
    let expected_opcode = match expected {
        ExpectedStatepoint::Relocating(_) | ExpectedStatepoint::NativeTransition(_) => {
            InstructionOpcode::Call
        }
        ExpectedStatepoint::ZeroLiveInvoke => InstructionOpcode::Invoke,
    };
    if instruction.get_opcode() != expected_opcode {
        return Err(CodegenError(format!(
            "statepoint {id} has {:?} control shape, expected {expected_opcode:?}",
            instruction.get_opcode()
        )));
    }
    Ok(())
}

fn call_constant(
    raw: inkwell::llvm_sys::prelude::LLVMValueRef,
    index: u32,
    what: &str,
) -> Result<u64, CodegenError> {
    // SAFETY: CallBase argument counts and operands come from verified IR.
    let argument_count = unsafe { LLVMGetNumArgOperands(raw) };
    if index >= argument_count {
        return Err(CodegenError(format!("{what} operand is missing")));
    }
    let operand = unsafe { LLVMGetOperand(raw, index) };
    if unsafe { LLVMIsAConstantInt(operand) }.is_null() {
        return Err(CodegenError(format!("{what} is not a constant integer")));
    }
    Ok(unsafe { LLVMConstIntGetZExtValue(operand) })
}

fn llvm_value_name(raw: inkwell::llvm_sys::prelude::LLVMValueRef) -> Result<String, CodegenError> {
    let mut length = 0usize;
    // SAFETY: LLVM owns the returned byte span for the lifetime of `raw`.
    let pointer = unsafe { LLVMGetValueName2(raw, &mut length) };
    if pointer.is_null() {
        return Ok(String::new());
    }
    let bytes = unsafe { slice::from_raw_parts(pointer.cast::<u8>(), length) };
    String::from_utf8(bytes.to_vec())
        .map_err(|error| CodegenError(format!("LLVM value name is not UTF-8: {error}")))
}

#[cfg(test)]
#[path = "statepoint_tests.rs"]
mod tests;
