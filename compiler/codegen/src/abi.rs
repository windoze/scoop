use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::context::Context;
use inkwell::module::Module as LlvmModule;
use inkwell::types::{AnyType, BasicMetadataTypeEnum, BasicType, BasicTypeEnum, FunctionType};
use inkwell::values::{CallSiteValue, FunctionValue};
use scoop_lir::{
    AbiPhysicalParameterOrigin, AbiReturn, AbiValue, EnumDefs, ScoopAbiSignature, StructDefs,
};

use crate::{CodegenError, ManagedAddressSpace, basic_ty, ptr_ty};

fn value_type<'ctx>(
    context: &'ctx Context,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    value: &AbiValue,
) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
    basic_ty(
        context,
        structs,
        enums,
        managed_address_space,
        value.storage_type(),
    )
}

pub(crate) fn direct_type<'ctx>(
    context: &'ctx Context,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    value: &scoop_lir::AbiDirectValue,
) -> Result<BasicTypeEnum<'ctx>, CodegenError> {
    match value {
        scoop_lir::AbiDirectValue::Scalar(value) => {
            value_type(context, structs, enums, managed_address_space, value)
        }
        scoop_lir::AbiDirectValue::DirectParts(parts) => Ok(context
            .struct_type(
                &parts
                    .parts()
                    .iter()
                    .map(|part| {
                        crate::pointer_ty(context, managed_address_space, part.pointer_kind).into()
                    })
                    .collect::<Vec<_>>(),
                false,
            )
            .into()),
    }
}

/// Translate one already-classified Scoop ABI signature into its physical
/// LLVM function type. No value shape is reclassified here.
pub(crate) fn function_type<'ctx>(
    context: &'ctx Context,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    signature: &ScoopAbiSignature,
) -> Result<FunctionType<'ctx>, CodegenError> {
    let parameters = signature
        .physical_parameters()
        .map(|parameter| match parameter.origin() {
            AbiPhysicalParameterOrigin::DirectArgument { .. } => value_type(
                context,
                structs,
                enums,
                managed_address_space,
                parameter.value(),
            )
            .map(Into::into),
            AbiPhysicalParameterOrigin::DirectArgumentPart { part, .. } => {
                Ok(crate::pointer_ty(context, managed_address_space, part.pointer_kind).into())
            }
            AbiPhysicalParameterOrigin::IndirectReturn
            | AbiPhysicalParameterOrigin::IndirectArgument { .. } => {
                Ok(BasicMetadataTypeEnum::from(ptr_ty(context)))
            }
        })
        .collect::<Result<Vec<_>, CodegenError>>()?;

    match signature.result() {
        AbiReturn::Direct(value) => {
            direct_type(context, structs, enums, managed_address_space, value)
                .map(|ty| ty.fn_type(&parameters, false))
        }
        AbiReturn::UnitVoid | AbiReturn::ElidedZst(_) | AbiReturn::Indirect(_) => {
            Ok(context.void_type().fn_type(&parameters, false))
        }
    }
}

fn apply_parameter_attributes(
    context: &Context,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    signature: &ScoopAbiSignature,
    parameter_offset: u32,
    mut add: impl FnMut(AttributeLoc, Attribute),
) -> Result<(), CodegenError> {
    for parameter in signature.physical_parameters() {
        let attribute_name = match parameter.origin() {
            AbiPhysicalParameterOrigin::IndirectReturn => "sret",
            AbiPhysicalParameterOrigin::IndirectArgument { .. } => "byval",
            AbiPhysicalParameterOrigin::DirectArgument { .. }
            | AbiPhysicalParameterOrigin::DirectArgumentPart { .. } => continue,
        };
        let index = u32::try_from(parameter.index())
            .ok()
            .and_then(|index| index.checked_add(parameter_offset))
            .ok_or_else(|| {
                CodegenError("Scoop ABI parameter index exceeds u32::MAX".to_string())
            })?;
        let ty = value_type(
            context,
            structs,
            enums,
            managed_address_space,
            parameter.value(),
        )?;
        add(
            AttributeLoc::Param(index),
            context.create_type_attribute(
                Attribute::get_named_enum_kind_id(attribute_name),
                ty.as_any_type_enum(),
            ),
        );
        add(
            AttributeLoc::Param(index),
            context.create_enum_attribute(
                Attribute::get_named_enum_kind_id("align"),
                parameter.value().layout().alignment().get(),
            ),
        );
    }
    Ok(())
}

pub(crate) fn apply_function_attributes(
    context: &Context,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    function: FunctionValue<'_>,
    signature: &ScoopAbiSignature,
) -> Result<(), CodegenError> {
    apply_parameter_attributes(
        context,
        structs,
        enums,
        managed_address_space,
        signature,
        0,
        |location, attribute| function.add_attribute(location, attribute),
    )
}

pub(crate) fn apply_call_attributes(
    context: &Context,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    call: CallSiteValue<'_>,
    signature: &ScoopAbiSignature,
) -> Result<(), CodegenError> {
    apply_parameter_attributes(
        context,
        structs,
        enums,
        managed_address_space,
        signature,
        0,
        |location, attribute| call.add_attribute(location, attribute),
    )
}

/// Apply wrapped-call ABI attributes to an explicitly constructed statepoint.
/// The intrinsic's five fixed operands precede the wrapped physical arguments.
pub(crate) fn apply_statepoint_attributes(
    context: &Context,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    statepoint: CallSiteValue<'_>,
    signature: &ScoopAbiSignature,
) -> Result<(), CodegenError> {
    apply_parameter_attributes(
        context,
        structs,
        enums,
        managed_address_space,
        signature,
        5,
        |location, attribute| statepoint.add_attribute(location, attribute),
    )
}

pub(crate) fn declare_or_get<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    structs: &StructDefs,
    enums: &EnumDefs,
    managed_address_space: ManagedAddressSpace,
    symbol: &str,
    signature: &ScoopAbiSignature,
) -> Result<FunctionValue<'ctx>, CodegenError> {
    let expected = function_type(context, structs, enums, managed_address_space, signature)?;
    let function = match llvm.get_function(symbol) {
        Some(function) if function.get_type() == expected => function,
        Some(_) => {
            return Err(CodegenError(format!(
                "typed target `{symbol}` disagrees with its Scoop ABI declaration"
            )));
        }
        None => llvm.add_function(symbol, expected, None),
    };
    apply_function_attributes(
        context,
        structs,
        enums,
        managed_address_space,
        function,
        signature,
    )?;
    Ok(function)
}
