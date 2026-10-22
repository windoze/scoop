use super::*;

#[test]
fn exact_values_atomic_orders_and_pointer_offsets_remain_explicit() {
    let int = expression(
        hir::DefaultExpressionKindV1::IntegerLiteral(hir::CanonicalIntegerConstantV1::Signed64(
            u64::MAX,
        )),
        core_type("Long"),
    );
    let operation = hir::AtomicOperation::CompareExchange {
        expected: int.clone(),
        value: int.clone(),
        order: AtomicCompareExchangeOrder::AcqRelAcquire,
        result: AtomicCompareExchangeResult::ObservedValue,
    };
    let value = serde_json::to_value(BodyJson::new(&operation)).unwrap();
    assert_eq!(value["successOrder"], "acqRel");
    assert_eq!(value["failureOrder"], "acquire");
    assert_eq!(value["result"], "observedValue");
    assert_eq!(value["expected"]["operation"]["value"]["value"], "-1");
    assert_eq!(
        value["expected"]["resultType"],
        json!(QueryJson(&core_type("Long")))
    );
    assert_eq!(
        value["expected"]["origin"],
        value["expected"]["evaluationOrigin"]
    );

    let pointer = expression(
        hir::DefaultExpressionKindV1::Local(LocalValueSelector::Parameter {
            declaration_index: 0,
        }),
        SignatureTypeKey::RawPointer(Box::new(core_type("Long"))),
    );
    let load = hir::DefaultExpressionKindV1::PtrLoad {
        pointer: Box::new(pointer.clone()),
        offset: hir::OptionalDefaultExpressionV1::absent(),
    };
    let store = hir::DefaultExpressionKindV1::PtrStore {
        pointer: Box::new(pointer),
        offset: hir::OptionalDefaultExpressionV1::present(int.clone()),
        value: Box::new(int),
    };
    assert_eq!(
        serde_json::to_value(BodyJson::new(&load)).unwrap()["offset"],
        Value::Null
    );
    let stored = serde_json::to_value(BodyJson::new(&store)).unwrap();
    assert_eq!(
        stored["pointer"]["operation"]["selector"],
        json!({"kind": "parameter", "index": 0})
    );
    assert_eq!(stored["offset"]["operation"]["value"]["value"], "-1");
    let float =
        hir::DefaultExpressionKindV1::FloatLiteral(FloatConstant::F64(0x7ff8_0000_0000_0009));
    assert_eq!(
        serde_json::to_value(BodyJson::new(&float)).unwrap()["value"]["bits"],
        "7ff8000000000009"
    );
}

#[test]
fn unary_binary_conversions_and_uninitialized_operations_keep_their_arity() {
    let value = expression(
        hir::DefaultExpressionKindV1::IntegerLiteral(hir::CanonicalIntegerConstantV1::Unsigned64(
            u64::MAX,
        )),
        core_type("ULong"),
    );
    let operation = hir::IntegerExpression::Binary {
        kind: hir::IntegerKind::UNSIGNED_64,
        operation: hir::IntegerBinaryOperator::Ushr,
        lhs: Box::new(value.clone()),
        rhs: Box::new(value.clone()),
    };
    let json = serde_json::to_value(BodyJson::new(&operation)).unwrap();
    assert_eq!(json["kind"], "binary");
    assert_eq!(json["operator"], "ushr");
    assert_eq!(
        json["integerKind"],
        json!({"kind": "integer", "signed": false, "width": 64})
    );
    assert_eq!(
        json["lhs"]["operation"]["value"]["value"],
        "18446744073709551615"
    );
    let conversion = hir::DefaultFloatConversionV1::ToInteger {
        source: FloatKind::F64,
        target: hir::DefaultIntegerKindV1::Unsigned64,
    };
    assert_eq!(
        serde_json::to_value(QueryJson(&conversion)).unwrap(),
        json!({"kind": "toInteger", "source": {"kind": "float", "width": 64}, "target": {"kind": "integer", "signed": false, "width": 64}})
    );
    let uninit = hir::MaybeUninitOperation::<hir::DefaultExpressionV1>::Uninit;
    assert_eq!(
        serde_json::to_value(BodyJson::new(&uninit)).unwrap(),
        json!({"kind": "uninit"})
    );
    let initialized = hir::MaybeUninitOperation::Initialized(value);
    assert_eq!(
        serde_json::to_value(BodyJson::new(&initialized)).unwrap()["kind"],
        "initialized"
    );
}
