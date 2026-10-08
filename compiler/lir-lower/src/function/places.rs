use super::*;

pub(super) fn source_local(
    context: &LoweringContext,
    module: &mir::Module,
    local: &mir::Local,
    address_taken: bool,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
) -> StorageResult<lir::Local> {
    let storage = match abi::classify_storage(context, lir_type(module, &local.ty), structs, enums)?
    {
        abi::ValueStorage::ZeroSized(representation) => {
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
        abi::ValueStorage::NonZero(value) => lir::LocalStorage::NonZero(value),
    };
    Ok(lir::Local::new(local.name.clone(), storage))
}
