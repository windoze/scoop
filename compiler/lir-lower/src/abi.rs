use scoop_lir as lir;
use scoop_mir as mir;

use crate::{LoweringContext, StorageResult, lir_type, safepoints};

mod leaves;
pub(crate) use leaves::{aggregate_layout, scalar_carrier};

pub(crate) enum ValueStorage {
    ZeroSized(lir::AbiZst),
    NonZero(lir::AbiValue),
}

pub(crate) fn classify_storage(
    context: &LoweringContext,
    ty: lir::LirType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<ValueStorage> {
    let (size, alignment) = safepoints::lir_size_align(context, &ty, structs, enums)?;
    let scan = safepoints::root_scan(context, &ty, structs, enums, 0)?;
    if size == 0 {
        let layout = lir::AbiZeroSizedLayout::new(alignment)?;
        return Ok(ValueStorage::ZeroSized(lir::AbiZst::new(ty, layout)?));
    }
    let layout = lir::AbiNonZeroLayout::new(size, alignment)?;
    Ok(ValueStorage::NonZero(lir::AbiValue::new(ty, layout, scan)?))
}

pub(crate) fn classify_argument(
    context: &LoweringContext,
    ty: lir::LirType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::AbiArgument> {
    classify_value(context, ty, structs, enums, lir::AbiValuePosition::Argument)
}

fn classify_value(
    context: &LoweringContext,
    ty: lir::LirType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    position: lir::AbiValuePosition,
) -> StorageResult<lir::AbiArgument> {
    Ok(match classify_storage(context, ty, structs, enums)? {
        ValueStorage::ZeroSized(value) => lir::AbiArgument::ElidedZst(value),
        ValueStorage::NonZero(value) => {
            match lir::scoop_abi_value_shape(enums, value.storage_type())? {
                lir::ScoopAbiValueShape::Scalar => lir::AbiArgument::Direct(value.into()),
                lir::ScoopAbiValueShape::Interface => lir::AbiArgument::Direct(
                    lir::AbiDirectValue::DirectParts(lir::AbiDirectParts::interface(value)?),
                ),
                lir::ScoopAbiValueShape::Aggregate => {
                    if value.layout().size().get() <= 16
                        && value.layout().alignment().get() <= 8
                        && value.scan() == &lir::RefScan::None
                        && let Some(coercion) =
                            aggregate_layout(context, value.storage_type(), structs, enums)?
                                .coercion(context.target_profile(), position)
                    {
                        lir::AbiArgument::Direct(lir::AbiDirectValue::DirectParts(
                            lir::AbiDirectParts::new(value, coercion)?,
                        ))
                    } else {
                        lir::AbiArgument::Indirect(value)
                    }
                }
            }
        }
    })
}

pub(crate) fn classify_return(
    context: &LoweringContext,
    ty: Option<lir::LirType>,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::AbiReturn> {
    let Some(ty) = ty else {
        return Ok(lir::AbiReturn::UnitVoid);
    };
    Ok(
        match classify_value(context, ty, structs, enums, lir::AbiValuePosition::Result)? {
            lir::AbiArgument::ElidedZst(value) => lir::AbiReturn::ElidedZst(value),
            lir::AbiArgument::Direct(value) => lir::AbiReturn::Direct(value),
            lir::AbiArgument::Indirect(value) => lir::AbiReturn::Indirect(value),
        },
    )
}

pub(crate) fn classify_signature(
    context: &LoweringContext,
    parameter_types: impl IntoIterator<Item = lir::LirType>,
    result_type: Option<lir::LirType>,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::ScoopAbiSignature> {
    let mut arguments = parameter_types
        .into_iter()
        .map(|ty| classify_argument(context, ty, structs, enums))
        .collect::<StorageResult<Vec<_>>>()?;
    let result = classify_return(context, result_type, structs, enums)?;
    if context.target_profile().id() != lir::TargetProfileId::DarwinAarch64 {
        let mut registers = lir::AbiArgumentRegisters::sysv(result.is_indirect());
        for argument in &mut arguments {
            match argument {
                lir::AbiArgument::Direct(lir::AbiDirectValue::Scalar(value)) => registers.scalar(
                    leaves::scalar_carrier(value.storage_type(), enums)
                        .expect("scalar ABI values have scalar carriers"),
                ),
                lir::AbiArgument::Direct(lir::AbiDirectValue::DirectParts(parts)) => {
                    if parts.coercion() == lir::AbiCoercion::interface() {
                        for part in parts.parts() {
                            registers.scalar(part.carrier());
                        }
                    } else if !registers.aggregate(parts.coercion()) {
                        *argument = lir::AbiArgument::Indirect(parts.value().clone());
                    }
                }
                lir::AbiArgument::ElidedZst(_) | lir::AbiArgument::Indirect(_) => {}
            }
        }
    }
    Ok(lir::ScoopAbiSignature::new(
        arguments,
        result,
        lir::CallingConvention::Cdecl,
    ))
}

pub(crate) fn classify_mir_signature<'a>(
    context: &LoweringContext,
    module: &mir::Module,
    parameter_types: impl IntoIterator<Item = &'a mir::Type>,
    result_type: &mir::Type,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::ScoopAbiSignature> {
    classify_signature(
        context,
        parameter_types.into_iter().map(|ty| lir_type(module, ty)),
        (result_type != &mir::Type::Unit).then(|| lir_type(module, result_type)),
        structs,
        enums,
    )
}
