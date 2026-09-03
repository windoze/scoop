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
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    profile: TargetProfile,
    function: &Function,
) -> Result<(), CodegenError> {
    let fn_ty = fn_type_of(
        context,
        structs,
        enums,
        profile.managed_address_space_contract(),
        function,
    )?;
    let llvm_function = llvm.add_function(&function.symbol, fn_ty, None);
    statepoint::configure_function(context, llvm_function, function.gc_effect, profile);
    Ok(())
}

pub(crate) fn c_basic_ty<'ctx>(
    context: &'ctx Context,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    managed_address_space: ManagedAddressSpace,
    ty: &scoop_lir::CType,
) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
    Ok(match ty {
        scoop_lir::CType::Int | scoop_lir::CType::UInt => context.i64_type().into(),
        scoop_lir::CType::Boolean => context.bool_type().into(),
        scoop_lir::CType::Pointer | scoop_lir::CType::FunctionPointer { .. } => {
            ptr_ty(context).into()
        }
        scoop_lir::CType::Struct(id) => {
            struct_ty(context, structs, enums, managed_address_space, *id)?.into()
        }
        scoop_lir::CType::Unit => {
            return Err(CodegenError(
                "Unit cannot be a C callback parameter type".to_string(),
            ));
        }
    })
}

pub(crate) fn declare_callback_trampoline<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    managed_address_space: ManagedAddressSpace,
    callback: &scoop_lir::CallbackBridge,
) -> Result<(), CodegenError> {
    let params = callback
        .params
        .iter()
        .map(|ty| c_basic_ty(context, structs, enums, managed_address_space, ty).map(Into::into))
        .collect::<Result<Vec<BasicMetadataTypeEnum<'ctx>>, _>>()?;
    let fn_ty = if callback.return_type == scoop_lir::CType::Unit {
        context.void_type().fn_type(&params, false)
    } else {
        c_basic_ty(
            context,
            structs,
            enums,
            managed_address_space,
            &callback.return_type,
        )?
        .fn_type(&params, false)
    };
    llvm.add_function(&callback.trampoline_symbol, fn_ty, None);
    Ok(())
}

pub(crate) fn declare_foreign_callback_trampoline<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    structs: &Arena<StructDef>,
    enums: &Arena<EnumDef>,
    managed_address_space: ManagedAddressSpace,
    callback: &scoop_lir::ForeignCallbackBridge,
) -> Result<(), CodegenError> {
    let params = callback
        .params
        .iter()
        .map(|ty| c_basic_ty(context, structs, enums, managed_address_space, ty).map(Into::into))
        .collect::<Result<Vec<BasicMetadataTypeEnum<'ctx>>, _>>()?;
    let fn_ty = if callback.return_type == scoop_lir::CType::Unit {
        context.void_type().fn_type(&params, false)
    } else {
        c_basic_ty(
            context,
            structs,
            enums,
            managed_address_space,
            &callback.return_type,
        )?
        .fn_type(&params, false)
    };
    llvm.add_function(&callback.trampoline_symbol, fn_ty, None);
    Ok(())
}
