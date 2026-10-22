use super::*;
use scoop_generator_protocol::{Annotations, annotation_declaration};
use serde_json::json;

#[test]
fn annotation_declaration_json_uses_bound_defaults_rules_and_nested_annotation_types() {
    let source = r#"
        annotation class Detail(val code: Int = 7)
        @Target([AnnotationTarget.Function])
        @Repeatable
        annotation class Mark(val names: Array<String> = ["默认"], val detail: Detail = Detail(),
            val details: Array<Detail> = [Detail(3)])
        @Mark @Mark(["显式"]) fun <T> inspect(value: T) {}
    "#;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let target = hir::SemanticAnnotationTarget::Current(hir::SourceAnnotationTarget::Callable(
            hir::SourceAnnotationCallable::Function(function),
        ));
        let applications = serde_json::to_value(Annotations { queries, target }).unwrap();
        let hir::SemanticAnnotations::Current(bound) = queries.describe_annotations(target) else {
            panic!("source applications")
        };
        let annotation = bound[0].annotation;
        let declaration = annotation_declaration(queries, annotation)
            .unwrap()
            .unwrap();
        assert_eq!(declaration["annotation"], annotation.to_string());
        assert_eq!(declaration["name"], "Mark");
        assert_eq!(declaration["targets"], json!(["Function"]));
        assert_eq!(declaration["repeatable"], true);
        assert_eq!(declaration["visibility"], "internal");
        assert_eq!(
            declaration["parameters"][0]["default"],
            applications[0]["arguments"][0]
        );
        assert_ne!(
            declaration["parameters"][0]["default"],
            applications[1]["arguments"][0]
        );
        assert_eq!(declaration["parameters"][0]["valueType"]["kind"], "value");
        assert_eq!(
            declaration["parameters"][1]["valueType"]["kind"],
            "annotation"
        );
        assert_eq!(
            declaration["parameters"][2]["valueType"]["kind"],
            "annotationArray"
        );
        assert_eq!(
            declaration["parameters"][1]["valueType"]["annotation"],
            declaration["parameters"][2]["valueType"]["element"]
        );
        for (index, parameter) in declaration["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
        {
            assert_eq!(
                parameter["declaration"],
                json!({"annotation": annotation.to_string(), "index": index})
            );
        }
        let origin = &declaration["parameters"][0]["default"]["elements"][0]["origin"];
        assert_eq!(origin["source"]["path"], "src/user.scoop");
        let start = origin["span"]["startByte"].as_u64().unwrap() as usize;
        let end = origin["span"]["endByte"].as_u64().unwrap() as usize;
        assert_eq!(&source[start..end], "\"默认\"");
        assert_eq!(
            annotation_declaration(queries, annotation)
                .unwrap()
                .unwrap(),
            declaration
        );
        Ok(Vec::new())
    });
    let mut core = crate::tests::core_file();
    let annotations = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../sysroot/src/scoop.core/src/serialization_annotations.scoop"
    ));
    core.declarations
        .extend(scoop_parser::parse(annotations).unwrap().declarations);
    compile_files_with_core(core, &[("src/user.scoop", source)], &mut driver).unwrap();
}
