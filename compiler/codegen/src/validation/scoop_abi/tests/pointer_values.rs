use super::*;

#[test]
fn abi_pointer_storage_retains_scan_and_provenance_at_the_complete_lir_boundary() {
    let module = module_with_types(StructDefs::default(), EnumDefs::default());
    let managed = scoop_lir::MANAGED_PTR;
    let valid = abi_value(managed.clone(), 8, 8, RefScan::References(vec![0]));
    AbiMetadataValidator::new(&module)
        .validate_value(&valid, "managed pointer")
        .unwrap();
    for value in [
        abi_value(managed.clone(), 8, 8, RefScan::None),
        abi_value(managed, 8, 16, RefScan::References(vec![0])),
        abi_value(scoop_lir::RAW_PTR, 8, 8, RefScan::References(vec![0])),
        abi_value(LirType::I64, 8, 8, RefScan::References(vec![0])),
    ] {
        assert!(
            AbiMetadataValidator::new(&module)
                .validate_value(&value, "invalid pointer storage")
                .is_err()
        );
    }
}

#[test]
fn abi_scalar_storage_cannot_claim_another_width() {
    let module = module_with_types(StructDefs::default(), EnumDefs::default());
    let invalid = abi_value(LirType::I64, 1, 1, RefScan::None);
    assert!(
        AbiMetadataValidator::new(&module)
            .validate_value(&invalid, "integer storage")
            .is_err()
    );
}
