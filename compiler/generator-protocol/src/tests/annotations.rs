use super::*;

#[test]
fn annotation_target_messages_keep_receiver_storage_and_parameter_identity_domains() {
    let id = "ab".repeat(32);
    let callable = CallableDeclaration::GenericFunction(id.parse().unwrap());
    let property = PropertyDeclaration::ExtensionProperty(id.parse().unwrap());
    let parameter = AnnotationTarget::Parameter { callable, index: 3 };
    let receiver = AnnotationTarget::CallableReceiver { callable };
    let storage = AnnotationTarget::PropertyStorage {
        property,
        role: AnnotationStorageRole::Delegate,
    };
    for target in [
        parameter,
        receiver,
        storage,
        AnnotationTarget::PropertyReceiver { property },
        AnnotationTarget::Field {
            field: id.parse().unwrap(),
        },
        AnnotationTarget::VariantField {
            field: id.parse().unwrap(),
        },
    ] {
        let json = serde_json::to_value(target).unwrap();
        assert_eq!(
            serde_json::from_value::<AnnotationTarget>(json).unwrap(),
            target
        );
    }
    assert_eq!(
        serde_json::to_value(parameter).unwrap(),
        json!({
            "kind": "parameter", "callable": {"kind": "genericFunction", "id": id}, "index": 3,
        })
    );
    assert_eq!(
        serde_json::to_value(storage).unwrap(),
        json!({
            "kind": "propertyStorage", "property": {"kind": "extensionProperty", "id": id}, "role": "delegate",
        })
    );
    for invalid in [
        json!({"kind": "parameter", "callable": callable, "index": -1}),
        json!({"kind": "parameter", "callable": callable, "index": u64::from(u32::MAX) + 1}),
        json!({"kind": "propertyReceiver", "property": id}),
        json!({"kind": "callableReceiver", "callable": {"kind": "concrete", "id": id}}),
        json!({"kind": "field", "field": 42}),
    ] {
        assert!(serde_json::from_value::<AnnotationTarget>(invalid).is_err());
    }
}
