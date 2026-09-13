use super::*;

/// Declare a function with its final symbol and signature. Managed
/// functions carry the GC strategy (M9, milestone9 DESIGN 5.5):
/// `rewrite-statepoints-for-gc` rewrites the body's call sites into
/// statepoints and LLVM emits their stackmaps. Only module functions
/// get it — the runtime declarations created at call sites are not
/// managed code.
pub(crate) fn declare_function<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    structs: &StructDefs,
    enums: &EnumDefs,
    profile: ValidatedBackendProfile,
    function: &Function,
) -> Result<(), CodegenError> {
    let managed_address_space = profile.managed_address_space_contract();
    let fn_ty = abi::function_type(
        context,
        structs,
        enums,
        managed_address_space,
        &function.signature,
    )?;
    let llvm_function = llvm.add_function(function.symbol(), fn_ty, None);
    apply_persistent_function_linkage(llvm_function, function.callable_body.symbol_request())?;
    abi::apply_function_attributes(
        context,
        structs,
        enums,
        managed_address_space,
        llvm_function,
        &function.signature,
    )?;
    statepoint::configure_function(context, llvm_function, function.gc_effect, profile);
    Ok(())
}

/// Declare one trusted-core callable from its typed LIR bridge. The symbol,
/// signature and caller protocol have already been bound by lir-lower; this
/// stage only translates that declaration to LLVM.
pub(crate) fn declare_core_external_callable<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    callable: &scoop_lir::CoreExternalCallable,
) -> Result<(), CodegenError> {
    let symbol = callable.expected_symbol().symbol();
    if llvm.get_function(symbol.as_str()).is_some() {
        return Err(CodegenError(format!(
            "core external callable `{symbol}` collides with an existing declaration"
        )));
    }
    let declaration = abi::declare_or_get(
        context,
        llvm,
        structs,
        enums,
        managed_address_space,
        symbol.as_str(),
        callable.signature(),
    )?;
    declaration.set_linkage(inkwell::module::Linkage::External);
    Ok(())
}

pub(crate) fn emit_executable_entry_shim<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    builder: &inkwell::builder::Builder<'ctx>,
    module: &Module,
) -> Result<(), CodegenError> {
    const EXECUTABLE_ENTRY_SYMBOL: &str = "scoop_main";

    let entry_index = module.entry.declaration().into_u32() as usize;
    let entry = module.functions.get(entry_index).ok_or_else(|| {
        CodegenError(format!(
            "module has invalid executable entry function id {entry_index}"
        ))
    })?;
    let target = llvm.get_function(entry.symbol()).ok_or_else(|| {
        CodegenError(format!(
            "typed executable entry @{} is not declared",
            entry.symbol()
        ))
    })?;
    if llvm.get_function(EXECUTABLE_ENTRY_SYMBOL).is_some() {
        return Err(CodegenError(format!(
            "fixed executable entry symbol @{EXECUTABLE_ENTRY_SYMBOL} collides with a module declaration"
        )));
    }
    let shim = llvm.add_function(EXECUTABLE_ENTRY_SYMBOL, target.get_type(), None);
    shim.set_linkage(inkwell::module::Linkage::External);
    let block = context.append_basic_block(shim, "entry");
    builder.position_at_end(block);
    let call = builder
        .build_call(target, &[], "")
        .map_err(|error| CodegenError(format!("emit executable entry call: {error}")))?;
    call.set_tail_call_kind(inkwell::values::LLVMTailCallKind::LLVMTailCallKindMustTail);
    builder
        .build_return(None)
        .map_err(|error| CodegenError(format!("emit executable entry return: {error}")))?;
    Ok(())
}

fn apply_persistent_function_linkage(
    function: inkwell::values::FunctionValue<'_>,
    request: scoop_lir::PersistentSymbolRequest,
) -> Result<(), CodegenError> {
    use inkwell::GlobalVisibility;
    use inkwell::module::Linkage;
    use scoop_lir::LinkageClass;

    match request.linkage() {
        LinkageClass::ConeStrong => function.set_linkage(Linkage::External),
        LinkageClass::TemplateSupportHidden => {
            function.set_linkage(Linkage::External);
            function
                .as_global_value()
                .set_visibility(GlobalVisibility::Hidden);
        }
        LinkageClass::OdrWeak => function.set_linkage(Linkage::WeakODR),
        LinkageClass::RuntimeAbi => {
            return Err(CodegenError(format!(
                "persistent symbol `{}` cannot use runtime ABI linkage",
                request.symbol()
            )));
        }
    }
    Ok(())
}

