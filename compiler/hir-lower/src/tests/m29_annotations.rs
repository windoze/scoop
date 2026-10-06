use super::*;

#[test]
fn annotations_bind_constants_and_original_source_targets() {
    let source = r#"
        public annotation class Label(val text: String, val flag: Boolean = true)
        annotation class Count(val value: Int = -3)
        const val TEXT: String = "stable"
        @Label(TEXT) @Count
        public struct Box<T>(@Label("item") val value: T)
        @Count(7)
        public enum Choice {
            @Label("empty") Empty,
            Named(@Label("named field") val value: Int),
            Tuple(@Label("position") String)
        }
        public class Record(@Label("property") val id: Int) {
            @Count val size: Int get() = id
        }
        public interface View { @Label("view") public val name: String }
        fun main() { val box = Box<Int>(1) }
    "#;
    let ast = scoop_parser::parse(source).unwrap();
    let output = lower(&[complete_core_file(), ast]).unwrap();
    let annotations = &output.export.annotations;
    assert_eq!(annotations.declarations.len(), 2);
    let nominal = annotations
        .targets
        .iter()
        .find(|entry| entry.annotations.len() == 2)
        .unwrap();
    assert!(matches!(
        nominal.target,
        hir::SourceAnnotationTarget::Nominal(_)
    ));
    assert_eq!(
        nominal.annotations[0].arguments[0],
        hir::CanonicalConstValueV1::String("stable".into())
    );
    assert_eq!(
        nominal.annotations[0].arguments[1].kind(),
        hir::CanonicalConstValueKindV1::Boolean
    );
    assert_eq!(
        annotations
            .targets
            .iter()
            .filter(|entry| matches!(entry.target, hir::SourceAnnotationTarget::VariantField(_)))
            .count(),
        2
    );
    assert_eq!(
        annotations
            .targets
            .iter()
            .filter(|entry| matches!(entry.target, hir::SourceAnnotationTarget::Property(_)))
            .count(),
        3
    );
}

#[test]
fn imported_annotations_keep_identity_and_normalized_default_arguments() {
    super::m23_generic_body_consumption::with_provider_consumer(
        r#"
            package shared
            public annotation class Label(val text: String, val fallback: String = "default")
            public struct Scope() {
                public annotation class Nested(val value: Int = 7)
            }
            @Label("provider")
            public struct Item(@Scope.Nested val value: Int)
        "#,
        r#"
            import shared.Label as Text
            import shared.Scope.Nested as Number
            @Text("consumer")
            public struct Other(@Number(9) val value: Int)
        "#,
        |output, _, foundation, interface, _| {
            let metadata = &output.output().export.annotations;
            assert!(metadata.declarations.is_empty());
            assert_eq!(metadata.targets.len(), 2);
            let application = &metadata
                .targets
                .iter()
                .find(|entry| matches!(entry.target, hir::SourceAnnotationTarget::Nominal(_)))
                .unwrap()
                .annotations[0];
            assert!(foundation.annotation(application.annotation).is_some());
            assert_eq!(
                application.arguments,
                vec![
                    hir::CanonicalConstValueV1::String("consumer".into()),
                    hir::CanonicalConstValueV1::String("default".into())
                ]
            );
            assert_eq!(interface.annotations().declarations().len(), 2);
            assert_eq!(interface.annotations().targets().len(), 2);
        },
    )
    .unwrap();
}
