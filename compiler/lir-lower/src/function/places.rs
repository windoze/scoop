use super::*;

pub(super) fn source_local(
    context: &LoweringContext,
    module: &mir::Module,
    local: &mir::Local,
    address_taken: bool,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> lir::Local {
    let storage = match abi::classify_argument(context, lir_type(&local.ty), structs, enums) {
        lir::AbiArgument::ElidedZst(representation) => {
            let value = lir::LogicalZstValue::new(
                exact_type_record(module, &local.ty).id(),
                representation,
            );
            if address_taken {
                lir::LocalStorage::AddressableZst(lir::AddressableZstPlace::new(
                    value,
                    lir::LocalPlaceLifetime::FunctionActivation,
                ))
            } else {
                lir::LocalStorage::LogicalZst(value)
            }
        }
        lir::AbiArgument::Direct(value) | lir::AbiArgument::Indirect(value) => {
            lir::LocalStorage::NonZero(value)
        }
    };
    lir::Local::new(local.name.clone(), storage)
}
