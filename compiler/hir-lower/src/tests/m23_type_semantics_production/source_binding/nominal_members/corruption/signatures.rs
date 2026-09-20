use super::*;

#[test]
fn nominal_member_binding_rejects_parameter_changes_and_invalid_host_own_binders() {
    with_sources(SOURCE, |_, fixture, sources, core| {
        let record = sources
            .callables
            .records()
            .iter()
            .find(|r| matches!(r.declaration(), CallableTemplateOrigin::GenericFunction(_)))
            .unwrap();
        let p = record.payload();
        let mut reverse = p.parameters().parameters().to_vec();
        reverse.reverse();
        for changed in [
            callable(
                record,
                hir::CanonicalSourceParameterShapesV1::try_new(reverse).unwrap(),
                p.result().clone(),
            ),
            callable(
                record,
                p.parameters().clone(),
                SignatureTypeKey::Binder { depth: 0, index: 1 },
            ),
            callable(
                record,
                p.parameters().clone(),
                SignatureTypeKey::Binder { depth: 1, index: 1 },
            ),
            callable(
                record,
                p.parameters().clone(),
                SignatureTypeKey::Binder { depth: 2, index: 0 },
            ),
        ] {
            let mut forged = sources.clone();
            replace_callable(&mut forged, changed);
            assert!(matches!(
                check(fixture, &forged, core),
                Err(Error::Callable(_))
            ));
        }
    });
}

#[test]
fn nominal_variant_source_binding_checks_field_names_and_owner_result() {
    with_sources(SOURCE, |_, fixture, sources, core| {
        let record = sources
            .callables
            .records()
            .iter()
            .find(|r| {
                matches!(
                    r.declaration(),
                    CallableTemplateOrigin::VariantConstructor(_)
                ) && !r.payload().parameters().is_empty()
            })
            .unwrap();
        let p = record.payload();
        let params = p
            .parameters()
            .parameters()
            .iter()
            .map(|p| {
                hir::SourceParameterShapeV1::new(
                    scoop_identity::CanonicalIdentifier::new("wrong").unwrap(),
                    p.value_type().clone(),
                )
            })
            .collect();
        for changed in [
            callable(
                record,
                hir::CanonicalSourceParameterShapesV1::try_new(params).unwrap(),
                p.result().clone(),
            ),
            callable(
                record,
                p.parameters().clone(),
                SignatureTypeKey::Nominal(core.unit().persistent()),
            ),
        ] {
            let mut forged = sources.clone();
            replace_callable(&mut forged, changed);
            assert!(matches!(
                check(fixture, &forged, core),
                Err(Error::Callable(_))
            ));
        }
    });
}

#[test]
fn nominal_accessor_results_and_const_types_must_match_their_independent_roles() {
    with_sources(SOURCE, |output, fixture, sources, core| {
        let property = property(sources, output, "localValue");
        let p = runtime(&property);
        let hir::ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } = p.mutability() else {
            panic!("setter");
        };
        for id in [p.getter(), *setter] {
            let record = sources
                .callables
                .get(CallableTemplateOrigin::Accessor(id))
                .unwrap();
            let mut forged = sources.clone();
            replace_callable(
                &mut forged,
                callable(
                    record,
                    record.payload().parameters().clone(),
                    SignatureTypeKey::Nominal(core.boolean().persistent()),
                ),
            );
            assert!(matches!(
                check(fixture, &forged, core),
                Err(Error::Callable(_))
            ));
        }
        let record = sources
            .properties
            .records()
            .iter()
            .find(|r| {
                matches!(
                    r.payload(),
                    hir::NominalSupportPropertyPayloadV1::Const { .. }
                )
            })
            .unwrap();
        let hir::NominalSupportPropertyPayloadV1::Const { value } = record.payload() else {
            unreachable!();
        };
        let mut forged = sources.clone();
        let changed = hir::ExportConstValueV1::new(
            value.property(),
            SignatureTypeKey::Nominal(core.boolean().persistent()),
            value.value().clone(),
            value.definition_origin().clone(),
        );
        replace_property(
            &mut forged,
            hir::NominalSupportPropertyInterfaceV1::try_new(
                record.declaration(),
                record.declaration_access().clone(),
                hir::NominalSupportPropertyPayloadV1::Const { value: changed },
            )
            .unwrap(),
        );
        assert!(matches!(
            check(fixture, &forged, core),
            Err(Error::Property(_))
        ));
    });
}

#[test]
fn nominal_member_modality_cannot_make_a_concrete_class_method_abstract() {
    with_sources(SOURCE, |_, fixture, sources, core| {
        let record = sources
            .callables
            .records()
            .iter()
            .find(|r| {
                matches!(r.declaration(), CallableTemplateOrigin::Function(_))
                    && !r.payload().slot_relations().is_empty()
                    && sources
                        .nominals
                        .get(r.payload().owner())
                        .unwrap()
                        .modality()
                        == hir::NominalInheritanceModalityV1::Final
            })
            .unwrap();
        let p = record.payload();
        let changed = hir::NominalSupportCallableInterfaceV1::try_new(
            record.declaration(),
            record.declaration_access().clone(),
            hir::NominalSourceCallablePayloadV1::try_new(
                record.declaration(),
                p.owner(),
                p.type_parameters().clone(),
                p.parameters().clone(),
                p.result().clone(),
                p.effects(),
                hir::CallableModalityV1::Abstract,
                p.slot_relations().clone(),
            )
            .unwrap(),
        )
        .unwrap();
        let mut forged = sources.clone();
        replace_callable(&mut forged, changed);
        assert!(
            matches!(check(fixture, &forged, core), Err(Error::Callable(error)) if matches!(*error, hir::NominalSupportCallableSemanticError::Modality))
        );
    });
}
