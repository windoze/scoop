use super::*;
use scoop_generator_protocol::{Query, QueryJson, execute_query};
use serde_json::{Value, json};

fn request(
    queries: &mut dyn SemanticQueries,
    function: hir::FunctionId,
    query: Value,
) -> Result<Value, Vec<scoop_ast::Diagnostic>> {
    execute_query(
        queries,
        function,
        serde_json::from_value::<Query>(query).unwrap(),
    )
}

fn type_reference(
    queries: &mut dyn SemanticQueries,
    function: hir::FunctionId,
    name: &str,
) -> Value {
    let context = hir::SemanticContext::Function(function);
    let ty = queries
        .resolve_type(context, &scoop_parser::parse_type(name).unwrap())
        .unwrap();
    json!(QueryJson(
        &queries.type_signature(context.into(), ty).unwrap()
    ))
}

#[test]
fn compiler_queries_consume_dependency_references_defaults_and_body_availability() {
    struct Inspect(bool);
    impl hir::MethodBodyGenerator for Inspect {
        fn generate(
            &mut self,
            queries: &mut dyn SemanticQueries,
        ) -> Result<Vec<hir::GeneratedMethodBody>, Vec<scoop_ast::Diagnostic>> {
            let function = queries
                .annotations()
                .targets
                .iter()
                .find_map(|entry| match entry.target {
                    hir::SourceAnnotationTarget::Callable(
                        hir::SourceAnnotationCallable::Function(id),
                    ) => Some(id),
                    _ => None,
                })
                .unwrap();
            let provider = ConeCoordinate::new("test", "query-provider", "1.0.0")
                .unwrap()
                .identity()
                .unwrap();
            assert_eq!(queries.resolve_provider(provider.into()), Some(provider));
            assert_eq!(
                queries.resolve_provider(scoop_identity::ConeIdentity::CORE.into()),
                Some(scoop_identity::ConeIdentity::CORE)
            );
            let mut lookup = |name: &str, namespace| {
                request(queries, function, json!({
                "kind": "lookupExport", "provider": provider.to_string(), "path": [name], "namespace": namespace,
            })).unwrap()["exports"][0].clone()
            };
            let choose = lookup("choose", "value");
            let opaque = lookup("opaque", "value");
            let small = lookup("small", "value");
            let mark = lookup("Mark", "type");
            let box_type = type_reference(queries, function, "Box<Int>");
            let reader_type = type_reference(queries, function, "Reader<Int>");
            let item = type_reference(queries, function, "Item");
            let int = type_reference(queries, function, "Int");
            let members = request(
                queries,
                function,
                json!({"kind": "describeMembers", "types": [box_type, reader_type]}),
            )?;
            let box_members = &members["members"][0];
            let constructor = &box_members["constructors"][0];
            let default = &constructor["parameters"][1]["calling"]["default"];
            let defaults = request(
                queries,
                function,
                json!({"kind": "describeDefaults", "defaults": [
                    default, {"owner": choose, "parameter": 1, "arguments": [item]},
                    {"owner": choose, "parameter": 0}, {"owner": opaque, "parameter": 0},
                ]}),
            )?;
            assert_eq!(defaults["defaults"][0]["applied"]["result"], int);
            assert_eq!(defaults["defaults"][1]["applied"]["result"], item);
            assert_eq!(
                defaults["defaults"][0]["definition"]["origin"]["source"]["cone"],
                provider.to_string()
            );
            assert!(defaults["defaults"][2].is_null());
            assert!(defaults["defaults"][3].is_null());
            let invalid = request(
                queries,
                function,
                json!({"kind": "describeDefaults", "defaults": [
                    {"owner": choose, "parameter": 1, "arguments": [int]},
                ]}),
            )
            .unwrap_err();
            assert!(
                invalid
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("must satisfy `ref`")),
                "{invalid:?}"
            );
            let abstract_method = &members["members"][1]["methods"][0]["declaration"];
            let getter = &box_members["properties"][2]["getter"]["body"]["declaration"];
            let bodies = request(
                queries,
                function,
                json!({"kind": "describeBodies", "callables": [
                    choose, opaque, constructor["declaration"], abstract_method, getter, small,
                ]}),
            )?;
            assert_eq!(bodies["bodies"][0]["availability"], "available");
            assert_eq!(bodies["bodies"][0]["body"]["owner"], choose);
            assert_eq!(bodies["bodies"][1], json!({"availability": "notPublished"}));
            assert_eq!(bodies["bodies"][2]["kind"], "constructor");
            assert_eq!(bodies["bodies"][3], json!({"availability": "noBody"}));
            assert_eq!(bodies["bodies"][4]["availability"], "available");
            assert_eq!(bodies["bodies"][5]["availability"], "available");
            assert_eq!(bodies["bodies"][5]["body"]["owner"], small);
            assert_eq!(bodies["bodies"][5]["body"]["typeParameters"], json!([]));
            assert_eq!(bodies["bodies"][5]["body"]["result"], int);
            let parameter = json!({"kind": "parameter", "callable": choose, "index": 1});
            let annotations = request(
                queries,
                function,
                json!({"kind": "describeAnnotations", "targets": [
                parameter, {"kind": "parameter", "callable": choose, "index": 9},
            ], "declarations": [mark["id"]]}),
            )?;
            assert_eq!(annotations["targets"][0], json!([]));
            assert!(annotations["targets"][1].is_null());
            assert_eq!(annotations["declarations"][0]["name"], "Mark");
            let locations = request(
                queries,
                function,
                json!({"kind": "sourceLocation", "targets": [parameter]}),
            )?;
            assert_eq!(
                locations["locations"][0]["source"]["cone"],
                provider.to_string()
            );
            assert_eq!(
                request(
                    queries,
                    function,
                    json!({"kind": "typeRelation", "relation": "equal", "left": item, "right": item})
                )?,
                json!({"holds": true})
            );
            self.0 = true;
            Ok(Vec::new())
        }
    }
    let mut inspect = Inspect(false);
    support::with_provider(
        r#"
            public annotation class Mark
            public class Box<T> public constructor(public val value: T, public val copy: T = value) {
                public val computed: T get() = copy
            }
            public class Item
            public fun <T: ref> choose(first: T, second: T = first): T = second
            public fun small(): Int = 1
            public fun opaque(): String = "opaque"
            public interface Reader<T> { public fun read(): T }
        "#,
        "@Mark fun inspect() {}",
        &mut inspect,
    );
    assert!(inspect.0);
}
