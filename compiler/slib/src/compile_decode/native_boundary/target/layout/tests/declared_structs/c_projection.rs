use super::*;
use scoop_hir::{IntegerKind, NativeBoundaryCAbiV1};

fn project(fixture: &mut Fixture) {
    let NativeBoundaryNominalShape::Struct { fields, .. } = fixture.records[0].shape() else {
        unreachable!()
    };
    fixture.records[0] = fixture.records[0]
        .clone()
        .with_c_abi(NativeBoundaryCAbiV1::UInt64Field {
            field: fields[0].field(),
        })
        .unwrap();
}

#[test]
fn explicit_uint64_field_projection_keeps_scoop_aggregate_and_c_scalar_distinct() {
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        for name in ["PinnedPtr", "GcHandle", "ResourceToken"] {
            let mut fixture = Fixture::new(provider, name, IntegerKind::UNSIGNED_64);
            project(&mut fixture);
            fixture.with_normalizer(|normalizer| {
                assert!(matches!(normalizer.c_storage(fixture.exact).unwrap(),
                    CanonicalCStorageType::Integer { exact_type, signedness: scoop_identity::Signedness::Unsigned, bit_width: scoop_identity::IntegerBitWidth::Bits64 }
                        if exact_type == fixture.exact));
                assert!(matches!(normalizer.scoop_argument(fixture.exact).unwrap(), ScoopAbiArgument::Indirect(storage)
                    if storage.shape() == ScoopAbiValueShape::Aggregate && storage.byte_size() == 8));
                assert!(matches!(normalizer.scoop_return(fixture.exact).unwrap(), ScoopAbiReturn::Indirect(storage)
                    if storage.shape() == ScoopAbiValueShape::Aggregate && storage.byte_size() == 8));
            });
        }
    }
}

#[test]
fn name_provider_and_same_size_do_not_grant_an_implicit_c_projection() {
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        for name in ["PinnedPtr", "GcHandle", "ResourceToken"] {
            let fixture = Fixture::new(provider, name, IntegerKind::UNSIGNED_64);
            fixture.with_normalizer(|normalizer| {
                assert!(matches!(normalizer.c_storage(fixture.exact), Err(NativeBoundaryCompileError::Target(
                    NativeBoundaryTargetError::NotCAbiSafe { exact }
                )) if exact == fixture.exact));
            });
        }
    }
}

#[test]
fn uint64_projection_replays_the_actual_field_type_and_requires_its_witness() {
    for kind in [IntegerKind::UNSIGNED_8, IntegerKind::SIGNED_64] {
        let mut fixture = Fixture::new(ConeIdentity::SINGLE_FILE, "ResourceToken", kind);
        project(&mut fixture);
        fixture.with_normalizer(|normalizer| {
            assert!(matches!(normalizer.c_storage(fixture.exact), Err(NativeBoundaryCompileError::Target(
                NativeBoundaryTargetError::NotCAbiSafe { exact }
            )) if exact == fixture.exact));
        });
    }
    let mut fixture = Fixture::new(
        ConeIdentity::SINGLE_FILE,
        "ResourceToken",
        IntegerKind::UNSIGNED_64,
    );
    project(&mut fixture);
    let owner = fixture.records.remove(1).owner();
    fixture.with_normalizer(|normalizer| {
        assert!(matches!(normalizer.c_storage(fixture.exact), Err(NativeBoundaryCompileError::ClosureRequired { owner: actual }) if actual == owner));
    });
}
