use super::*;

pub(super) fn require_callable_entry<'ctx>(
    llvm: &LlvmModule<'ctx>,
    plan: StrongInitializationCallableRefPlanV1,
) -> Result<FunctionValue<'ctx>, CodegenError> {
    let symbol = plan.entry_symbol().symbol();
    llvm.get_function(symbol.as_str()).ok_or_else(|| {
        CodegenError(format!(
            "initialization callable `{symbol}` is not declared in the LLVM module"
        ))
    })
}

pub(super) fn declare_global<'ctx>(
    llvm: &LlvmModule<'ctx>,
    request: PersistentSymbolRequest,
    ty: StructType<'ctx>,
    prior: Option<GlobalValue<'ctx>>,
) -> GlobalValue<'ctx> {
    prior.unwrap_or_else(|| {
        let global = llvm.add_global(ty, None, request.symbol().as_str());
        global.set_linkage(Linkage::External);
        global
    })
}

pub(super) fn define_global<'ctx>(
    llvm: &LlvmModule<'ctx>,
    request: PersistentSymbolRequest,
    ty: StructType<'ctx>,
    prior: Option<GlobalValue<'ctx>>,
    constant: bool,
    initializer: inkwell::values::BasicValueEnum<'ctx>,
) -> GlobalValue<'ctx> {
    let global = declare_global(llvm, request, ty, prior);
    global.set_constant(constant);
    global.set_initializer(&initializer);
    global.set_linkage(
        super::super::registration_identity::definition_linkage(request)
            .expect("initialization plans use definition linkage"),
    );
    global
}

fn digest_bytes<'ctx>(
    context: &'ctx Context,
    bytes: &[u8; 32],
) -> inkwell::values::ArrayValue<'ctx> {
    let i8 = context.i8_type();
    let values = bytes
        .iter()
        .map(|byte| i8.const_int(u64::from(*byte), false))
        .collect::<Vec<_>>();
    i8.const_array(&values)
}

pub(super) fn digest_value<'ctx>(
    context: &'ctx Context,
    digest_type: StructType<'ctx>,
    bytes: &[u8; 32],
) -> StructValue<'ctx> {
    digest_type.const_named_struct(&[digest_bytes(context, bytes).into()])
}
