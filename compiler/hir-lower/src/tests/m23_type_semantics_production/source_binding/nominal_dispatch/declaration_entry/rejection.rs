use super::*;

#[test]
fn section_entry_rejects_complete_payload_tampering_after_identity_checks_pass() {
    with_entry(
        ENTRY,
        |authority, table, protocols, representations, graph| {
            let source = table
                .records()
                .iter()
                .find_map(|r| match r {
                    Declaration::Callable(r)
                        if matches!(r.declaration(), CallableTemplateOrigin::Function(_)) =>
                    {
                        Some(r)
                    }
                    _ => None,
                })
                .unwrap();
            let p = source.payload();
            let e = p.effects();
            let effects = hir::CallableSourceEffectsV1::try_new(
                e.execution(),
                hir::CallableSafetyV1::Unsafe,
                e.gc_effect(),
                e.implementation(),
                e.operator_role(),
                e.infix(),
            )
            .unwrap();
            let payload = hir::ProtectedCallablePayloadV1::try_new(
                source.declaration(),
                p.owner(),
                p.type_parameters().clone(),
                p.parameters().clone(),
                p.result().clone(),
                effects,
                p.modality(),
                p.slot_relations().clone(),
            )
            .unwrap();
            let changed = Declaration::Callable(Box::new(
                hir::ProtectedCallableInterfaceV1::try_new(
                    source.declaration(),
                    source.declaration_access().clone(),
                    payload,
                )
                .unwrap(),
            ));
            let forged = hir::CanonicalProtectedDeclarationInterfacesV1::try_new(
                table
                    .records()
                    .iter()
                    .map(|r| {
                        if r.reference() == changed.reference() {
                            changed.clone()
                        } else {
                            r.clone()
                        }
                    })
                    .collect(),
            )
            .unwrap();
            forged
                .validate_sources(graph, representations, authority, &mut meter())
                .unwrap();
            let error = authority
                .validate_protected_sources(
                    &forged,
                    protocols,
                    representations,
                    graph,
                    &mut meter(),
                )
                .unwrap_err();
            assert!(matches!(
                error,
                hir::ProtectedDeclarationSemanticError::Foundation(Error::Protected(_))
            ));
        },
    );
}

#[test]
fn section_entry_requires_every_protected_root_and_zero_parameter_protocol() {
    with_entry(
        ENTRY,
        |authority, table, protocols, representations, graph| {
            for omitted in table.records() {
                let forged = hir::CanonicalProtectedDeclarationInterfacesV1::try_new(
                    table
                        .records()
                        .iter()
                        .filter(|r| r.reference() != omitted.reference())
                        .cloned()
                        .collect(),
                )
                .unwrap();
                assert!(
                    authority
                        .validate_protected_sources(
                            &forged,
                            protocols,
                            representations,
                            graph,
                            &mut meter()
                        )
                        .is_err()
                );
            }
            let mut zero = false;
            for omitted in protocols.records() {
                zero |= omitted.parameters().parameters().is_empty();
                let forged = hir::CanonicalProtectedCallableSourceInterfacesV1::try_new(
                    protocols
                        .records()
                        .iter()
                        .filter(|r| r.owner() != omitted.owner())
                        .cloned()
                        .collect(),
                )
                .unwrap();
                assert!(
                    authority
                        .validate_protected_sources(
                            table,
                            &forged,
                            representations,
                            graph,
                            &mut meter()
                        )
                        .is_err()
                );
            }
            assert!(zero);
        },
    );
}

#[test]
fn section_entry_preserves_resource_errors_without_publishing_a_partial_proof() {
    with_entry(
        ENTRY,
        |authority, table, protocols, representations, graph| {
            for limits in [
                DecodeLimits {
                    decoded_nodes: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    validation_work_units: 0,
                    ..DecodeLimits::default()
                },
            ] {
                let error = authority
                    .validate_protected_sources(
                        table,
                        protocols,
                        representations,
                        graph,
                        &mut BudgetMeter::new(limits),
                    )
                    .unwrap_err();
                assert!(matches!(
                    error,
                    hir::ProtectedDeclarationSemanticError::Resource(_)
                ));
            }
        },
    );
}
