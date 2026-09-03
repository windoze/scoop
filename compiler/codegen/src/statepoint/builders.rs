use super::*;

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
