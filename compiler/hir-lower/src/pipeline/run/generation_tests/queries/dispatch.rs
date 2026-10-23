use super::*;
use scoop_generator_protocol::{CompilerQuery, MethodId, QueryJson, execute_query};
use serde_json::{Value, json};

mod defaults;

fn request(
    queries: &mut dyn SemanticQueries,
    function: hir::FunctionId,
    query: Value,
) -> Result<Value, Vec<Diagnostic>> {
    let method = MethodId::from(&queries.method_identity(function).unwrap());
    let request: CompilerQuery =
        serde_json::from_value(json!({"context": method, "query": query})).unwrap();
    assert_eq!(request.context, method);
    execute_query(queries, function, request.query)
}

fn type_reference(
    queries: &mut dyn SemanticQueries,
    function: hir::FunctionId,
    text: &str,
) -> Value {
    let context = SemanticContext::Function(function);
    let ty = super::types::resolve(queries, context, text);
    json!(QueryJson(
        &queries.type_signature(context.into(), ty).unwrap()
    ))
}

fn lookup(
    queries: &mut dyn SemanticQueries,
    function: hir::FunctionId,
    name: &str,
    namespace: &str,
) -> Value {
    let provider = queries.current_cone().to_string();
    request(
        queries,
        function,
        json!({"kind": "lookupExport", "provider": provider,
        "path": name.split('.').collect::<Vec<_>>(), "namespace": namespace}),
    )
    .unwrap()["exports"]
        .clone()
}

#[test]
fn compiler_queries_round_trip_all_families_in_a_pending_method_context() {
    let source = r#"
        public annotation class Label(val names: Array<String>)
        @Label(["first", "第二"]) public class Box<T>(val value: T) {
            fun choose(first: T, second: T = first): T = second
            val computed: T get() = value
        }
        public enum Choice { Some(val number: Int = 4), None }
        interface Reader { fun read(): Int }
        class Pending : Reader
        public fun empty() {}
        public fun overloaded(value: Int): Int = value
        public fun overloaded(value: String): String = value
        private fun hidden() {}
    "#;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = queries.pending_methods()[0].function;
        let method = json!(MethodId::from(&queries.method_identity(function).unwrap()));
        let int = type_reference(queries, function, "Int");
        let box_type = type_reference(queries, function, "Box<Int>");
        let holder = type_reference(queries, function, "Pending");
        let reader = type_reference(queries, function, "Reader");
        let empty = lookup(queries, function, "empty", "value")[0].clone();
        assert_eq!(
            lookup(queries, function, "Box", "type")[0]["kind"],
            "genericType"
        );
        assert_eq!(
            lookup(queries, function, "overloaded", "value")
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(lookup(queries, function, "hidden", "value"), json!([]));

        let types = request(
            queries,
            function,
            json!({"kind": "describeTypes", "types": [box_type, int, box_type]}),
        )
        .unwrap();
        assert_eq!(types["types"][0], types["types"][2]);
        let members = request(
            queries,
            function,
            json!({"kind": "describeMembers", "types": [box_type]}),
        )
        .unwrap();
        let members = &members["members"][0];
        let choose = &members["methods"][0];
        let default = &choose["parameters"][1]["calling"]["default"];
        let mut definition_only = default.clone();
        definition_only["arguments"] = Value::Null;
        let defaults = request(
            queries,
            function,
            json!({"kind": "describeDefaults", "defaults": [default, definition_only,
            {"owner": choose["declaration"], "parameter": 0}]}),
        )
        .unwrap();
        assert_eq!(defaults["defaults"][0]["applied"]["result"], int);
        assert_eq!(
            defaults["defaults"][0]["definition"]["result"]["kind"],
            "binder"
        );
        assert!(defaults["defaults"][1]["applied"].is_null());
        assert!(defaults["defaults"][2].is_null());

        let unknown = json!({"kind": "function", "id": "0".repeat(64)});
        let variant = lookup(queries, function, "Choice.Some", "value")[0].clone();
        let bodies = request(queries, function, json!({"kind": "describeBodies", "callables": [
            choose["declaration"], members["constructors"][0]["declaration"],
            members["properties"][0]["getter"]["body"]["declaration"], members["properties"][1]["getter"]["body"]["declaration"],
            method, empty, {"kind": "variantConstructor", "id": variant["id"]}, unknown,
        ]})).unwrap();
        let bodies = &bodies["bodies"];
        assert_eq!(bodies[0]["availability"], "available");
        assert_eq!(bodies[0]["kind"], "callable");
        assert_eq!(bodies[1]["kind"], "constructor");
        assert_eq!(bodies[1]["body"]["initialization"]["kind"], "classPrimary");
        assert_eq!(bodies[2]["availability"], "noBody");
        assert_eq!(bodies[3]["availability"], "available");
        assert_eq!(bodies[4], json!({"availability": "pendingGeneration"}));
        assert_eq!(bodies[5]["body"]["statements"], json!([]));
        assert_eq!(bodies[6], json!({"availability": "noBody"}));
        assert!(bodies[7].is_null());

        let nominal = json!({"kind": "nominal", "owner": members["declaration"]});
        let missing = json!({"kind": "callable", "callable": unknown});
        let annotation = lookup(queries, function, "Label", "type")[0]["id"].clone();
        let annotations = request(queries, function, json!({"kind": "describeAnnotations",
            "targets": [nominal, {"kind": "callable", "callable": empty}, missing], "declarations": [annotation]})).unwrap();
        assert_eq!(
            annotations["targets"][0][0]["arguments"][0]["elements"][1]["value"]["value"]["value"],
            "第二"
        );
        assert_eq!(annotations["targets"][1], json!([]));
        assert!(annotations["targets"][2].is_null());
        assert_eq!(annotations["declarations"][0]["annotation"], annotation);
        let parameter = json!({"kind": "parameter", "callable": choose["declaration"], "index": 1});
        let locations = request(
            queries,
            function,
            json!({"kind": "sourceLocation", "targets": [nominal, parameter, missing]}),
        )
        .unwrap();
        assert_eq!(
            locations["locations"][0]["source"]["path"],
            "src/user.scoop"
        );
        assert_eq!(locations["locations"][1], choose["parameters"][1]["origin"]);
        assert!(locations["locations"][2].is_null());
        assert_eq!(request(queries, function, json!({"kind": "typeRelation", "relation": "subtype", "left": holder, "right": reader})).unwrap(), json!({"holds": true}));
        assert_eq!(
            request(
                queries,
                function,
                json!({"kind": "typeRelation", "relation": "equal", "left": int, "right": box_type})
            )
            .unwrap(),
            json!({"holds": false})
        );
        Ok(vec![generated(queries, function, "{ return 1 }")])
    });
    compile(source, &mut driver).unwrap();
}
