use super::*;
use hir::{ProtectedParameterCallingKindV1 as Kind, ProtectedParameterCallingV1 as Calling};

use super::super::parameter_candidates::candidate;

#[test]
fn complete_protocol_authority_replays_methods_variants_and_rejects_candidate_swaps() {
    with_sources(SOURCE, |_, fixture, sources, core| {
        let foundation = fixture.bind().unwrap();
        let nominals = foundation
            .bind_nominal_sources(&sources.members.nominals, &mut meter())
            .unwrap();
        let mut candidate_members = nominals
            .bind_member_sources(
                &sources.members.properties,
                &sources.members.callables,
                core,
                &mut meter(),
            )
            .unwrap();
        let entries = foundation.source().entries();
        let graph = hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            entries.local_inheritance_edges.records().iter(),
            entries.source_roots.values().iter().copied(),
            &foundation,
            &mut meter(),
        )
        .unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let mut authority = members
                .bind_parameter_protocols(constructors, &sources.protocols, &mut meter())
                .unwrap();
            let mut checked_count = 0;
            for record in sources.protocols.records().iter().filter(|r| {
                !matches!(r.owner(), CallableTemplateOrigin::Constructor(_))
            }) {
                let callable = sources.members.callables.get(record.owner()).unwrap();
                let checked = callable
                    .validate_source(&graph, &mut candidate_members, &mut meter())
                    .unwrap();
                let original = candidate(record.owner(), record.parameters());
                assert_eq!(
                    original
                        .validate_nominal_support(checked, &mut authority, &mut meter())
                        .unwrap()
                        .record(),
                    &original
                );
                for (position, parameter) in original.parameters().parameters().iter().enumerate() {
                    for change_origin in [false, true] {
                        let mut parameters = original.parameters().parameters().to_vec();
                        let (calling, origin) = if change_origin {
                            let other = sources
                                .protocols
                                .records()
                                .iter()
                                .flat_map(|r| r.parameters())
                                .map(|p| p.definition_origin())
                                .find(|other| *other != parameter.definition_origin())
                                .unwrap();
                            assert_eq!(
                                other.origin().source(),
                                parameter.definition_origin().origin().source()
                            );
                            (parameter.calling().clone(), other.clone())
                        } else {
                            let calling = if parameter.calling().kind() == Kind::Required {
                                Calling::Default {
                                    template: hir::ProtectedDefaultTemplateKeyV1::try_new(
                                        record.owner(),
                                        position as u32,
                                    )
                                    .unwrap(),
                                }
                            } else {
                                Calling::Required
                            };
                            (calling, parameter.definition_origin().clone())
                        };
                        parameters[position] = hir::ProtectedSourceParameterV1::new(
                            parameter.name().clone(),
                            parameter.value_type().clone(),
                            calling,
                            origin,
                        );
                        let forged = hir::ProtectedCallableSourceInterfaceV1::try_new(
                            record.owner(),
                            hir::CanonicalProtectedSourceParametersV1::try_new(parameters).unwrap(),
                        )
                        .unwrap();
                        let error = forged
                            .validate_nominal_support(checked, &mut authority, &mut meter())
                            .unwrap_err();
                        match (change_origin, error) {
                            (
                                false,
                                hir::ProtectedSourceSemanticError::Calling { position: actual },
                            ) => assert_eq!(actual, position as u32),
                            (
                                true,
                                hir::ProtectedSourceSemanticError::Foundation(Error::Contract(error)),
                            ) => {
                                let ContractError::Origin { owner, position: actual } = *error else {
                                    panic!("parameter origin rejection");
                                };
                                assert_eq!((owner, actual), (record.owner(), position as u32));
                            }
                            (_, error) => panic!("unexpected protocol error: {error}"),
                        }
                    }
                }
                checked_count += 1;
            }
            assert!(checked_count >= 8);
            // Constructor parameters use the same position-addressed authority.
            for record in sources.protocols.records().iter().filter(|r| {
                matches!(r.owner(), CallableTemplateOrigin::Constructor(_))
            }) {
                for (position, parameter) in record.parameters().iter().enumerate() {
                    hir::ProtectedSourceProtocolSemanticAuthority::validate_source_parameter_origin(
                        &authority,
                        record.owner(),
                        position as u32,
                        parameter.definition_origin(),
                    )
                    .unwrap();
                    assert_eq!(
                        hir::ProtectedSourceProtocolSemanticAuthority::source_parameter_calling_kind(
                            &authority,
                            record.owner(),
                            position as u32,
                        )
                        .unwrap(),
                        parameter.calling_kind()
                    );
                }
            }
        });
    });
}
