use super::*;

fn effects(
    payload: &hir::ProtectedCallablePayloadV1,
    owner: CallableTemplateOrigin,
) -> hir::ProtectedCallablePayloadV1 {
    let e = payload.effects();
    hir::ProtectedCallablePayloadV1::try_new(
        owner,
        payload.owner(),
        payload.type_parameters().clone(),
        payload.parameters().clone(),
        payload.result().clone(),
        hir::CallableSourceEffectsV1::try_new(
            e.execution(),
            hir::CallableSafetyV1::Unsafe,
            e.gc_effect(),
            e.implementation(),
            e.operator_role(),
            e.infix(),
        )
        .unwrap(),
        payload.modality(),
        payload.slot_relations().clone(),
    )
    .unwrap()
}

#[test]
fn protected_binding_compares_every_kind_of_complete_source_contract() {
    with_binding(|authority, fixture, table, protocols| {
        let mut counts = [0; 4];
        for (index, record) in table.records().iter().enumerate() {
            let changed = match record {
                Declaration::Callable(r)
                    if !matches!(r.declaration(), CallableTemplateOrigin::Accessor(_)) =>
                {
                    counts[0] += 1;
                    Declaration::Callable(Box::new(
                        hir::ProtectedCallableInterfaceV1::try_new(
                            r.declaration(),
                            r.declaration_access().clone(),
                            effects(r.payload(), r.declaration()),
                        )
                        .unwrap(),
                    ))
                }
                Declaration::Constructor(r) => {
                    counts[1] += 1;
                    Declaration::Constructor(Box::new(
                        hir::ProtectedConstructorInterfaceV1::try_new(
                            r.declaration(),
                            r.declaration_access().clone(),
                            effects(
                                r.payload(),
                                CallableTemplateOrigin::Constructor(r.declaration()),
                            ),
                        )
                        .unwrap(),
                    ))
                }
                Declaration::Property(r) => {
                    let p = r.payload();
                    let hir::ProtectedPropertyMutabilityV1::ReadWrite {
                        setter,
                        setter_access: a,
                    } = p.mutability()
                    else {
                        continue;
                    };
                    counts[2] += 1;
                    let setter_access = hir::DeclarationAccessSourceV1::try_new(
                        hir::DeclaredVisibilityV1::Internal,
                        a.lexical_owners().to_vec(),
                        a.definition_origin().clone(),
                    )
                    .unwrap();
                    let payload = hir::ProtectedPropertyPayloadV1::try_new(
                        p.owner(),
                        p.value_type().clone(),
                        p.getter(),
                        hir::ProtectedPropertyMutabilityV1::ReadWrite {
                            setter: *setter,
                            setter_access,
                        },
                        p.representation(),
                        p.slot_relations().clone(),
                    )
                    .unwrap();
                    Declaration::Property(Box::new(
                        hir::ProtectedPropertyInterfaceV1::try_new(
                            r.declaration(),
                            r.declaration_access().clone(),
                            payload,
                        )
                        .unwrap(),
                    ))
                }
                Declaration::NestedNominal(r)
                    if r.payload().source_interface().kind() == hir::PublicNominalKindV1::Class =>
                {
                    counts[3] += 1;
                    let s = r.payload().source_interface();
                    let modality = if s.modality() == hir::NominalInheritanceModalityV1::Final {
                        hir::NominalInheritanceModalityV1::Open
                    } else {
                        hir::NominalInheritanceModalityV1::Final
                    };
                    let interface = hir::ProtectedNestedSourceInterfaceV1::try_new(
                        s.kind(),
                        modality,
                        s.type_parameters().clone(),
                        s.supertypes().clone(),
                        s.constructors().clone(),
                        s.members().clone(),
                        s.children().clone(),
                        s.source_shape().clone(),
                        s.source_support().clone(),
                    )
                    .unwrap();
                    let payload =
                        hir::ProtectedNestedNominalPayloadV1::try_new(r.declaration(), interface)
                            .unwrap();
                    Declaration::NestedNominal(Box::new(
                        hir::ProtectedNestedNominalInterfaceV1::try_new(
                            r.declaration(),
                            r.declaration_access().clone(),
                            payload,
                        )
                        .unwrap(),
                    ))
                }
                _ => continue,
            };
            let mut records = table.records().to_vec();
            records[index] = changed;
            let forged = hir::CanonicalProtectedDeclarationInterfacesV1::try_new(records).unwrap();
            let Error::Source(error) = authority
                .validate_protected_declarations(
                    &forged,
                    protocols,
                    &fixture.source.entries().representations,
                )
                .unwrap_err()
            else {
                panic!("changed source contract");
            };
            assert!(matches!(
                *error,
                hir::NominalNestedBindingError::Contract { .. }
            ));
        }
        assert!(counts.into_iter().all(|count| count > 0));
    });
}
