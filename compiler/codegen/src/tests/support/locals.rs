use super::*;

pub(in crate::tests) fn test_local(name: &str, ty: LirType) -> Local {
    test_local_with_types(
        &scoop_lir::StructDefs::default(),
        &scoop_lir::EnumDefs::default(),
        name,
        ty,
    )
}

pub(in crate::tests) fn test_local_with_types(
    structs: &scoop_lir::StructDefs,
    enums: &scoop_lir::EnumDefs,
    name: &str,
    ty: LirType,
) -> Local {
    let storage = match abi_argument(structs, enums, ty) {
        scoop_lir::AbiArgument::ElidedZst(value) => {
            assert_eq!(value.storage_type(), &LirType::Aggregate(Vec::new()));
            scoop_lir::LocalStorage::LogicalZst(unit_zst(value))
        }
        scoop_lir::AbiArgument::Direct(value) => {
            scoop_lir::LocalStorage::NonZero(value.value().clone())
        }
        scoop_lir::AbiArgument::Indirect(value) => scoop_lir::LocalStorage::NonZero(value),
    };
    Local::new(name, storage)
}

pub(in crate::tests) fn unit_zst(representation: scoop_lir::AbiZst) -> scoop_lir::LogicalZstValue {
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .expect("Unit has a canonical exact identity")
    .id();
    scoop_lir::LogicalZstValue::new(exact, representation)
}

pub(in crate::tests) fn test_zst_place(name: &str) -> Local {
    let representation = scoop_lir::AbiZst::new(
        LirType::Aggregate(Vec::new()),
        scoop_lir::AbiZeroSizedLayout::new(1).unwrap(),
    )
    .unwrap();
    Local::new(
        name,
        scoop_lir::LocalStorage::AddressableZst(scoop_lir::AddressableZstPlace::new(
            unit_zst(representation),
            scoop_lir::LocalPlaceLifetime::FunctionActivation,
        )),
    )
}
