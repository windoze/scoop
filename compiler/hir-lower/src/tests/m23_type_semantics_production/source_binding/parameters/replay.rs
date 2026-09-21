use super::*;
use hir::{ProtectedParameterCallingKindV1 as Kind, ProtectedParameterCallingV1 as Calling};

use super::super::parameter_candidates::candidate;

#[test]
fn restored_protocol_authority_replays_candidates_and_rejects_calling_and_origin_swaps() {
    for source in [SOURCE, COMBINED] {
        with_source(source, |output, core| {
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let foundation = fixture.bind().unwrap();
            let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
            let core = inputs.protocols().fundamental_types();
            let entries = foundation.source().entries();
            let graph = hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
                entries.local_inheritance_edges.records().iter(),
                entries.source_roots.values().iter().copied(),
                &foundation,
                &mut meter(),
            )
            .unwrap();
            let mut protected = sources.protected(&foundation, core);
            let mut authority = sources.bind(&foundation, core, &mut meter()).unwrap();
            let mut checked_count = 0;
            for record in sources.protocols.records().iter().filter(|r| {
                matches!(
                    r.owner(),
                    CallableTemplateOrigin::Function(_)
                        | CallableTemplateOrigin::GenericFunction(_)
                )
            }) {
                let callable = sources.callables.get(record.owner()).unwrap();
                let checked = callable
                    .validate_source(&graph, &mut protected, &mut meter())
                    .unwrap();
                let original = candidate(record.owner(), record.parameters());
                assert_eq!(
                    original
                        .validate_protected(checked, &mut authority, &mut meter())
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
                            .validate_protected(checked, &mut authority, &mut meter())
                            .unwrap_err();
                        match (change_origin, error) {
                            (
                                false,
                                hir::ProtectedSourceSemanticError::Calling { position: actual },
                            ) => assert_eq!(actual, position as u32),
                            (
                                true,
                                hir::ProtectedSourceSemanticError::Foundation(Error::Origin {
                                    owner,
                                    position: actual,
                                }),
                            ) => {
                                assert_eq!(owner, record.owner());
                                assert_eq!(actual, position as u32);
                            }
                            (_, error) => panic!("unexpected protocol error: {error}"),
                        }
                    }
                }
                checked_count += 1;
            }
            assert!(checked_count >= 3);
        });
    }
}
