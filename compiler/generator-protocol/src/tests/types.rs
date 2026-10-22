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
    struct References(PersistentTypeId, PersistentGenericTypeId);
    impl PersistentIdResolver<PersistentTypeId> for References {
        type Error = String;
        fn resolve(
            &mut self,
            id: DecodedPersistentId<PersistentTypeId>,
        ) -> Result<PersistentTypeId, Self::Error> {
            id.verify(self.0).map_err(|error| error.to_string())
        }
    }
    impl PersistentIdResolver<PersistentGenericTypeId> for References {
        type Error = String;
        fn resolve(
            &mut self,
            id: DecodedPersistentId<PersistentGenericTypeId>,
        ) -> Result<PersistentGenericTypeId, Self::Error> {
            id.verify(self.1).map_err(|error| error.to_string())
        }
    }
    let decoded: TypeReference = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        decoded.0.resolve(&mut References(plain, generic)).unwrap(),
        ty
    );
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

#[test]
fn type_inputs_reject_invalid_shapes_and_keep_nonempty_single_element_tuples() {
    let binder = json!({"kind": "binder", "depth": 0, "index": 0});
    let single = json!({"kind": "tuple", "elements": [binder.clone()]});
    let decoded: TypeReference = serde_json::from_value(single).unwrap();
    assert!(
        matches!(decoded.0, DecodedSignatureTypeKey::Tuple(elements) if elements.as_slice().len() == 1)
    );
    for value in [
        json!({"kind": "tuple", "elements": []}),
        json!({"kind": "nominalApplication", "declaration": "0".repeat(64), "arguments": []}),
        json!({"kind": "nominal", "declaration": "0".repeat(63)}),
        json!({"kind": "binder", "depth": -1, "index": 0}),
        json!({"kind": "binder", "depth": 0, "index": 4294967296_u64}),
        json!({"kind": "function", "effect": "async", "parameters": [], "result": binder}),
        json!({"kind": "nativeFunctionPointer", "callingConvention": "scoop", "parameters": [], "result": binder}),
        json!({"kind": "companion"}),
        json!({"kind": "unresolved", "name": "T"}),
    ] {
        assert!(
            serde_json::from_value::<TypeReference>(value.clone()).is_err(),
            "{value}"
        );
    }
}
