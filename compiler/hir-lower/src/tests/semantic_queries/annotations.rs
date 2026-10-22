use super::*;
use scoop_generator_protocol::{Annotations, QueryJson, annotation_declaration};
use serde_json::json;

#[test]
fn dependency_annotation_json_keeps_provider_defaults_separate_from_consumer_arguments() {
    struct Inspect(bool);
    impl hir::MethodBodyGenerator for Inspect {
        fn generate(
            &mut self,
            queries: &mut dyn SemanticQueries,
        ) -> Result<Vec<hir::GeneratedMethodBody>, Vec<scoop_ast::Diagnostic>> {
            let target = queries
                .annotations()
                .targets
                .iter()
                .find(|entry| {
                    matches!(
                        entry.target,
                        hir::SourceAnnotationTarget::Callable(
                            hir::SourceAnnotationCallable::Function(_)
                        )
                    )
                })
                .unwrap();
            let annotation = target.annotations[0].annotation;
            let target = hir::SemanticAnnotationTarget::Current(target.target);
            let hir::SemanticAnnotationDeclaration::Imported(imported) =
                queries.annotation_declaration(annotation).unwrap()
            else {
                panic!("imported annotation")
            };
            let default =
                serde_json::to_value(QueryJson(&imported.declaration.parameters[0].default))
                    .unwrap();
            let declaration = annotation_declaration(queries, annotation)
                .unwrap()
                .unwrap();
            let applications = serde_json::to_value(Annotations { queries, target }).unwrap();
            assert_eq!(declaration["name"], "Labels");
            assert_eq!(declaration["visibility"], "public");
            assert_eq!(declaration["targets"], json!(["Function"]));
            assert_eq!(declaration["parameters"][0]["default"], default);
            assert_eq!(default["elements"][0]["value"]["value"]["value"], "提供方");
            assert_eq!(
                applications[0]["arguments"][0]["elements"][0]["value"]["value"]["value"],
                "调用处"
            );
            assert_eq!(
                declaration["parameters"][1]["valueType"]["kind"],
                "annotation"
            );
            let provider = ConeCoordinate::new("test", "query-provider", "1.0.0")
                .unwrap()
                .identity()
                .unwrap();
            assert_eq!(
                declaration["origin"]["source"]["cone"],
                provider.to_string()
            );
            assert_eq!(
                default["elements"][0]["origin"]["source"]["cone"],
                provider.to_string()
            );
            assert_eq!(
                applications[0]["arguments"][0]["elements"][0]["origin"]["source"]["cone"],
                queries.current_cone().to_string()
            );
            self.0 = true;
            Ok(Vec::new())
        }
    }
    let mut inspect = Inspect(false);
    let annotations = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../sysroot/src/scoop.core/src/serialization_annotations.scoop"
    ));
    let mut source = crate::tests::complete_core_file();
    source
        .declarations
        .extend(scoop_parser::parse(annotations).unwrap().declarations);
    let core = crate::tests::m23_ordinary_core_only::support::trusted_core_from_source(
        source,
        annotations,
    );
    support::with_core_provider(
        core,
        r#"public annotation class Detail(val code: Int = 7)
            @Target([AnnotationTarget.Function])
            public annotation class Labels(val names: Array<String> = ["提供方"], val detail: Detail = Detail())"#,
        r#"@Labels(["调用处"]) fun inspect() {}"#,
        &mut inspect,
    );
    assert!(inspect.0);
}
