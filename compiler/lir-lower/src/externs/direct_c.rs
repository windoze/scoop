use std::num::NonZeroU64;

use scoop_lir as lir;

use crate::{LoweringContext, StorageLoweringError, StorageResult, abi};

pub(super) fn classify(
    context: &LoweringContext,
    signature: &lir::CFunctionType,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::DirectCSignature> {
    let classify = |ty: &lir::CType, position| {
        let abi::ValueStorage::NonZero(value) =
            abi::classify_storage(context, ty.storage_type(), structs, enums)?
        else {
            return Err(StorageLoweringError::InvalidRepresentation(
                "canonical C values require nonzero storage",
            ));
        };
        if matches!(ty, lir::CType::Struct(_)) {
            let coercion = abi::aggregate_layout(context, value.storage_type(), structs, enums)?
                .coercion(context.target_profile(), position);
            return match coercion {
                Some(coercion) => Ok(lir::DirectCArgument::DirectParts(lir::AbiDirectParts::new(
                    value, coercion,
                )?)),
                None => Ok(indirect(context.target_profile(), value)),
            };
        }
        Ok(lir::DirectCArgument::Scalar(lir::DirectCValue {
            storage: value,
            extension: extension(ty),
        }))
    };
    let result = match &signature.return_type {
        lir::CReturnType::Void => lir::DirectCReturn::Void,
        lir::CReturnType::Value(ty) => match classify(ty, lir::AbiValuePosition::Result)? {
            lir::DirectCArgument::Scalar(value) => lir::DirectCReturn::Value(value),
            lir::DirectCArgument::DirectParts(parts) => lir::DirectCReturn::DirectParts(parts),
            lir::DirectCArgument::Indirect { value, .. } => lir::DirectCReturn::Indirect(value),
        },
    };
    let mut params = signature
        .params
        .iter()
        .map(|ty| classify(ty, lir::AbiValuePosition::Argument))
        .collect::<StorageResult<Vec<_>>>()?;
    if context.target_profile().id() != lir::TargetProfileId::DarwinAarch64 {
        let mut registers =
            lir::AbiArgumentRegisters::sysv(matches!(result, lir::DirectCReturn::Indirect(_)));
        for parameter in &mut params {
            match parameter {
                lir::DirectCArgument::Scalar(value) => registers.scalar(
                    abi::scalar_carrier(value.storage.storage_type(), enums)
                        .expect("canonical C scalars have scalar carriers"),
                ),
                lir::DirectCArgument::DirectParts(parts) => {
                    if !registers.aggregate(parts.coercion()) {
                        *parameter = indirect(context.target_profile(), parts.value().clone());
                    }
                }
                lir::DirectCArgument::Indirect { .. } => {}
            }
        }
    }
    Ok(lir::DirectCSignature { params, result })
}

fn indirect(target: lir::LirTargetProfile, value: lir::AbiValue) -> lir::DirectCArgument {
    let passing = match target.id() {
        lir::TargetProfileId::DarwinAarch64 => lir::CIndirectPassing::CallerCopy,
        lir::TargetProfileId::LinuxX86_64Gnu | lir::TargetProfileId::LinuxX86_64Musl => {
            lir::CIndirectPassing::ByValue {
                alignment: NonZeroU64::new(value.layout().alignment().get().max(8))
                    .expect("C byval storage is at least eight-byte aligned"),
            }
        }
    };
    lir::DirectCArgument::Indirect { value, passing }
}

fn extension(ty: &lir::CType) -> lir::CIntegerExtension {
    use lir::CIntegerExtension as Extension;
    match ty {
        lir::CType::Integer(kind) if kind.width().bits() < 32 => match kind.signedness() {
            lir::IntegerSignedness::Signed => Extension::Sign,
            lir::IntegerSignedness::Unsigned => Extension::Zero,
        },
        lir::CType::Boolean => Extension::Zero,
        _ => Extension::None,
    }
}
