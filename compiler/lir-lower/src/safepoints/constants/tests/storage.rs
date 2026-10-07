use super::*;

#[test]
fn known_enum_copies_keep_the_typed_variant_without_evaluating_payloads() {
    let mut enums = lir::EnumDefs::default();
    let id = enums.alloc(lir::EnumDef {
        exact_type: crate::tests::test_physical_exact(
            "Folded",
            scoop_identity::SourceNominalKind::Enum,
        ),
        name: "Folded".to_string(),
        repr: lir::EnumRepr::Niche {
            kind: lir::NichePointerKind::Managed,
            payload_variant: 1,
        },
        scan: lir::RefScan::References(vec![0]),
    });
    let out = lir::TempId::from_raw(la_arena::RawIdx::from(0));
    let local = lir::LocalId::from_raw(la_arena::RawIdx::from(0));
    let mut constants = HashMap::new();
    for instruction in [
        lir::Instruction::EnumWrap {
            out,
            variant: enums.variant_ref(id, 1).unwrap(),
            fields: vec![lir::Value::Param(0)],
        },
        lir::Instruction::Store {
            local,
            value: lir::Value::Temp(out),
        },
    ] {
        let (key, value) = known_result(&instruction, &constants).unwrap();
        constants.insert(key, value);
    }
    for (index, expected) in [(0, false), (1, true)] {
        assert_eq!(
            known_result(
                &lir::Instruction::VariantTest {
                    out,
                    operand: lir::Value::Local(local),
                    variant: enums.variant_ref(id, index).unwrap(),
                },
                &constants
            ),
            Some((
                LiveValue::Temp(out),
                KnownValue::Scalar(lir::Value::BoolConst(expected))
            ))
        );
    }
    assert_eq!(
        known_result(
            &lir::Instruction::EnumTag {
                out,
                enum_id: id,
                operand: lir::Value::Local(local),
            },
            &constants
        ),
        Some((
            LiveValue::Temp(out),
            KnownValue::Scalar(lir::Value::MachineScalar(lir::MachineScalarValue::EnumTag(
                1
            )))
        ))
    );
}

#[test]
fn precise_pointer_stores_update_the_place_and_unknown_stores_invalidate_only_escaped_places() {
    let pointee = lir::AbiValue::new(
        lir::LirType::I64,
        lir::AbiNonZeroLayout::new(8, 8).unwrap(),
        lir::RefScan::None,
    )
    .unwrap();
    let mut locals = Arena::new();
    let local = locals.alloc(lir::Local::new(
        "place",
        lir::LocalStorage::NonZero(pointee.clone()),
    ));
    let copied = locals.alloc(lir::Local::new(
        "copy",
        lir::LocalStorage::NonZero(pointee.clone()),
    ));
    let pointer = lir::TempId::from_raw(la_arena::RawIdx::from(0));
    let address_taken = HashSet::from([local]);
    let mut constants = BlockConstants::default();
    for instruction in [
        lir::Instruction::Store {
            local,
            value: lir::Value::IntegerConst(C::Signed64(10)),
        },
        lir::Instruction::LocalAddress {
            out: pointer,
            local,
        },
        lir::Instruction::RawStore {
            pointer: lir::Value::Temp(pointer),
            value: lir::Value::IntegerConst(C::Signed64(9)),
            pointee: pointee.clone(),
        },
        lir::Instruction::Store {
            local: copied,
            value: lir::Value::Local(local),
        },
    ] {
        constants.apply(&instruction, &locals, &address_taken);
    }
    let expected = Some(KnownValue::Scalar(lir::Value::IntegerConst(C::Signed64(9))));
    assert_eq!(
        known_value(lir::Value::Local(local), &constants.values),
        expected
    );
    constants.apply(
        &lir::Instruction::RawStore {
            pointer: lir::Value::Param(0),
            value: lir::Value::IntegerConst(C::Signed64(2)),
            pointee,
        },
        &locals,
        &address_taken,
    );
    assert_eq!(
        known_value(lir::Value::Local(local), &constants.values),
        None
    );
    assert_eq!(
        known_value(lir::Value::Local(copied), &constants.values),
        expected
    );
}
