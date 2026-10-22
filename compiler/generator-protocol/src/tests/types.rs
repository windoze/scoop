use super::*;

#[test]
fn type_json_preserves_structural_types_receiver_and_scoped_binders() {
    let plain =
        PersistentTypeId::from_source_declaration(&nominal_key(SourceNominalKind::Struct, 0))
            .unwrap();
    let generic =
        PersistentGenericTypeId::from_source_declaration(&nominal_key(SourceNominalKind::Class, 1))
            .unwrap();
    let ty = SignatureTypeKey::Function {
        effect: Effect::Suspend,
        receiver: Some(Box::new(SignatureTypeKey::NominalApplication {
            origin: generic,
            arguments: NonEmptyVec::from_first(SignatureTypeKey::Binder { depth: 1, index: 2 }, []),
        })),
        parameters: vec![SignatureTypeKey::RawPointer(Box::new(
            SignatureTypeKey::Nominal(plain),
        ))],
        result: Box::new(SignatureTypeKey::Tuple(NonEmptyVec::from_first(
            SignatureTypeKey::Companion(Box::new(SignatureTypeKey::Binder { depth: 0, index: 0 })),
            [SignatureTypeKey::NativeFunctionPointer {
                calling_convention: CallingConvention::C,
                parameters: vec![SignatureTypeKey::Binder { depth: 1, index: 0 }],
                result: Box::new(SignatureTypeKey::Nominal(plain)),
            }],
        ))),
    };
    let value = serde_json::to_value(QueryJson(&ty)).unwrap();
    assert_eq!(
        value,
        json!({
            "kind": "function", "effect": "suspend",
            "receiver": {"kind": "nominalApplication", "declaration": generic.to_string(), "arguments": [{"kind": "binder", "depth": 1, "index": 2}]},
            "parameters": [{"kind": "rawPointer", "pointee": {"kind": "nominal", "declaration": plain.to_string()}}],
            "result": {"kind": "tuple", "elements": [
                {"kind": "companion", "host": {"kind": "binder", "depth": 0, "index": 0}},
                {"kind": "nativeFunctionPointer", "callingConvention": "c", "parameters": [{"kind": "binder", "depth": 1, "index": 0}], "result": {"kind": "nominal", "declaration": plain.to_string()}}
            ]},
        })
    );
    let ordinary = SignatureTypeKey::Function {
        effect: Effect::Ordinary,
        receiver: None,
        parameters: vec![],
        result: Box::new(SignatureTypeKey::Nominal(plain)),
    };
    let value = serde_json::to_value(QueryJson(&ordinary)).unwrap();
    assert_eq!(value["receiver"], Value::Null);
    assert_eq!(value["parameters"], json!([]));
    assert_eq!(value["effect"], "ordinary");
}
