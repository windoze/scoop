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
            super::super::protocols::equal(&new, &old, &mut meter(), &path()).unwrap(),
            expected
        );
    }
}

#[test]
fn metadata_binders_compare_typed_bounds_and_pay_for_wide_signatures() {
    let make = |bound| {
        CanonicalBinderListV1::try_new(vec![TypeParameterBinderV1::new(
            CanonicalIdentifier::new("T").unwrap(),
            bound,
        )])
        .unwrap()
    };
    let value = make(TypeParameterBoundsV1::Value);
    let reference = make(TypeParameterBoundsV1::Ref);
    assert!(!binders(&value, &reference, &mut meter(), &path()).unwrap());
    assert!(binders(&value, &value, &mut meter(), &path()).unwrap());
    let signature =
        SignatureTypeKey::Tuple(scoop_identity::NonEmptyVec::new(vec![unit(); 4096]).unwrap());
    let key = SourceDeclarationKey::nominal(
        crate::cross_cone_type_semantics::inheritance::tests::support::site(&[]),
        CanonicalIdentifier::new("Bound").unwrap(),
        scoop_identity::SourceNominalKind::Class,
        1,
    );
    let bound_type = SignatureTypeKey::NominalApplication {
        origin: scoop_identity::PersistentGenericTypeId::from_source_declaration(&key).unwrap(),
        arguments: scoop_identity::NonEmptyVec::new(vec![signature]).unwrap(),
    };
    let bound = make(TypeParameterBoundsV1::Nominal(
        NominalTypeParameterBoundsV1::try_new(
            Some(bound_type),
            CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
        )
        .unwrap(),
    ));
    let mut resources = BudgetMeter::new(DecodeLimits {
        semantic_table_entries: 128,
        ..DecodeLimits::default()
    });
    limit(
        binders(&bound, &bound, &mut resources, &path()).unwrap_err(),
        ResourceKind::SemanticTableEntries,
    );
}
