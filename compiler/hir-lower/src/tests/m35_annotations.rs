use super::*;
use hir::{CanonicalAnnotationValueV1 as Value, CanonicalConstValueKindV1 as Kind};

#[test]
fn annotation_arrays_retain_scalar_types_order_defaults_and_float_bits() {
    let source = r#"
        typealias Texts = Array<String>
        annotation class Mark(val words: Texts, val counts: Array<Int8> = [],
                              val weights: Array<Float> = [-0f, 1.0])
        const val TEXT: String = "雪"
        @Mark([TEXT, "same", TEXT], counts = [-128, 127,])
        struct Item(val value: Int)
        fun main() {}
    "#;
    let output = lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap();
    let annotations = &output.export.annotations;
    assert_eq!(annotations.declarations.len(), 1);
    let values = |value: &Value| {
        let Value::Array {
            element_type: hir::AnnotationElementTypeV1::Scalar(kind),
            elements,
        } = value
        else {
            panic!("expected scalar array")
        };
        (
            *kind,
            elements
                .iter()
                .map(|element| {
                    let Value::Scalar(value) = &element.value else {
                        panic!("expected scalar element")
                    };
                    value.clone()
                })
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(
        values(
            annotations.declarations[0].parameters[1]
                .default
                .as_ref()
                .unwrap()
        ),
        (Kind::Integer(hir::IntegerKind::SIGNED_8), vec![])
    );
    let arguments = &annotations.targets[0].annotations[0].arguments;
    assert_eq!(
        values(&arguments[0]),
        (
            Kind::String,
            ["雪", "same", "雪"]
                .map(|v| hir::CanonicalConstValueV1::String(v.into()))
                .to_vec()
        )
    );
    assert_eq!(
        values(&arguments[1]),
        (
            Kind::Integer(hir::IntegerKind::SIGNED_8),
            vec![
                hir::CanonicalConstValueV1::Integer(hir::CanonicalIntegerConstantV1::Signed8(128)),
                hir::CanonicalConstValueV1::Integer(hir::CanonicalIntegerConstantV1::Signed8(127)),
            ]
        )
    );
    assert_eq!(
        values(&arguments[2]),
        (
            Kind::Float(hir::FloatKind::F32),
            vec![
                hir::CanonicalConstValueV1::Float(hir::HirFloatConstant::F32(0x8000_0000)),
                hir::CanonicalConstValueV1::Float(hir::HirFloatConstant::F32(0x3f80_0000)),
            ]
        )
    );
    let Value::Array { elements, .. } = &arguments[0] else {
        unreachable!()
    };
    assert_eq!(
        elements
            .iter()
            .map(|e| &source[e.span.start_byte() as usize..e.span.end_byte() as usize])
            .collect::<Vec<_>>(),
        ["TEXT", "\"same\"", "TEXT"]
    );
    let Value::Array { elements, .. } = &arguments[2] else {
        unreachable!()
    };
    assert!(
        elements
            .iter()
            .all(|e| e.span.end_byte() < source.find("@Mark").unwrap() as u64)
    );
}

#[test]
fn annotation_array_errors_identify_the_element_instead_of_the_application() {
    for (element, expected) in [
        ("128", "not representable"),
        ("false", "must be Int8"),
        ("[1]", "found an array"),
    ] {
        let source = format!(
            "annotation class Mark(val values: Array<Int8>)\n@Mark([1, {element}, 2]) struct Item()"
        );
        let errors =
            lower(&[complete_core_file(), scoop_parser::parse(&source).unwrap()]).unwrap_err();
        let error = errors
            .iter()
            .find(|error| error.message.contains(expected))
            .unwrap_or_else(|| panic!("missing {expected}: {errors:?}"));
        let start = source.find(&format!(", {element},")).unwrap() + 2;
        assert_eq!(
            error.span,
            Some(ast::Span {
                start: start as u32,
                end: (start + element.len()) as u32
            })
        );
    }
}
