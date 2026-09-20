use super::*;

#[test]
fn nested_protocol_replay_requires_even_zero_parameter_records_and_checks_calling_kinds() {
    with_candidates(SOURCE, |fixture, sources, candidates, core| {
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let mut authority = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            let mut zero_parameters = 0;
            for record in &candidates.records {
                let mut required = BTreeSet::new();
                collect_protocols(record, &mut required);
                for owner in required {
                    let protocol = candidates.protocols.get(owner).unwrap();
                    zero_parameters += usize::from(protocol.parameters().is_empty());
                    let missing = hir::CanonicalProtectedCallableSourceInterfacesV1::try_new(
                        candidates
                            .protocols
                            .records()
                            .iter()
                            .filter(|p| p.owner() != owner)
                            .cloned()
                            .collect(),
                    )
                    .unwrap();
                    let Error::MissingProtocol(actual) = authority
                        .validate_nested_source(
                            record,
                            &missing,
                            &fixture.source.entries().representations,
                            &mut meter(),
                        )
                        .unwrap_err()
                    else {
                        panic!("missing protocol");
                    };
                    assert_eq!(actual, owner);
                    for (position, parameter) in
                        protocol.parameters().parameters().iter().enumerate()
                    {
                        let calling = if parameter.calling().kind()
                            == hir::ProtectedParameterCallingKindV1::Required
                        {
                            hir::ProtectedParameterCallingV1::Default {
                                template: hir::ProtectedDefaultTemplateKeyV1::try_new(
                                    owner,
                                    position as u32,
                                )
                                .unwrap(),
                            }
                        } else {
                            hir::ProtectedParameterCallingV1::Required
                        };
                        let mut parameters = protocol.parameters().parameters().to_vec();
                        parameters[position] = hir::ProtectedSourceParameterV1::new(
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
                            candidates
                                .protocols
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
                        let Error::Protocol(error) = authority
                            .validate_nested_source(
                                record,
                                &forged,
                                &fixture.source.entries().representations,
                                &mut meter(),
                            )
                            .unwrap_err()
                        else {
                            panic!("calling protocol rejection");
                        };
                        let hir::ProtectedSourceSemanticError::Calling { position: actual } =
                            *error
                        else {
                            panic!("calling kind differs");
                        };
                        assert_eq!(actual, position as u32);
                    }
                }
            }
            assert!(zero_parameters > 0);
        });
    });
}

#[test]
fn bound_protocol_entry_rejects_other_valid_origins_for_constructors_and_variants() {
    with_candidates(SOURCE, |fixture, sources, candidates, core| {
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let mut authority = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            let mut roles = BTreeSet::new();
            for protocol in candidates
                .protocols
                .records()
                .iter()
                .filter(|p| !p.parameters().is_empty())
            {
                authority
                    .validate_source_protocol(protocol, &mut meter())
                    .unwrap();
                let owner = protocol.owner();
                roles.insert(match owner {
                    CallableTemplateOrigin::Constructor(_) => 0,
                    CallableTemplateOrigin::VariantConstructor(_) => 1,
                    CallableTemplateOrigin::GenericFunction(_) => 2,
                    CallableTemplateOrigin::Function(_) => 3,
                    CallableTemplateOrigin::Accessor(_) => {
                        panic!("accessors have no source protocol")
                    }
                });
                let first = &protocol.parameters().parameters()[0];
                let other = candidates
                    .protocols
                    .records()
                    .iter()
                    .flat_map(|r| r.parameters().parameters())
                    .map(|p| p.definition_origin())
                    .find(|origin| *origin != first.definition_origin())
                    .unwrap();
                assert_eq!(
                    other.origin().source(),
                    first.definition_origin().origin().source()
                );
                let mut parameters = protocol.parameters().parameters().to_vec();
                parameters[0] = hir::ProtectedSourceParameterV1::new(
                    first.name().clone(),
                    first.value_type().clone(),
                    first.calling().clone(),
                    other.clone(),
                );
                let forged = hir::ProtectedCallableSourceInterfaceV1::try_new(
                    owner,
                    hir::CanonicalProtectedSourceParametersV1::try_new(parameters).unwrap(),
                )
                .unwrap();
                let hir::ProtectedSourceSemanticError::Foundation(
                    hir::NominalParameterBindingError::Contract(error),
                ) = authority
                    .validate_source_protocol(&forged, &mut meter())
                    .unwrap_err()
                else {
                    panic!("bound origin rejection");
                };
                let hir::SourceParameterContractError::Origin {
                    owner: actual,
                    position: 0,
                } = *error
                else {
                    panic!("parameter origin differs");
                };
                assert_eq!(actual, owner);
            }
            assert_eq!(roles, BTreeSet::from([0, 1, 2, 3]));
        });
    });
}
