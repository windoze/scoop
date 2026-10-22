use super::*;

#[test]
fn compiler_query_default_applications_use_original_providers_and_isolate_failures() {
    let source = r#"
        annotation class Mark
        interface Picker<A, B> { fun choose(first: B, second: B = first): B }
        class Choice<T> : Picker<String, T> { override fun choose(first: T, second: T): T = second }
        class Item
        public fun <T: ref> reference(first: T, second: T = first): T = second
        @Mark fun inspect() {}
    "#;
    let mut driver = Generator(|queries: &mut dyn SemanticQueries| {
        let function = marked_functions(queries)[0];
        let int = type_reference(queries, function, "Int");
        let item = type_reference(queries, function, "Item");
        let choice = type_reference(queries, function, "Choice<Int>");
        let members = request(
            queries,
            function,
            json!({"kind": "describeMembers", "types": [choice]}),
        )
        .unwrap();
        let default = &members["members"][0]["methods"][0]["parameters"][1]["calling"]["default"];
        assert_eq!(default["arguments"].as_array().unwrap().len(), 2);
        let output = request(
            queries,
            function,
            json!({"kind": "describeDefaults", "defaults": [default]}),
        )
        .unwrap();
        let value = &output["defaults"][0];
        assert_ne!(
            value["definition"]["key"]["owner"],
            value["definition"]["definitionRoot"]
        );
        assert_eq!(value["applied"]["result"], int);
        assert_eq!(value["applied"]["key"], value["definition"]["key"]);
        let declaration = lookup(queries, function, "reference", "value")[0].clone();
        let query = |arguments: Value| {
            json!({"kind": "describeDefaults", "defaults": [
            {"owner": declaration, "parameter": 1, "arguments": arguments}]})
        };
        let invalid = request(queries, function, query(json!([int]))).unwrap_err();
        assert!(
            invalid
                .iter()
                .any(|diagnostic| diagnostic.message.contains("must satisfy `ref`")),
            "{invalid:?}"
        );
        let invalid = request(queries, function, query(json!([]))).unwrap_err();
        assert!(
            invalid
                .iter()
                .any(|diagnostic| diagnostic.message.contains("mapping has 0 arguments")),
            "{invalid:?}"
        );
        let valid = request(queries, function, query(json!([item]))).unwrap();
        assert_eq!(valid["defaults"][0]["applied"]["result"], item);
        let unknown = json!({"kind": "nominal", "declaration": "0".repeat(64)});
        assert!(
            request(
                queries,
                function,
                json!({"kind": "describeTypes", "types": [unknown]})
            )
            .is_err()
        );
        assert!(
            request(
                queries,
                function,
                json!({"kind": "lookupExport", "provider": "0".repeat(64),
            "path": ["Choice"], "namespace": "type"})
            )
            .is_err()
        );
        let provider = queries.current_cone().to_string();
        for path in [json!([]), json!([""])] {
            assert!(
                request(
                    queries,
                    function,
                    json!({"kind": "lookupExport", "provider": provider,
                "path": path, "namespace": "type"})
                )
                .is_err()
            );
        }
        let valid = request(
            queries,
            function,
            json!({"kind": "describeTypes", "types": [int]}),
        )
        .unwrap();
        assert_eq!(valid["types"][0]["type"], int);
        Ok(Vec::new())
    });
    compile(source, &mut driver).unwrap();
}
