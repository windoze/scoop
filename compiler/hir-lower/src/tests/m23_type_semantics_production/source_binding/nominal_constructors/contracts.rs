use super::*;

#[test]
fn complete_constructor_origin_must_match_its_exact_foundation_subject() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let nominals = foundation.bind_nominal_sources(&sources.nominals).unwrap();
        let record = sources.named(&fixture, "Box", &["value"]);
        let access = record.declaration_access();
        let other = sources
            .named(&fixture, "Box", &["stored", "count"])
            .declaration_access()
            .definition_origin();
        assert!(foundation.contains_definition_source(other));
        let mut forged = sources.clone();
        forged.replace(
            Record::try_new(
                record.declaration(),
                hir::DeclarationAccessSourceV1::try_new(
                    access.declared_visibility(),
                    access.lexical_owners().to_vec(),
                    other.clone(),
                )
                .unwrap(),
                record.payload().clone(),
            )
            .unwrap(),
        );
        assert!(
            matches!(nominals.bind_constructor_sources(&forged.constructors), Err(Error::Origin(id)) if id == record.declaration())
        );
    });
}

#[test]
fn complete_constructor_signatures_reject_changed_parameter_order_self_type_and_binders() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let nominals = foundation.bind_nominal_sources(&sources.nominals).unwrap();
        for owner in ["Box", "Static"] {
            let parameter_names: &[&str] = if owner == "Box" {
                &["stored", "count"]
            } else {
                &["count"]
            };
            let record = sources.named(&fixture, owner, parameter_names);
            let payload = record.payload();
            let mut reverse = payload.parameters().parameters().to_vec();
            reverse.reverse();
            let reversed = with_payload(
                record,
                payload.owner(),
                hir::CanonicalSourceParameterShapesV1::try_new(reverse).unwrap(),
                payload.result().clone(),
                payload.effects(),
            );
            let other = sources
                .named(&fixture, "Empty", &[])
                .payload()
                .result()
                .clone();
            let mut corruptions = vec![
                with_payload(
                    record,
                    payload.owner(),
                    payload.parameters().clone(),
                    other,
                    payload.effects(),
                ),
                with_payload(
                    record,
                    payload.owner(),
                    payload.parameters().clone(),
                    SignatureTypeKey::Binder { depth: 0, index: 1 },
                    payload.effects(),
                ),
                with_payload(
                    record,
                    payload.owner(),
                    payload.parameters().clone(),
                    SignatureTypeKey::Binder { depth: 1, index: 0 },
                    payload.effects(),
                ),
            ];
            if owner == "Box" {
                corruptions.push(reversed);
            }
            for record in corruptions {
                let mut forged = sources.clone();
                forged.replace(record);
                assert!(
                    matches!(
                        nominals.bind_constructor_sources(&forged.constructors),
                        Err(Error::Contract { .. })
                    ),
                    "{owner}"
                );
            }
        }
    });
}

#[test]
fn complete_constructor_binding_rejects_non_constructor_effect_contracts() {
    with_source(SOURCE, |output, _| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let nominals = foundation.bind_nominal_sources(&sources.nominals).unwrap();
        for owner in ["Envelope", "Empty"] {
            let params: &[&str] = if owner == "Envelope" {
                &["text"]
            } else {
                &["flag"]
            };
            let record = sources.named(&fixture, owner, params);
            let payload = record.payload();
            for (gc, implementation, operator, infix) in [
                (
                    scoop_identity::GcEffect::Managed,
                    hir::CallableImplementationV1::Intrinsic(hir::IntrinsicFunctionKind::GcCollect),
                    hir::CallableOperatorRoleV1::None,
                    hir::CallableInfixV1::Ordinary,
                ),
                (
                    scoop_identity::GcEffect::Managed,
                    hir::CallableImplementationV1::Scoop,
                    hir::CallableOperatorRoleV1::None,
                    hir::CallableInfixV1::Infix,
                ),
                (
                    scoop_identity::GcEffect::Managed,
                    hir::CallableImplementationV1::Scoop,
                    hir::CallableOperatorRoleV1::Language(hir::CallableOperatorV1::Plus),
                    hir::CallableInfixV1::Ordinary,
                ),
                (
                    scoop_identity::GcEffect::Managed,
                    hir::CallableImplementationV1::Scoop,
                    hir::CallableOperatorRoleV1::PropertyDelegate(
                        hir::PropertyDelegateOperatorV1::GetValue,
                    ),
                    hir::CallableInfixV1::Ordinary,
                ),
            ] {
                let effects = hir::CallableSourceEffectsV1::try_new(
                    scoop_identity::Effect::Ordinary,
                    payload.effects().safety(),
                    gc,
                    implementation,
                    operator,
                    infix,
                )
                .unwrap();
                let mut forged = sources.clone();
                forged.replace(with_payload(
                    record,
                    payload.owner(),
                    payload.parameters().clone(),
                    payload.result().clone(),
                    effects,
                ));
                assert!(matches!(
                    nominals.bind_constructor_sources(&forged.constructors),
                    Err(Error::Contract { .. })
                ));
            }
        }
        let record = sources.named(&fixture, "Envelope", &["text"]);
        let payload = record.payload();
        let effects = hir::CallableSourceEffectsV1::try_new(
            scoop_identity::Effect::Ordinary,
            hir::CallableSafetyV1::Unsafe,
            scoop_identity::GcEffect::NoGc,
            hir::CallableImplementationV1::Scoop,
            hir::CallableOperatorRoleV1::None,
            hir::CallableInfixV1::Ordinary,
        )
        .unwrap();
        let mut forged = sources.clone();
        forged.replace(with_payload(
            record,
            payload.owner(),
            payload.parameters().clone(),
            payload.result().clone(),
            effects,
        ));
        assert!(matches!(
            nominals.bind_constructor_sources(&forged.constructors),
            Err(Error::Contract { .. })
        ));
    });
}
