use super::*;

#[test]
fn source_protocol_keeps_default_kind_owner_position_and_parameter_names() {
    let origin = ExpressionFixture::new().origin();
    let owner = owner();
    let new = ProtectedCallableSourceInterfaceV1::try_new(
        owner,
        CanonicalProtectedSourceParametersV1::try_new(vec![ProtectedSourceParameterV1::new(
            CanonicalIdentifier::new("value").unwrap(),
            unit(),
            ProtectedParameterCallingV1::Default {
                template: ProtectedDefaultTemplateKeyV1::try_new(owner, 0).unwrap(),
            },
            origin.clone(),
        )])
        .unwrap(),
    )
    .unwrap();
    for (name, calling, expected) in [
        (
            "value",
            CallableParameterCallingV1::Default {
                template: ExportDefaultTemplateKeyV1::new(owner, 0),
            },
            true,
        ),
        (
            "renamed",
            CallableParameterCallingV1::Default {
                template: ExportDefaultTemplateKeyV1::new(owner, 0),
            },
            false,
        ),
        ("value", CallableParameterCallingV1::Required, false),
        (
            "value",
            CallableParameterCallingV1::VarargDefault {
                template: ExportDefaultTemplateKeyV1::new(owner, 0),
                element_type: unit(),
            },
            false,
        ),
    ] {
        let old = CallableSourceInterfaceV1::try_new(
            owner,
            CanonicalCallableSourceParametersV1::try_new(vec![CallableSourceParameterV1::new(
                CanonicalIdentifier::new(name).unwrap(),
                unit(),
                calling,
                origin.clone(),
            )])
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            super::super::protocols::equal(&new, &old, &path()).unwrap(),
            expected
        );
    }
}

#[test]
fn metadata_binders_distinguish_value_and_reference_bounds() {
    let make = |bound| {
        CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            bound,
        )])
        .unwrap()
    };
    let value = make(TypeParameterBoundsV1::Value);
    let reference = make(TypeParameterBoundsV1::Ref);
    assert!(!binders(&value, &reference, &path()).unwrap());
    assert!(binders(&value, &value, &path()).unwrap());
}
