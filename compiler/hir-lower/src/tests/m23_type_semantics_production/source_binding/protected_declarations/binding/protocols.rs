use super::*;

#[test]
fn protected_binding_requires_zero_parameter_protocols_and_validates_calling_kinds() {
    with_binding(|authority, fixture, table, protocols| {
        let mut zero = 0;
        for protocol in protocols.records() {
            let owner = protocol.owner();
            zero += usize::from(protocol.parameters().is_empty());
            let missing = hir::CanonicalProtectedCallableSourceInterfacesV1::try_new(
                protocols
                    .records()
                    .iter()
                    .filter(|p| p.owner() != owner)
                    .cloned()
                    .collect(),
            )
            .unwrap();
            let Error::Source(error) = authority
                .validate_protected_declarations(
                    table,
                    &missing,
                    &fixture.source.entries().representations,
                    &mut meter(),
                )
                .unwrap_err()
            else {
                panic!("missing protocol");
            };
            assert!(
                matches!(*error, hir::NominalNestedBindingError::MissingProtocol(actual) if actual == owner)
            );
            let Some(parameter) = protocol.parameters().parameters().first() else {
                continue;
            };
            let calling =
                if parameter.calling().kind() == hir::ProtectedParameterCallingKindV1::Required {
                    hir::ProtectedParameterCallingV1::Default {
                        template: hir::ProtectedDefaultTemplateKeyV1::try_new(owner, 0).unwrap(),
                    }
                } else {
                    hir::ProtectedParameterCallingV1::Required
                };
            let mut parameters = protocol.parameters().parameters().to_vec();
            parameters[0] = hir::ProtectedSourceParameterV1::new(
                parameter.name().clone(),
                parameter.value_type().clone(),
                calling,
                parameter.definition_origin().clone(),
            );
            let changed = hir::ProtectedCallableSourceInterfaceV1::try_new(
                owner,
                hir::CanonicalProtectedSourceParametersV1::try_new(parameters).unwrap(),
            )
            .unwrap();
            let forged = hir::CanonicalProtectedCallableSourceInterfacesV1::try_new(
                protocols
                    .records()
                    .iter()
                    .map(|p| {
                        if p.owner() == owner {
                            changed.clone()
                        } else {
                            p.clone()
                        }
                    })
                    .collect(),
            )
            .unwrap();
            let Error::Source(error) = authority
                .validate_protected_declarations(
                    table,
                    &forged,
                    &fixture.source.entries().representations,
                    &mut meter(),
                )
                .unwrap_err()
            else {
                panic!("calling kind");
            };
            assert!(matches!(
                *error,
                hir::NominalNestedBindingError::Protocol(_)
            ));
        }
        assert!(zero > 0);
    });
}

#[test]
fn protected_binding_only_retains_protocols_used_by_the_verified_surface() {
    with_binding(|authority, fixture, table, protocols| {
        let all = hir::CanonicalProtectedCallableSourceInterfacesV1::try_new(
            authority
                .table()
                .records()
                .iter()
                .map(|r| {
                    super::super::super::parameter_candidates::candidate(r.owner(), r.parameters())
                })
                .collect(),
        )
        .unwrap();
        assert!(all.records().len() > protocols.records().len());
        let checked = authority
            .validate_protected_declarations(
                table,
                &all,
                &fixture.source.entries().representations,
                &mut meter(),
            )
            .unwrap();
        assert_eq!(
            checked
                .protocols()
                .map(|r| r.record().owner())
                .collect::<BTreeSet<_>>(),
            protocols.records().iter().map(|r| r.owner()).collect()
        );
    });
}
