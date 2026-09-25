use super::*;
use crate::CallingConvention;

fn nonzero(size: u64, align: u64, ty: LirType, scan: RefScan) -> AbiValue {
    AbiValue::new(ty, AbiNonZeroLayout::new(size, align).unwrap(), scan).unwrap()
}

#[test]
fn physical_abi_preserves_managed_pointer_provenance_and_complete_scan() {
    let reference: ExactLayoutExportV1 = managed().into();
    let expected = fixtures::function(&reference, &[&reference]);
    let signature = |argument: AbiValue| {
        ScoopAbiSignature::new(
            vec![AbiArgument::Direct(argument)],
            AbiReturn::Direct(nonzero(8, 8, MANAGED_PTR, RefScan::References(vec![0]))),
            CallingConvention::Cdecl,
        )
    };
    let valid = signature(nonzero(8, 8, MANAGED_PTR, RefScan::References(vec![0])));
    expected
        .validate_physical_signature(&EnumDefs::default(), &valid, crate::GcEffect::NoGc)
        .unwrap();
    for invalid in [
        nonzero(8, 8, MANAGED_PTR, RefScan::None),
        nonzero(8, 8, RAW_PTR, RefScan::References(vec![0])),
        nonzero(8, 8, LirType::I64, RefScan::References(vec![0])),
        nonzero(8, 16, MANAGED_PTR, RefScan::References(vec![0])),
    ] {
        assert!(matches!(
            expected.validate_physical_signature(
                &EnumDefs::default(),
                &signature(invalid),
                crate::GcEffect::NoGc
            ),
            Err(ExactCallablePhysicalAbiError::Argument(0))
        ));
    }
    assert!(matches!(
        expected.validate_physical_signature(
            &EnumDefs::default(),
            &valid,
            crate::GcEffect::Managed
        ),
        Err(ExactCallablePhysicalAbiError::Protocol)
    ));
}

#[test]
fn physical_abi_distinguishes_unit_void_user_zst_and_indirect_aggregate() {
    let unit: ExactLayoutExportV1 = unit().into();
    let zst = fixtures::aggregate(true);
    let structure = fixtures::aggregate(false);
    let enums = EnumDefs::default();
    let empty = ScoopAbiSignature::new(vec![], AbiReturn::UnitVoid, CallingConvention::Cdecl);
    fixtures::function(&unit, &[])
        .validate_physical_signature(&enums, &empty, crate::GcEffect::NoGc)
        .unwrap();
    assert!(matches!(
        fixtures::function(&zst, &[]).validate_physical_signature(
            &enums,
            &empty,
            crate::GcEffect::NoGc
        ),
        Err(ExactCallablePhysicalAbiError::Result)
    ));
    let structure_type = LirType::Struct(StructDefId::from_raw(0_u32.into()));
    let zst_signature = ScoopAbiSignature::new(
        vec![],
        AbiReturn::ElidedZst(
            AbiZst::new(structure_type.clone(), AbiZeroSizedLayout::new(1).unwrap()).unwrap(),
        ),
        CallingConvention::Cdecl,
    );
    fixtures::function(&zst, &[])
        .validate_physical_signature(&enums, &zst_signature, crate::GcEffect::NoGc)
        .unwrap();
    let value = nonzero(1, 1, structure_type, RefScan::None);
    let indirect = ScoopAbiSignature::new(
        vec![AbiArgument::Indirect(value.clone())],
        AbiReturn::Indirect(value.clone()),
        CallingConvention::Cdecl,
    );
    let expected = fixtures::function(&structure, &[&structure]);
    expected
        .validate_physical_signature(&enums, &indirect, crate::GcEffect::NoGc)
        .unwrap();
    let direct = ScoopAbiSignature::new(
        vec![AbiArgument::Direct(value.clone())],
        AbiReturn::Indirect(value),
        CallingConvention::Cdecl,
    );
    assert!(matches!(
        expected.validate_physical_signature(&enums, &direct, crate::GcEffect::NoGc),
        Err(ExactCallablePhysicalAbiError::Argument(0))
    ));
}

#[test]
fn physical_abi_rejects_wrong_integer_storage_type_and_bounds_validation_work() {
    let byte: ExactLayoutExportV1 = integer("Byte", IntegerKind::SIGNED_8).into();
    let expected = fixtures::function(&byte, &[]);
    let invalid = ScoopAbiSignature::new(
        vec![],
        AbiReturn::Direct(nonzero(1, 1, LirType::I64, RefScan::None)),
        CallingConvention::Cdecl,
    );
    assert!(matches!(
        expected.validate_physical_signature(&EnumDefs::default(), &invalid, crate::GcEffect::NoGc),
        Err(ExactCallablePhysicalAbiError::Result)
    ));
}
