use super::*;

fn effects(
    payload: &hir::NominalSourceCallablePayloadV1,
    declaration: CallableTemplateOrigin,
) -> hir::NominalSourceCallablePayloadV1 {
    let e = payload.effects();
    let effects = hir::CallableSourceEffectsV1::try_new(
        e.execution(),
        hir::CallableSafetyV1::Unsafe,
        e.gc_effect(),
        e.implementation(),
        e.operator_role(),
        e.infix(),
    )
    .unwrap();
    hir::NominalSourceCallablePayloadV1::try_new(
        declaration,
        payload.owner(),
        payload.type_parameters().clone(),
        payload.parameters().clone(),
        payload.result().clone(),
        effects,
        payload.modality(),
        payload.slot_relations().clone(),
    )
    .unwrap()
}

#[test]
fn nested_source_replay_rejects_callable_constructor_and_property_contract_changes() {
    with_candidates(SOURCE, |fixture, sources, candidates, core| {
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let mut authority = members
                .bind_parameter_protocols(constructors, &sources.protocols)
                .unwrap();
            let mut counts = [0; 3];
            for record in &candidates.records {
                for (index, entry) in record
                    .payload()
                    .source_interface()
                    .source_support()
                    .records()
                    .iter()
                    .enumerate()
                {
                    let changed = match entry {
                        Entry::Callable(source)
                            if !matches!(
                                source.declaration(),
                                CallableTemplateOrigin::VariantConstructor(_)
                            ) =>
                        {
                            counts[0] += 1;
                            Entry::Callable(Box::new(
                                hir::NominalSupportCallableInterfaceV1::try_new(
                                    source.declaration(),
                                    source.declaration_access().clone(),
                                    effects(source.payload(), source.declaration()),
                                )
                                .unwrap(),
                            ))
                        }
                        Entry::Constructor(source) => {
                            counts[1] += 1;
                            Entry::Constructor(Box::new(
                                hir::NominalSupportConstructorInterfaceV1::try_new(
                                    source.declaration(),
                                    source.declaration_access().clone(),
                                    effects(
                                        source.payload(),
                                        CallableTemplateOrigin::Constructor(source.declaration()),
                                    ),
                                )
                                .unwrap(),
                            ))
                        }
                        Entry::Property(source) => {
                            counts[2] += 1;
                            let a = source.declaration_access();
                            let visibility =
                                if a.declared_visibility() == hir::DeclaredVisibilityV1::Internal {
                                    hir::DeclaredVisibilityV1::Public
                                } else {
                                    hir::DeclaredVisibilityV1::Internal
                                };
                            let access = hir::DeclarationAccessSourceV1::try_new(
                                visibility,
                                a.lexical_owners().to_vec(),
                                a.definition_origin().clone(),
                            )
                            .unwrap();
                            Entry::Property(Box::new(
                                hir::NominalSupportPropertyInterfaceV1::try_new(
                                    source.declaration(),
                                    access,
                                    source.payload().clone(),
                                )
                                .unwrap(),
                            ))
                        }
                        Entry::Callable(_) | Entry::NestedNominal(_) => continue,
                    };
                    let mut entries = record
                        .payload()
                        .source_interface()
                        .source_support()
                        .records()
                        .to_vec();
                    entries[index] = changed;
                    let forged = replacing(record, entries);
                    assert!(matches!(
                        authority.validate_nested_source(
                            &forged,
                            &candidates.protocols,
                            &fixture.source.entries().representations
                        ),
                        Err(Error::Contract { .. })
                    ));
                }
            }
            assert!(counts.into_iter().all(|n| n > 0));
        });
    });
}

#[test]
fn nested_source_replay_checks_contracts_inside_grandchild_support() {
    with_candidates(SOURCE, |fixture, sources, candidates, core| {
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let mut authority = members
                .bind_parameter_protocols(constructors, &sources.protocols)
                .unwrap();
            let record = candidates
                .records
                .iter()
                .find(|r| {
                    r.payload()
                        .source_interface()
                        .source_support()
                        .records()
                        .iter()
                        .any(|entry| match entry {
                            Entry::NestedNominal(child) => child
                                .payload()
                                .source_interface()
                                .source_support()
                                .records()
                                .iter()
                                .any(|entry| matches!(entry, Entry::Callable(_))),
                            _ => false,
                        })
                })
                .unwrap();
            let mut parent_entries = record
                .payload()
                .source_interface()
                .source_support()
                .records()
                .to_vec();
            let Entry::NestedNominal(child) = parent_entries
                .iter_mut()
                .find(|e| matches!(e, Entry::NestedNominal(_)))
                .unwrap()
            else {
                panic!("nested child");
            };
            let mut child_entries = child
                .payload()
                .source_interface()
                .source_support()
                .records()
                .to_vec();
            let Entry::Callable(callable) = child_entries
                .iter_mut()
                .find(|e| matches!(e, Entry::Callable(_)))
                .unwrap()
            else {
                panic!("child callable");
            };
            let declaration = hir::NestedSupportDeclarationV1::Callable(callable.declaration());
            **callable = hir::NominalSupportCallableInterfaceV1::try_new(
                callable.declaration(),
                callable.declaration_access().clone(),
                effects(callable.payload(), callable.declaration()),
            )
            .unwrap();
            **child = replacing(child, child_entries);
            let forged = replacing(record, parent_entries);
            let Error::Contract {
                declaration: actual,
                ..
            } = authority
                .validate_nested_source(
                    &forged,
                    &candidates.protocols,
                    &fixture.source.entries().representations,
                )
                .unwrap_err()
            else {
                panic!("recursive source contract rejection");
            };
            assert_eq!(actual, declaration);
        });
    });
}
