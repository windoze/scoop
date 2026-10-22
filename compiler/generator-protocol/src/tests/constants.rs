use super::*;
use hir::{CanonicalConstValueV1 as Const, CanonicalIntegerConstantV1 as Int};

#[test]
fn integer_constants_preserve_each_width_and_values_beyond_json_precision() {
    for (value, signed, width, expected) in [
        (Int::Signed8(255), true, 8, "-1"),
        (Int::Signed16(1 << 15), true, 16, "-32768"),
        (Int::Signed32(1 << 31), true, 32, "-2147483648"),
        (Int::Signed64(1 << 63), true, 64, "-9223372036854775808"),
        (Int::Unsigned8(255), false, 8, "255"),
        (Int::Unsigned16(u16::MAX), false, 16, "65535"),
        (Int::Unsigned32(u32::MAX), false, 32, "4294967295"),
        (Int::Unsigned64(u64::MAX), false, 64, "18446744073709551615"),
    ] {
        assert_eq!(
            serde_json::to_value(QueryJson(&Const::Integer(value))).unwrap(),
            json!({
                "kind": "integer", "signed": signed, "width": width, "value": expected,
            })
        );
    }
}

#[test]
fn float_constants_preserve_negative_zero_infinities_and_nan_payloads() {
    for (value, width, bits) in [
        (FloatConstant::F32(0), 32, "00000000"),
        (FloatConstant::F32(0x8000_0000), 32, "80000000"),
        (FloatConstant::F32(0x7f80_0000), 32, "7f800000"),
        (FloatConstant::F32(0xffc1_2345), 32, "ffc12345"),
        (
            FloatConstant::F64(0x8000_0000_0000_0000),
            64,
            "8000000000000000",
        ),
        (
            FloatConstant::F64(0xfff0_0000_0000_0000),
            64,
            "fff0000000000000",
        ),
        (
            FloatConstant::F64(0x7ff0_0000_0000_0001),
            64,
            "7ff0000000000001",
        ),
    ] {
        assert_eq!(
            serde_json::to_value(QueryJson(&Const::Float(value))).unwrap(),
            json!({
                "kind": "float", "width": width, "bits": bits,
            })
        );
    }
    for (value, expected) in [
        (
            Const::String("引号\"\n😀".into()),
            json!({"kind": "string", "value": "引号\"\n😀"}),
        ),
        (
            Const::Char('😀'.into()),
            json!({"kind": "char", "value": "😀"}),
        ),
        (
            Const::Boolean(true.into()),
            json!({"kind": "boolean", "value": true}),
        ),
    ] {
        assert_eq!(serde_json::to_value(QueryJson(&value)).unwrap(), expected);
    }
}

#[test]
fn nested_annotation_and_enum_array_values_keep_identity_order_and_element_origins() {
    let annotation = PersistentAnnotationId::from_source_declaration(&nominal_key(
        SourceNominalKind::AnnotationClass,
        0,
    ))
    .unwrap();
    let enum_key = nominal_key(SourceNominalKind::Enum, 0);
    let owner = PersistentTypeId::from_source_declaration(&enum_key).unwrap();
    let variant = PersistentEnumVariantId::from_key(
        &EnumVariantIdentityKey::source(&enum_key, CanonicalIdentifier::new("Selected").unwrap())
            .unwrap(),
    )
    .unwrap();
    let nested = hir::CanonicalAnnotationValueV1::Annotation {
        annotation,
        arguments: vec![hir::CanonicalAnnotationValueV1::Array {
            element_type: hir::AnnotationElementTypeV1::IntegralEnum(SignatureTypeKey::Nominal(
                owner,
            )),
            elements: vec![hir::AnnotationElementV1 {
                value: hir::CanonicalAnnotationValueV1::IntegralEnum {
                    value_type: SignatureTypeKey::Nominal(owner),
                    variant,
                    value: Int::Signed32(3),
                },
                source: SourceIdentity::single_file(),
                span: SourceSpan::new(5, 13).unwrap(),
            }],
        }],
    };
    let value = serde_json::to_value(QueryJson(&nested)).unwrap();
    assert_eq!(value["annotation"], annotation.to_string());
    let array = &value["arguments"][0];
    assert_eq!(array["elementType"]["kind"], "integralEnum");
    assert_eq!(
        array["elements"][0]["value"]["variant"],
        variant.to_string()
    );
    assert_eq!(array["elements"][0]["value"]["value"]["value"], "3");
    assert_eq!(
        array["elements"][0]["origin"]["span"],
        json!({"startByte": 5, "endByte": 13})
    );
    assert_eq!(
        array["elements"][0]["origin"]["source"]["path"],
        "main.scoop"
    );
    assert_eq!(
        serde_json::to_value(QueryJson(&hir::AnnotationElementTypeV1::Annotation(
            annotation
        )))
        .unwrap(),
        json!({"kind": "annotation", "annotation": annotation.to_string()})
    );
}
