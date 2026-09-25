use super::*;
use crate::{DecodedStrongStaticStorageSemanticProjectionV1, StrongSemanticProjectionError};
use scoop_wire::{decode_canonical, encode};

fn plan(encoded: bool) -> StrongStaticStorageSemanticPlanV1 {
    let mut structs = crate::StructDefs::default();
    let empty = structs.alloc_scoop("Empty".to_owned(), 0, 1, false, Vec::new());
    let state = if encoded {
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::Struct {
                struct_id: empty,
                fields: Vec::new(),
            },
        }
    } else {
        LirStaticInitialState::ZeroedForRuntimeUnit
    };
    let mut globals = Arena::new();
    globals.alloc(storage_global(
        "token",
        LirType::Struct(empty),
        RefScan::None,
        state,
    ));
    StrongStaticStorageSemanticPlanSetV1::from_parts(
        ConeIdentity::SINGLE_FILE,
        LirTargetProfile::DARWIN_AARCH64,
        &globals,
        &structs,
        &crate::EnumDefs::default(),
    )
    .unwrap()
    .storages()[0]
        .clone()
}

fn decoded(
    plan: &StrongStaticStorageSemanticPlanV1,
) -> DecodedStrongStaticStorageSemanticProjectionV1 {
    decode_canonical(&encode(&plan.semantic_projection()).unwrap()).unwrap()
}

#[test]
fn token_projection_retains_logical_extent_scan_and_initial_state_distinction() {
    let zeroed = plan(false);
    let encoded = plan(true);
    assert_eq!((zeroed.byte_size(), zeroed.allocation_extent()), (0, 1));
    assert_eq!(encoded.initial_state().initial_template(), &[0]);
    for plan in [&zeroed, &encoded] {
        let bytes = encode(&plan.semantic_projection()).unwrap();
        assert_eq!(bytes[0], 0xaa);
        assert_eq!(encode(&decoded(plan)).unwrap(), bytes);
        decoded(plan).validate_against(plan).unwrap();
    }
    assert!(matches!(
        decoded(&zeroed).validate_against(&encoded),
        Err(StrongSemanticProjectionError::Mismatch)
    ));
    assert!(matches!(
        decoded(&encoded).validate_against(&zeroed),
        Err(StrongSemanticProjectionError::Mismatch)
    ));
}

#[test]
fn storage_projection_replays_immortal_initializers_and_rejects_wrong_extent() {
    let mut globals = Arena::new();
    let immortal = globals.alloc(string_global(0, "value"));
    globals.alloc(storage_global(
        "value",
        LirType::Ptr(PointerKind::Managed),
        RefScan::References(vec![0]),
        LirStaticInitialState::EncodedStaticValue {
            payload: LirConstantImage::GlobalPointer {
                global: immortal,
                kind: PointerKind::Managed,
            },
        },
    ));
    let plans = StrongStaticStorageSemanticPlanSetV1::from_parts(
        ConeIdentity::SINGLE_FILE,
        LirTargetProfile::DARWIN_AARCH64,
        &globals,
        &crate::StructDefs::default(),
        &crate::EnumDefs::default(),
    )
    .unwrap();
    let expected = &plans.storages()[0];
    decoded(expected).validate_against(expected).unwrap();
    let changed = StrongStaticStorageSemanticPlanV1::from_artifact(
        expected.storage(),
        expected.symbol(),
        expected.layout(),
        expected.scan(),
        expected.scan_program().clone(),
        expected.scan_kind(),
        expected.byte_size(),
        16,
        expected.required_alignment(),
        expected.initial_state().clone(),
    );
    assert!(matches!(
        decoded(&changed).validate_against(expected),
        Err(StrongSemanticProjectionError::Mismatch)
    ));
}
