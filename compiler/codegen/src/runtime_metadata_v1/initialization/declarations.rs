use super::*;

pub(super) enum CallableEntryRoleV1 {
    ManagedUnit,
    StartupGateway,
}

pub(super) fn require_callable_entry<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    plan: StrongInitializationCallableRefPlanV1,
    role: CallableEntryRoleV1,
) -> Result<FunctionValue<'ctx>, CodegenError> {
    let request = plan.entry_symbol();
    let linkage = super::super::registration_identity::definition_linkage(request)?;
    let symbol = request.symbol();
    if llvm.get_global(symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "initialization callable `{symbol}` collides with an LLVM global"
        )));
    }
    let function = llvm.get_function(symbol.as_str()).ok_or_else(|| {
        CodegenError(format!(
            "initialization callable `{symbol}` is not declared in the LLVM module"
        ))
    })?;
    let expected = match role {
        CallableEntryRoleV1::ManagedUnit => context.void_type().fn_type(&[], false),
        CallableEntryRoleV1::StartupGateway => context.i32_type().fn_type(&[], false),
    };
    let expected_linkage = if function.get_first_basic_block().is_some() {
        linkage
    } else {
        Linkage::External
    };
    if function.get_linkage() != expected_linkage
        || function.as_global_value().get_unnamed_address() != UnnamedAddress::None
        || function.get_type() != expected
    {
        return Err(CodegenError(format!(
            "initialization callable `{symbol}` has an incompatible entry declaration with its planned linkage"
        )));
    }
    Ok(function)
}

pub(super) fn prepare_global_declaration<'ctx>(
    llvm: &LlvmModule<'ctx>,
    request: PersistentSymbolRequest,
    expected_type: StructType<'ctx>,
    kind: &str,
    require_undefined: bool,
) -> Result<Option<GlobalValue<'ctx>>, CodegenError> {
    let definition_linkage = super::super::registration_identity::definition_linkage(request)?;
    let symbol = request.symbol();
    if llvm.get_function(symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "{kind} `{symbol}` collides with an LLVM function"
        )));
    }
    let Some(global) = llvm.get_global(symbol.as_str()) else {
        return Ok(None);
    };
    let expected_linkage = if global.get_initializer().is_some() {
        definition_linkage
    } else {
        Linkage::External
    };
    if global.get_value_type() != expected_type.as_any_type_enum()
        || global.get_linkage() != expected_linkage
        || global.get_unnamed_address() != UnnamedAddress::None
    {
        return Err(CodegenError(format!(
            "{kind} `{symbol}` has an incompatible LLVM declaration"
        )));
    }
    if require_undefined && global.get_initializer().is_some() {
        return Err(CodegenError(format!(
            "{kind} `{symbol}` is already defined"
        )));
    }
    if !require_undefined && global.get_initializer().is_some() && !global.is_constant() {
        return Err(CodegenError(format!(
            "{kind} `{symbol}` has an incompatible mutable LLVM definition"
        )));
    }
    Ok(Some(global))
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
            .expect("prepare_global_declaration validates the definition linkage"),
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
