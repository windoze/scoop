use super::*;

#[test]
fn compiler_query_envelopes_decode_all_eight_families_and_keep_typed_context() {
    let id = ConeIdentity::CORE.to_string();
    let ty = json!({"kind": "nominal", "declaration": id});
    let context = json!({"kind": "genericFunction", "id": id});
    for query in [
        json!({"kind": "describeTypes", "types": [ty]}),
        json!({"kind": "describeMembers", "types": []}),
        json!({"kind": "describeAnnotations", "targets": [], "declarations": [id]}),
        json!({"kind": "describeDefaults", "defaults": [{"owner": context, "parameter": 2}]}),
        json!({"kind": "describeBodies", "callables": [context]}),
        json!({"kind": "typeRelation", "relation": "subtype", "left": ty, "right": ty}),
        json!({"kind": "lookupExport", "provider": id, "path": ["scoop", "core", "Option"], "namespace": "type"}),
        json!({"kind": "sourceLocation", "targets": [{"kind": "parameter", "callable": context, "index": 1}]}),
    ] {
        let message = serde_json::from_value::<CompilerQuery>(json!({
            "context": context, "query": query,
        }))
        .unwrap();
        assert!(matches!(message.context, MethodId::GenericFunction(_)));
    }
    for arguments in [None, Some(Value::Null), Some(json!([])), Some(json!([ty]))] {
        let mut default = json!({"owner": context, "parameter": 2});
        if let Some(arguments) = &arguments {
            default["arguments"] = arguments.clone();
        }
        let reference = serde_json::from_value::<DefaultReference>(default).unwrap();
        assert_eq!(
            reference.arguments.is_some(),
            arguments.is_some_and(|value| !value.is_null())
        );
    }
}

#[test]
fn malformed_query_variants_and_positions_fail_before_semantic_dispatch() {
    let id = ConeIdentity::CORE.to_string();
    let callable = json!({"kind": "function", "id": id});
    let ty = json!({"kind": "nominal", "declaration": id});
    for query in [
        json!({"kind": "invoke", "callable": callable}),
        json!({"kind": "describeAnnotations", "targets": []}),
        json!({"kind": "describeDefaults", "defaults": [{"owner": callable, "parameter": -1}]}),
        json!({"kind": "describeDefaults", "defaults": [{"owner": callable, "parameter": 4_294_967_296_u64}]}),
        json!({"kind": "describeBodies", "callables": [{"kind": "concrete", "id": id}]}),
        json!({"kind": "typeRelation", "relation": "convertible", "left": ty, "right": ty}),
        json!({"kind": "lookupExport", "provider": id, "path": ["Box"], "namespace": "any"}),
        json!({"kind": "lookupExport", "provider": "missing", "path": ["Box"], "namespace": "type"}),
    ] {
        assert!(
            serde_json::from_value::<CompilerQuery>(json!({
                "context": callable, "query": query,
            }))
            .is_err()
        );
    }
    assert!(
        serde_json::from_value::<CompilerQuery>(json!({
            "context": {"kind": "constructor", "id": id},
            "query": {"kind": "describeTypes", "types": []},
        }))
        .is_err()
    );
}
