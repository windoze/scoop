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
            assert_eq!(
                annotation.arguments,
                &[
                    hir::CanonicalAnnotationValueV1::Array {
                        element_type: hir::CanonicalConstValueKindV1::String,
                        elements: vec![],
                    },
                    hir::CanonicalAnnotationValueV1::Array {
                        element_type: hir::CanonicalConstValueKindV1::Boolean,
                        elements: [true, false, true]
                            .into_iter()
                            .map(|value| hir::CanonicalConstValueV1::Boolean(value.into()))
                            .collect(),
                    },
                ]
            );
        },
    );
}
