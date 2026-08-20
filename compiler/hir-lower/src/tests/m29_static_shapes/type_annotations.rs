use super::*;
use hir::{AnnotationTypeSlot as Slot, TypeUseStep as Step};

fn labels(nodes: &[hir::TypeAnnotationNode]) -> Vec<(Vec<Step>, Vec<&str>)> {
    nodes
        .iter()
        .map(|node| {
            (
                node.path.clone(),
                node.annotations
                    .iter()
                    .map(|annotation| {
                        let hir::CanonicalAnnotationValueV1::Scalar(
                            hir::CanonicalConstValueV1::String(value),
                        ) = &annotation.arguments[0]
                        else {
                            panic!("string label")
                        };
                        value.as_str()
                    })
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn type_annotations_support_generic_static_queries_before_and_after_import() {
    with_shapes(
        r#"package model
        public annotation class A(val label: String)
        public annotation class B(val label: String)
        public annotation class C(val label: String)
        public struct Pair<T, U>(val values: @A("field") (T, U))
        public typealias Swapped<T, U> = @A("alias") Pair<U, T>
    "#,
        r#"import model.*
        fun shape(): Pair<Int, Int> = Pair((1, 2))
        fun args(first: @B("first") Int, second: @C("second") Int) {}
        fun alias(value: Swapped<@B("first") Int, @C("second") Int>) {}
        fun qualified(value: model.Swapped<@B("first") Int, @C("second") Int>) {}
        fun again(value: model.Swapped<@B("first") Int, @C("second") Int>) {}
    "#,
        |module, world, imported| {
            let shape = nominal(module, "shape");
            let field = shape.fields().next().unwrap();
            let nodes = field.type_annotations(world);
            assert_eq!(labels(nodes), [(vec![], vec!["field"])]);
            let (function, _) = module
                .functions
                .iter()
                .find(|(_, function)| function.name == "args")
                .unwrap();
            let arguments = (0..2)
                .map(|index| {
                    module
                        .annotations
                        .annotated_type(
                            hir::SourceAnnotationTarget::Parameter {
                                callable: hir::SourceAnnotationCallable::Function(function),
                                index,
                            },
                            Slot::Value,
                        )
                        .unwrap()
                        .nodes
                        .clone()
                })
                .collect::<Vec<_>>();
            let result = field
                .value_type()
                .annotated_signature(module, &[], nodes, &arguments)
                .unwrap();
            assert_eq!(
                labels(&result.nodes),
                [
                    (vec![], vec!["field"]),
                    (vec![Step::TupleElement(0)], vec!["first"]),
                    (vec![Step::TupleElement(1)], vec!["second"])
                ]
            );
            assert_eq!(
                result.ty,
                field.value_type().signature(module, &[]).unwrap()
            );
            for name in ["alias", "qualified", "again"] {
                let (function, _) = module
                    .functions
                    .iter()
                    .find(|(_, function)| function.name == name)
                    .unwrap();
                let usage = module
                    .annotations
                    .annotated_type(
                        hir::SourceAnnotationTarget::Parameter {
                            callable: hir::SourceAnnotationCallable::Function(function),
                            index: 0,
                        },
                        Slot::Value,
                    )
                    .unwrap();
                assert_eq!(
                    labels(&usage.nodes),
                    [
                        (vec![], vec!["alias"]),
                        (vec![Step::Argument(0)], vec!["second"]),
                        (vec![Step::Argument(1)], vec!["first"])
                    ],
                    "{name}"
                );
                assert_eq!(
                    usage.nodes[0].annotations[0].source.cone() != module.cone,
                    imported
                );
            }
        },
    );
}
