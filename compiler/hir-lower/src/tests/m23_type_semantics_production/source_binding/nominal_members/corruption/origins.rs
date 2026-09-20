use super::*;

#[test]
fn nominal_member_origins_must_match_the_exact_artifact_subject() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        for record in sources.callables.records() {
            let expected = match record.declaration() {
                CallableTemplateOrigin::Function(id) => DefinitionOriginSubject::Function(id),
                CallableTemplateOrigin::GenericFunction(id) => {
                    DefinitionOriginSubject::GenericFunction(id)
                }
                CallableTemplateOrigin::Accessor(id) => {
                    DefinitionOriginSubject::PropertyAccessor(id)
                }
                CallableTemplateOrigin::VariantConstructor(id) => {
                    DefinitionOriginSubject::EnumVariant(id)
                }
                _ => panic!("member source role"),
            };
            let other = sources
                .callables
                .records()
                .iter()
                .find(|r| {
                    r.declaration_access().definition_origin()
                        != record.declaration_access().definition_origin()
                })
                .unwrap();
            let old = record.declaration_access();
            let access = hir::DeclarationAccessSourceV1::try_new(
                old.declared_visibility(),
                old.lexical_owners().to_vec(),
                other.declaration_access().definition_origin().clone(),
            )
            .unwrap();
            let mut forged = sources.clone();
            replace_callable(
                &mut forged,
                hir::NominalSupportCallableInterfaceV1::try_new(
                    record.declaration(),
                    access,
                    record.payload().clone(),
                )
                .unwrap(),
            );
            assert!(
                matches!(check(fixture, &forged, core), Err(Error::Origin(actual)) if actual == expected)
            );
        }
        let record = property(sources, output, "localValue");
        let other = property(sources, output, "data");
        let source = record.declaration_access();
        let changed = hir::DeclarationAccessSourceV1::try_new(
            source.declared_visibility(),
            source.lexical_owners().to_vec(),
            other.declaration_access().definition_origin().clone(),
        )
        .unwrap();
        let mut forged = sources.clone();
        replace_property(
            &mut forged,
            hir::NominalSupportPropertyInterfaceV1::try_new(
                record.declaration(),
                changed,
                record.payload().clone(),
            )
            .unwrap(),
        );
        assert!(
            matches!(check(fixture, &forged, core), Err(Error::Origin(DefinitionOriginSubject::Property(id))) if id == record.declaration())
        );
    });
}

#[test]
fn nominal_member_binding_rejects_wrong_owner_and_shortened_lexical_chain() {
    with_sources(SOURCE, |_, fixture, sources, core| {
        let record = sources
            .callables
            .records()
            .iter()
            .find(|r| matches!(r.declaration(), CallableTemplateOrigin::GenericFunction(_)))
            .unwrap();
        let old = record.declaration_access();
        let mut owners = old.lexical_owners().to_vec();
        owners.remove(0);
        let changed = hir::DeclarationAccessSourceV1::try_new(
            old.declared_visibility(),
            owners,
            old.definition_origin().clone(),
        )
        .unwrap();
        let mut forged = sources.clone();
        replace_callable(
            &mut forged,
            hir::NominalSupportCallableInterfaceV1::try_new(
                record.declaration(),
                changed,
                record.payload().clone(),
            )
            .unwrap(),
        );
        assert!(matches!(
            check(fixture, &forged, core),
            Err(Error::Callable(_))
        ));
        let other = sources
            .callables
            .records()
            .iter()
            .find(|r| r.payload().owner() != record.payload().owner())
            .unwrap();
        replace_callable(
            &mut forged,
            hir::NominalSupportCallableInterfaceV1::try_new(
                record.declaration(),
                other.declaration_access().clone(),
                hir::NominalSourceCallablePayloadV1::try_new(
                    record.declaration(),
                    other.payload().owner(),
                    record.payload().type_parameters().clone(),
                    record.payload().parameters().clone(),
                    record.payload().result().clone(),
                    record.payload().effects(),
                    record.payload().modality(),
                    record.payload().slot_relations().clone(),
                )
                .unwrap(),
            )
            .unwrap(),
        );
        assert!(matches!(
            check(fixture, &forged, core),
            Err(Error::Inventory("callable owners"))
        ));
    });
}
