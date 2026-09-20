use super::*;

#[test]
fn nominal_callable_binders_keep_host_and_method_frames_separate() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        let export = output.output().export.module();
        let (id, _) = export
            .functions
            .iter()
            .find(|(_, function)| function.name == "Envelope.Box.combine")
            .unwrap();
        let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(identity)) =
            &export.function_identities[id]
        else {
            panic!("generic method source");
        };
        let source = table
            .get(CallableTemplateOrigin::GenericFunction(identity.id()))
            .unwrap();
        let payload = source.payload();
        let host = SignatureTypeKey::Binder { depth: 1, index: 0 };
        let own = SignatureTypeKey::Binder { depth: 0, index: 0 };
        assert_eq!(payload.result(), &host);
        let parameters = payload.parameters().parameters();
        assert_eq!(parameters.len(), 2);
        assert_eq!(parameters[0].value_type(), &host);
        assert_eq!(parameters[1].value_type(), &own);
        let [binder] = payload.type_parameters().binders() else {
            panic!("one own binder, no repeated host binder");
        };
        assert_eq!(binder.name().as_str(), "U");
        let hir::TypeParameterBoundsV1::Nominal(bounds) = binder.bounds() else {
            panic!("nominal method bound");
        };
        let (id, _) = export
            .interfaces
            .iter()
            .find(|(_, i)| i.name == "Bound")
            .unwrap();
        assert_eq!(
            bounds.interfaces().values(),
            &[SignatureTypeKey::Nominal(
                export.nominal_identities[id]
                    .source()
                    .unwrap()
                    .concrete_id()
                    .unwrap()
            )]
        );
    });
}

#[test]
fn nominal_variant_callables_match_their_real_fields_and_enum_result() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        let export = output.output().export.module();
        let mapper =
            hir::HirSignatureTypeMapper::new(hir::HirTypeIdentityInputs::from_export(export));
        let mut count = 0;
        for (id, enumeration) in export.enums.iter() {
            let binders = enumeration
                .type_params
                .iter()
                .enumerate()
                .map(|(index, parameter)| hir::HirSignatureBinder {
                    parameter: parameter.id,
                    depth: 0,
                    index: index as u32,
                })
                .collect::<Vec<_>>();
            for (index, variant) in enumeration.variants.iter().enumerate() {
                let reference =
                    hir::EnumVariantRef::checked(&export.enums, id, index as u32).unwrap();
                let declaration = CallableTemplateOrigin::VariantConstructor(
                    export.enum_member_identities[reference].id(),
                );
                let Some(source) = table.get(declaration) else {
                    continue;
                };
                count += 1;
                let payload = source.payload();
                assert!(payload.type_parameters().binders().is_empty());
                assert_eq!(payload.modality(), hir::CallableModalityV1::Final);
                assert!(payload.slot_relations().is_empty());
                assert_eq!(
                    payload.result(),
                    &mapper
                        .map(
                            export.enum_applications[enumeration.self_application].canonical_type,
                            &binders
                        )
                        .unwrap()
                );
                let parameters = payload.parameters().parameters();
                assert_eq!(parameters.len(), variant.fields.len());
                let protocol = export
                    .source_parameter_interfaces
                    .iter()
                    .find(|interface| {
                        interface.owner == hir::ExportParameterOwner::VariantConstructor(reference)
                    })
                    .unwrap();
                for ((parameter, field), source) in parameters
                    .iter()
                    .zip(&variant.fields)
                    .zip(&protocol.parameters)
                {
                    assert_eq!(
                        parameter.value_type(),
                        &mapper.map(field.ty, &binders).unwrap()
                    );
                    assert_eq!(parameter.name().as_str(), source.name);
                }
                assert!(hir::ProtectedCallableInterfaceV1::try_from(source.clone()).is_err());
            }
        }
        assert_eq!(count, 4);
    });
}

#[test]
fn nominal_callable_projection_does_not_silently_drop_missing_required_ids() {
    let foreign = with_source(
        "public class Foreign { public fun missing(): Int = 1 }",
        |output, _| *required(output).first().unwrap(),
    );
    with_source(SOURCE, |output, _| {
        let mut required = required(output);
        required.insert(foreign);
        assert!(matches!(
            Table::from_export_hir(&output.output().export, &required, &mut meter()),
            Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(message))
                if message == "required nominal source callable has no sealed declaration"
        ));
    });
}
