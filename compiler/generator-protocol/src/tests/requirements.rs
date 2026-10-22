use super::*;

#[test]
fn requirement_json_distinguishes_inference_named_spread_arguments_and_effects() {
    let binder = SignatureTypeKey::Binder { depth: 0, index: 1 };
    let requirement = MemberRequirementKey {
        subject: SignatureTypeKey::Companion(Box::new(binder.clone())),
        call: RequiredMemberKey::Method {
            name: "make".into(),
            type_arguments: vec![None, Some(binder.clone())],
        },
        arguments: vec![RequiredArgumentKey {
            name: Some("values".into()),
            spread: true,
            ty: binder.clone(),
        }],
        result: binder,
        effects: RequiredEffectsKey {
            is_suspend: true,
            is_unsafe: true,
            is_no_gc: true,
        },
    };
    let value = serde_json::to_value(QueryJson(&requirement)).unwrap();
    assert_eq!(value["subject"]["kind"], "companion");
    assert_eq!(value["call"]["kind"], "method");
    assert_eq!(value["call"]["typeArguments"][0], Value::Null);
    assert_eq!(value["call"]["typeArguments"][1], value["result"]);
    assert_eq!(value["arguments"][0]["name"], "values");
    assert_eq!(value["arguments"][0]["spread"], true);
    assert_eq!(
        value["effects"],
        json!({"suspend": true, "unsafe": true, "noGc": true})
    );
    assert_eq!(
        serde_json::to_value(QueryJson(
            &RequiredMemberKey::<SignatureTypeKey>::Constructor
        ))
        .unwrap(),
        json!({"kind": "constructor"})
    );
    assert_eq!(
        serde_json::to_value(QueryJson(&hir::OperatorKind::Component {
            index: std::num::NonZeroU32::new(2).unwrap()
        }))
        .unwrap(),
        json!({"kind": "component", "index": 2})
    );
    assert_eq!(
        serde_json::to_value(QueryJson(&hir::ContextParameterLabel::Unnamed)).unwrap(),
        json!({"kind": "unnamed"})
    );
}
