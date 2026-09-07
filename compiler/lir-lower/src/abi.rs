use scoop_lir as lir;
use scoop_mir as mir;

use crate::{LoweringContext, lir_type, safepoints};

enum ClassifiedAbiValue {
    ZeroSized(lir::AbiZst),
    Direct(lir::AbiValue),
    Indirect(lir::AbiValue),
}

fn classify_value(
    context: &LoweringContext,
    ty: lir::LirType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> Result<ClassifiedAbiValue, lir::AbiLayoutError> {
    let (size, alignment) = safepoints::lir_size_align(context, &ty, structs, enums);
    if size == 0 {
        let layout = lir::AbiZeroSizedLayout::new(alignment)?;
        let value = lir::AbiZst::new(ty, layout)
            .expect("LIR type lowering never classifies void as a value");
        return Ok(ClassifiedAbiValue::ZeroSized(value));
    }

    let layout = lir::AbiNonZeroLayout::new(size, alignment)?;
    let scan = safepoints::root_scan(context, &ty, structs, enums, 0);
    let value = lir::AbiValue::new(ty, layout, scan)
        .expect("LIR type lowering never classifies void as a value");
    Ok(
        match lir::classify_non_zero_scoop_abi_value(
            context.target_profile(),
            enums,
            value.storage_type(),
        )
        .expect("LIR type lowering only classifies valid non-void value types")
        {
            lir::ScoopAbiPassing::Direct => ClassifiedAbiValue::Direct(value),
            lir::ScoopAbiPassing::Indirect => ClassifiedAbiValue::Indirect(value),
        },
    )
}

pub(crate) fn classify_argument(
    context: &LoweringContext,
    ty: lir::LirType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> lir::AbiArgument {
    match classify_value(context, ty, structs, enums)
        .expect("target-produced LIR layouts have valid size and alignment")
    {
        ClassifiedAbiValue::ZeroSized(value) => lir::AbiArgument::ElidedZst(value),
        ClassifiedAbiValue::Direct(value) => lir::AbiArgument::Direct(value),
        ClassifiedAbiValue::Indirect(value) => lir::AbiArgument::Indirect(value),
    }
}

pub(crate) fn classify_return(
    context: &LoweringContext,
    ty: Option<lir::LirType>,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> lir::AbiReturn {
    let Some(ty) = ty else {
        return lir::AbiReturn::UnitVoid;
    };
    match classify_value(context, ty, structs, enums)
        .expect("target-produced LIR layouts have valid size and alignment")
    {
        ClassifiedAbiValue::ZeroSized(value) => lir::AbiReturn::ElidedZst(value),
        ClassifiedAbiValue::Direct(value) => lir::AbiReturn::Direct(value),
        ClassifiedAbiValue::Indirect(value) => lir::AbiReturn::Indirect(value),
    }
}

pub(crate) fn classify_signature(
    context: &LoweringContext,
    parameter_types: impl IntoIterator<Item = lir::LirType>,
    result_type: Option<lir::LirType>,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> lir::ScoopAbiSignature {
    lir::ScoopAbiSignature::new(
        parameter_types
            .into_iter()
            .map(|ty| classify_argument(context, ty, structs, enums))
            .collect(),
        classify_return(context, result_type, structs, enums),
        lir::CallingConvention::Cdecl,
    )
}

pub(crate) fn classify_mir_signature<'a>(
    context: &LoweringContext,
    parameter_types: impl IntoIterator<Item = &'a mir::Type>,
    result_type: &mir::Type,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> lir::ScoopAbiSignature {
    classify_signature(
        context,
        parameter_types.into_iter().map(lir_type),
        (result_type != &mir::Type::Unit).then(|| lir_type(result_type)),
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
