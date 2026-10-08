use scoop_lir as lir;
use scoop_mir as mir;

use crate::{LoweringContext, StorageResult, lir_type, safepoints};

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
    Ok(match classify_storage(context, ty, structs, enums)? {
        ValueStorage::ZeroSized(value) => lir::AbiArgument::ElidedZst(value),
        ValueStorage::NonZero(value) => match lir::classify_non_zero_scoop_abi_value(
            context.target_profile(),
            enums,
            value.storage_type(),
        )? {
            lir::ScoopAbiPassing::Direct => lir::AbiArgument::Direct(value.into()),
            lir::ScoopAbiPassing::DirectParts => lir::AbiArgument::Direct(
                lir::AbiDirectValue::DirectParts(lir::AbiDirectParts::interface(value)?),
            ),
            lir::ScoopAbiPassing::Indirect => lir::AbiArgument::Indirect(value),
        },
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
    Ok(match classify_argument(context, ty, structs, enums)? {
        lir::AbiArgument::ElidedZst(value) => lir::AbiReturn::ElidedZst(value),
        lir::AbiArgument::Direct(value) => lir::AbiReturn::Direct(value),
        lir::AbiArgument::Indirect(value) => lir::AbiReturn::Indirect(value),
    })
}

pub(crate) fn classify_signature(
    context: &LoweringContext,
    parameter_types: impl IntoIterator<Item = lir::LirType>,
    result_type: Option<lir::LirType>,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::ScoopAbiSignature> {
    Ok(lir::ScoopAbiSignature::new(
        parameter_types
            .into_iter()
            .map(|ty| classify_argument(context, ty, structs, enums))
            .collect::<StorageResult<Vec<_>>>()?,
        classify_return(context, result_type, structs, enums)?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_profile_keeps_shape_out_of_size_thresholds() {
        let profile = lir::LirTargetProfile::DARWIN_AARCH64;
        assert_eq!(
            profile.classify_scoop_abi_value(lir::ScoopAbiValueShape::Scalar),
            lir::ScoopAbiPassing::Direct
        );
        assert_eq!(
            profile.classify_scoop_abi_value(lir::ScoopAbiValueShape::Aggregate),
            lir::ScoopAbiPassing::Indirect
        );
    }
}
