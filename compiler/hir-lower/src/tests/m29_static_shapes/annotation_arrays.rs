use super::*;

#[test]
fn annotation_arrays_have_the_same_static_description_in_source_and_dependencies() {
    with_shapes(
        r#"
            package model
            public annotation class Labels(val words: Array<String> = [], val flags: Array<Boolean> = [true, false, true])
            @Labels
            public struct Item(val value: Int)
        "#,
        r#"import model.Item
            fun shape(): Item = Item(1)
        "#,
        |module, world, _| {
            let shape = nominal(module, "shape");
            let annotations = shape.annotations(world);
            let annotation = annotations.iter().next().unwrap();
            assert_eq!(annotation.name(module, world), "Labels");
            let [
                hir::CanonicalAnnotationValueV1::Array {
                    element_type: words,
                    elements: empty,
                },
                hir::CanonicalAnnotationValueV1::Array {
                    element_type: flags,
                    elements,
                },
            ] = annotation.arguments
            else {
                panic!("array defaults are retained")
            };
            assert_eq!(
                words,
                &hir::AnnotationElementTypeV1::Scalar(hir::CanonicalConstValueKindV1::String)
            );
            assert!(empty.is_empty());
            assert_eq!(
                flags,
                &hir::AnnotationElementTypeV1::Scalar(hir::CanonicalConstValueKindV1::Boolean)
            );
            assert_eq!(
                elements.iter().map(|e| e.value.clone()).collect::<Vec<_>>(),
                [true, false, true].map(|v| hir::CanonicalAnnotationValueV1::Scalar(
                    hir::CanonicalConstValueV1::Boolean(v.into())
                ))
            );
            assert!(
                elements
                    .windows(2)
                    .all(|pair| pair[0].source == pair[1].source
                        && pair[0].span.end_byte() < pair[1].span.start_byte())
            );
        },
    );
}

#[test]
fn nested_annotations_and_integral_values_survive_static_description_imports() {
    with_shapes(
        r#"package model
            public enum Level: Int16 { Low = -3, High = 9 }
            public annotation class Message(val text: String, val levels: Array<Level> = [Level.Low, Level.High])
            public annotation class Mark(val message: Message = Message("bound"))
            @Mark
            public struct Item(val value: Int)
        "#,
        "import model.Item\nfun shape(): Item = Item(1)",
        |module, world, _| {
            let shape = nominal(module, "shape");
            let annotations = shape.annotations(world);
            let annotation = annotations.iter().next().unwrap();
            let [hir::CanonicalAnnotationValueV1::Annotation { arguments, .. }] =
                annotation.arguments
            else {
                panic!("nested annotation is retained")
            };
            assert_eq!(
                arguments[0],
                hir::CanonicalAnnotationValueV1::Scalar(hir::CanonicalConstValueV1::String(
                    "bound".into()
                ))
            );
            let hir::CanonicalAnnotationValueV1::Array {
                element_type: hir::AnnotationElementTypeV1::IntegralEnum(ty),
                elements,
            } = &arguments[1]
            else {
                panic!("integral enum array is retained")
            };
            let raw = elements
                .iter()
                .map(|element| {
                    let hir::CanonicalAnnotationValueV1::IntegralEnum {
                        value_type, value, ..
                    } = &element.value
                    else {
                        panic!("typed enum variant is retained")
                    };
                    assert_eq!(value_type, ty);
                    assert_eq!(value.kind(), hir::IntegerKind::SIGNED_16);
                    value.raw_bits()
                })
                .collect::<Vec<_>>();
            assert_eq!(raw, [65533, 9]);
            assert!(elements[0].span.end_byte() < elements[1].span.start_byte());
        },
    );
}
