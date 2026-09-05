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
            symbol: &callback.trampoline_symbol,
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
            symbol: &callback.trampoline_symbol,
            params: &callback.params,
            return_type: &callback.return_type,
        },
    )
}