pub(crate) fn c_basic_ty<'ctx>(
    context: &'ctx Context,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    ty: &scoop_lir::CType,
) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
    Ok(match ty {
        scoop_lir::CType::Integer(kind) => integer_ty(context, kind.width()).into(),
        scoop_lir::CType::Boolean => context.bool_type().into(),
        scoop_lir::CType::DataPointer { .. } | scoop_lir::CType::CodePointer { .. } => {
            ptr_ty(context).into()
        }
        scoop_lir::CType::Struct(reference) => struct_ty(
            context,
            structs,
            enums,
            managed_address_space,
            reference.definition(),
        )?
        .into(),
    })
}

#[derive(Clone, Copy)]
enum CAbiIntegerExtension {
    Sign,
    Zero,
}

impl CAbiIntegerExtension {
    const fn llvm_attribute(self) -> &'static str {
        match self {
            Self::Sign => "signext",
            Self::Zero => "zeroext",
        }
    }
}

fn c_abi_integer_extension(ty: &scoop_lir::CType) -> Option<CAbiIntegerExtension> {
    match ty {
        scoop_lir::CType::Boolean => Some(CAbiIntegerExtension::Zero),
        scoop_lir::CType::Integer(kind)
            if matches!(
                kind.width(),
                scoop_lir::IntegerWidth::W8 | scoop_lir::IntegerWidth::W16
            ) =>
        {
            Some(match kind.signedness() {
                scoop_lir::IntegerSignedness::Signed => CAbiIntegerExtension::Sign,
                scoop_lir::IntegerSignedness::Unsigned => CAbiIntegerExtension::Zero,
            })
        }
        scoop_lir::CType::Integer(_)
        | scoop_lir::CType::DataPointer { .. }
        | scoop_lir::CType::CodePointer { .. }
        | scoop_lir::CType::Struct(_) => None,
    }
}

fn add_c_abi_integer_extension(
    context: &Context,
    function: inkwell::values::FunctionValue<'_>,
    location: AttributeLoc,
    ty: &scoop_lir::CType,
) {
    let Some(extension) = c_abi_integer_extension(ty) else {
        return;
    };
    function.add_attribute(
        location,
        context.create_enum_attribute(
            Attribute::get_named_enum_kind_id(extension.llvm_attribute()),
            0,
        ),
    );
}

struct CCallbackDeclaration<'a> {
    symbol: &'a str,
    params: &'a [scoop_lir::CType],
    return_type: &'a scoop_lir::CReturnType,
}

fn declare_c_callback_trampoline<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    declaration: CCallbackDeclaration<'_>,
) -> Result<(), CodegenError> {
    let llvm_params = declaration
        .params
        .iter()
        .map(|ty| c_basic_ty(context, structs, enums, managed_address_space, ty).map(Into::into))
        .collect::<Result<Vec<BasicMetadataTypeEnum<'ctx>>, _>>()?;
    let fn_ty = match declaration.return_type {
        scoop_lir::CReturnType::Void => context.void_type().fn_type(&llvm_params, false),
        scoop_lir::CReturnType::Value(result) => {
            c_basic_ty(context, structs, enums, managed_address_space, result)?
                .fn_type(&llvm_params, false)
        }
    };
    let function = llvm.add_function(declaration.symbol, fn_ty, None);
    for (index, ty) in declaration.params.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| {
            CodegenError(format!(
                "C callback @{} has too many parameters",
                declaration.symbol
            ))
        })?;
        add_c_abi_integer_extension(context, function, AttributeLoc::Param(index), ty);
    }
    if let scoop_lir::CReturnType::Value(result) = declaration.return_type {
        add_c_abi_integer_extension(context, function, AttributeLoc::Return, result);
    }
    Ok(())
}

pub(crate) fn declare_callback_trampoline<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    callback: &scoop_lir::CallbackBridge,
) -> Result<(), CodegenError> {
    declare_c_callback_trampoline(
        context,
        llvm,
        structs,
        enums,
        managed_address_space,
        CCallbackDeclaration {
            symbol: callback.trampoline.entry().symbol(),
            params: &callback.params,
            return_type: &callback.return_type,
        },
    )
}

pub(crate) fn declare_foreign_callback_trampoline<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    callback: &scoop_lir::ForeignCallbackBridge,
) -> Result<(), CodegenError> {
    declare_c_callback_trampoline(
        context,
        llvm,
        structs,
        enums,
        managed_address_space,
        CCallbackDeclaration {
            symbol: callback.trampoline.entry().symbol(),
            params: &callback.params,
            return_type: &callback.return_type,
        },
    )
}
