use super::*;
use hir::{
    AnnotationTypeSlot as Slot, SourceAnnotationCallable as Callable,
    SourceAnnotationTarget as Target, TypeUseStep as Step,
};

const ANNOTATIONS: &str = r#"
    annotation class A(val label: String)
    annotation class B(val label: String)
    annotation class C(val label: String)
    annotation class D(val label: String)
"#;

fn with_types(source: &str, verify: impl FnOnce(&hir::Module)) {
    let source = format!("{ANNOTATIONS}\n{source}\nfun main() {{}}\n");
    let output = lower(&[complete_core_file(), scoop_parser::parse(&source).unwrap()]).unwrap();
    verify(output.export.module());
}

fn parameter<'a>(
    module: &'a hir::Module,
    name: &str,
    index: u32,
) -> &'a hir::SourceAnnotatedTypeUse {
    let (function, _) = module
        .functions
        .iter()
        .find(|(_, function)| function.name == name)
        .unwrap();
    module
        .annotations
        .annotated_type(
            Target::Parameter {
                callable: Callable::Function(function),
                index,
            },
            Slot::Value,
        )
        .unwrap()
}

fn labels(nodes: &[hir::TypeAnnotationNode]) -> Vec<(Vec<Step>, Vec<&str>)> {
    nodes
        .iter()
        .map(|node| {
            (
                node.path.clone(),
                node.annotations
                    .iter()
                    .map(|application| {
                        let hir::CanonicalAnnotationValueV1::Scalar(
                            hir::CanonicalConstValueV1::String(label),
                        ) = &application.arguments[0]
                        else {
                            panic!("string annotation argument")
                        };
                        label.as_str()
                    })
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn type_annotations_preserve_nullable_function_and_pointer_positions() {
    with_types(
        r#"
        fun nullable(a: @A("outside") Int?, b: (@B("inside") Int)?) {}
        fun function(f: @A("function") (@B("receiver") Int).(@C("parameter") String) -> @D("result") Int) {}
        fun pointers(p: @A("pointer") Ptr<@B("pointee") Int>, f: FunPtr<@A("signature") (@B("parameter") Int) -> @C("result") Int>) {}
    "#,
        |module| {
            assert_eq!(
                labels(&parameter(module, "nullable", 0).nodes),
                [(vec![], vec!["outside"])]
            );
            assert_eq!(
                labels(&parameter(module, "nullable", 1).nodes),
                [(vec![Step::Argument(0)], vec!["inside"])]
            );
            assert_eq!(
                labels(&parameter(module, "function", 0).nodes),
                [
                    (vec![], vec!["function"]),
                    (vec![Step::FunctionReceiver], vec!["receiver"]),
                    (vec![Step::FunctionParameter(0)], vec!["parameter"]),
                    (vec![Step::FunctionResult], vec!["result"]),
                ]
            );
            assert_eq!(
                labels(&parameter(module, "pointers", 0).nodes),
                [
                    (vec![], vec!["pointer"]),
                    (vec![Step::PointerPointee], vec!["pointee"])
                ]
            );
            assert_eq!(
                labels(&parameter(module, "pointers", 1).nodes),
                [
                    (vec![Step::FunctionPointee], vec!["signature"]),
                    (
                        vec![Step::FunctionPointee, Step::FunctionParameter(0)],
                        vec!["parameter"]
                    ),
                    (
                        vec![Step::FunctionPointee, Step::FunctionResult],
                        vec!["result"]
                    ),
                ]
            );
            for usage in &module.annotations.type_uses {
                let usage = usage.published(module).unwrap();
                assert!(
                    usage
                        .nodes
                        .iter()
                        .all(|node| hir::type_annotation_path_exists(&usage.ty, &node.path))
                );
            }
        },
    );
}

#[test]
fn type_annotations_preserve_nullable_declaration_and_anonymous_receivers() {
    with_types(
        r#"
        fun @A("root") String?.outer(): Int = 42
        fun (@A("inner") String)?.inner(): Int = 42
        val @A("root") String?.outer: Int get() = 42
        val (@A("inner") String)?.inner: Int get() = 42
        fun make() {
            val outer = fun @A("root") String?.(): Int { return 42 }
            val inner = fun (@A("inner") String)?.(): Int { return 42 }
        }
    "#,
        |module| {
            let sites = module
                .annotations
                .type_sites
                .iter()
                .filter(|site| matches!(site.target, Target::Receiver(_)))
                .collect::<Vec<_>>();
            assert_eq!(sites.len(), 6);
            for site in sites {
                let usage = module.annotations.type_use(&site.type_use).unwrap();
                let labels = labels(&usage.nodes);
                assert_eq!(labels.len(), 1);
                let expected = match labels[0].1.as_slice() {
                    ["root"] => vec![],
                    ["inner"] => vec![Step::Argument(0)],
                    other => panic!("unexpected receiver annotation: {other:?}"),
                };
                assert_eq!(labels[0].0, expected);
            }
        },
    );
}

#[test]
fn type_annotations_follow_original_alias_parameters_even_when_types_coincide() {
    with_types(
        r#"
        typealias Mix<T, U> = @A("template") (U, @B("formal") T, T)
        typealias Drop<T> = @A("fixed") Int
        fun mixed(x: @D("use") Mix<@C("first") Int, @D("second") Int>) {}
        fun plain(x: (Int, Int, Int)) {}
        fun dropped(x: Drop<@B("discarded") String>) {}
    "#,
        |module| {
            let usage = parameter(module, "mixed", 0);
            assert_eq!(
                labels(&usage.nodes),
                [
                    (vec![], vec!["template", "use"]),
                    (vec![Step::TupleElement(0)], vec!["second"]),
                    (vec![Step::TupleElement(1)], vec!["formal", "first"]),
                    (vec![Step::TupleElement(2)], vec!["first"]),
                ]
            );
            let plain = module
                .functions
                .values()
                .find(|function| function.name == "plain")
                .unwrap();
            assert_eq!(
                usage.ty,
                hir::AnnotationParameterType::Value(plain.params[0].ty)
            );
            assert_eq!(
                labels(&parameter(module, "dropped", 0).nodes),
                [(vec![], vec!["fixed"])]
            );
        },
    );
}

#[test]
fn type_annotations_survive_alias_chains_in_template_argument_use_order() {
    with_types(
        r#"
        typealias First<T> = @A("template") Array<@B("element") T>
        typealias Second<U> = First<U>
        fun chained(x: @C("use") Second<@D("actual") Int>) {}
    "#,
        |module| {
            assert_eq!(
                labels(&parameter(module, "chained", 0).nodes),
                [
                    (vec![], vec!["template", "use"]),
                    (vec![Step::Argument(0)], vec!["element", "actual"]),
                ]
            );
        },
    );
}

#[test]
fn type_annotations_keep_static_annotation_parameter_types_without_marker_binders() {
    with_types(
        r#"
        typealias Items<T> = @C("container") Array<T>
        annotation class Nested(val n: Int)
        annotation class Outer(val nested: @A("nested") Nested, val values: Items<@B("element") Nested>)
    "#,
        |module| {
            let declaration = module.annotations.declarations.iter().find(|declaration| matches!(declaration.identity.key().name(), scoop_identity::DeclarationName::Named(name) if name.as_str() == "Outer")).unwrap();
            let target = Target::Annotation(declaration.identity.id());
            let direct = module
                .annotations
                .annotated_type(target, Slot::AnnotationParameter(0))
                .unwrap();
            assert!(matches!(
                direct.ty,
                hir::AnnotationParameterType::Annotation(_)
            ));
            let array = module
                .annotations
                .annotated_type(target, Slot::AnnotationParameter(1))
                .unwrap();
            assert!(matches!(
                array.ty,
                hir::AnnotationParameterType::AnnotationArray { .. }
            ));
            assert!(array.parameters.is_empty());
            assert_eq!(
                labels(&array.nodes),
                [
                    (vec![], vec!["container"]),
                    (vec![Step::Argument(0)], vec!["element"])
                ]
            );
            assert_eq!(array.published(module).unwrap().binder_count, 0);
        },
    );
}

#[test]
fn type_annotations_reject_local_runtime_values_after_their_body_scope_has_closed() {
    for body in [
        "fun f(value: String) { val x: @A(value) Int = 1 }",
        "const val value: String = \"global\"\nfun f() { val value: String = \"local\"; val x: @A(value) Int = 1 }",
        "fun f(value: String) { val body: () -> Unit = { val x: @A(value) Int = 1 }; body() }",
    ] {
        let source = format!("{ANNOTATIONS}\n{body}");
        let errors =
            lower(&[complete_core_file(), scoop_parser::parse(&source).unwrap()]).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message == "annotation arguments must refer to a const val"),
            "{errors:?}"
        );
    }
}

#[test]
fn type_annotation_alias_merge_errors_point_to_the_actual_use() {
    let source = "annotation class A\ntypealias Named = @A Int\nfun f(x: @A Named) {}";
    let errors = lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap_err();
    let error = errors
        .iter()
        .find(|error| error.message.contains("must not be repeated"))
        .unwrap();
    let span = error.span.unwrap();
    assert_eq!(&source[span.start as usize..span.end as usize], "@A");
    assert_eq!(span.start as usize, source.rfind("@A").unwrap());
}
