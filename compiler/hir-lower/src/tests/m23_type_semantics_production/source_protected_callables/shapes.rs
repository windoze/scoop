use super::*;
use scoop_identity::{CoreBuiltinNominal, SignatureTypeKey};

#[test]
fn protected_stored_and_body_accessors_keep_logical_types_and_setter_names() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        let export = output.output().export.module();
        let mapper =
            hir::HirSignatureTypeMapper::new(hir::HirTypeIdentityInputs::from_export(export));
        let mut getters = 0;
        let mut setters = 0;
        for (_, property) in export.properties.iter() {
            let getter = export.property_accessor_identities[property.capability.getter()].id();
            if let Some(record) = table.get(CallableTemplateOrigin::Accessor(getter)) {
                assert!(record.payload().parameters().is_empty());
                assert_eq!(
                    record.payload().result(),
                    &mapper.map(property.ty, &[]).unwrap()
                );
                assert_eq!(
                    record.payload().source_interface(),
                    hir::ProtectedSourceInterfaceUseV1::AccessorNoSourceInterface
                );
                getters += 1;
            }
            if let Some(setter) = property.capability.setter() {
                let declaration = export.property_accessor_identities[setter].id();
                if let Some(record) = table.get(CallableTemplateOrigin::Accessor(declaration)) {
                    let parameters = record.payload().parameters().parameters();
                    assert_eq!(parameters.len(), 1);
                    assert_eq!(
                        parameters[0].name().as_str(),
                        export.property_setters[setter].parameter_name
                    );
                    assert_eq!(
                        parameters[0].value_type(),
                        &mapper.map(property.ty, &[]).unwrap()
                    );
                    assert_eq!(
                        record.payload().result(),
                        &SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
                    );
                    setters += 1;
                }
            }
        }
        assert_eq!((getters, setters), (4, 2));
    });
}

#[test]
fn protected_generic_methods_retain_source_bounds_and_callable_modifiers() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        let export = output.output().export.module();
        let bounded = export
            .functions
            .iter()
            .find(|(_, function)| function.name == "Source.bounded")
            .unwrap()
            .0;
        let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(identity)) =
            &export.function_identities[bounded]
        else {
            panic!("generic source declaration");
        };
        let record = table
            .get(CallableTemplateOrigin::GenericFunction(identity.id()))
            .unwrap();
        let binders = record.payload().type_parameters().binders();
        assert_eq!(binders.len(), 1);
        assert_eq!(binders[0].name().as_str(), "T");
        let hir::TypeParameterBoundsV1::Nominal(bounds) = binders[0].bounds() else {
            panic!("nominal source bound");
        };
        let bound = export
            .interfaces
            .iter()
            .find(|(_, interface)| interface.name == "Bound")
            .unwrap()
            .0;
        let owner = export.nominal_identities[bound]
            .source()
            .unwrap()
            .concrete_id()
            .unwrap();
        assert!(bounds.class().is_none());
        assert_eq!(
            bounds.interfaces().values(),
            &[SignatureTypeKey::Nominal(owner)]
        );
        assert_eq!(
            record.payload().result(),
            &SignatureTypeKey::Binder { depth: 0, index: 0 }
        );
        let effects = |name| {
            let id = export
                .functions
                .iter()
                .find(|(_, function)| function.name == name)
                .unwrap()
                .0;
            let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(identity)) =
                &export.function_identities[id]
            else {
                panic!("plain source declaration");
            };
            table
                .get(CallableTemplateOrigin::Function(identity.id()))
                .unwrap()
                .payload()
                .effects()
        };
        assert_eq!(
            effects("Source.joined").infix(),
            hir::CallableInfixV1::Infix
        );
        assert_eq!(
            effects("Source.contains").operator_role(),
            hir::CallableOperatorRoleV1::Language(hir::CallableOperatorV1::Contains)
        );
    });
}

#[test]
fn protected_result_type_tree_is_budgeted_before_signature_projection() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-dispatch/protected-signatures.scoop"
    ));
    with_source(source, |output, _| {
        let limits = DecodeLimits {
            semantic_recursion: 3,
            ..DecodeLimits::default()
        };
        hir::CanonicalSourceInheritanceInventoriesV1::from_ordinary_hir(
            output,
            &mut BudgetMeter::new(limits),
        )
        .unwrap();
        let error = Table::from_ordinary_hir(output, &mut BudgetMeter::new(limits)).unwrap_err();
        let hir::CrossConeTypeSemanticsProductionError::SourceInventory(
            hir::SourceInventoryError::Resource(error),
        ) = error
        else {
            panic!("shared resource budget");
        };
        assert!(matches!(
            error.kind(),
            scoop_wire::WireErrorKind::LimitExceeded {
                resource: scoop_wire::ResourceKind::SemanticRecursion,
                limit: 3,
                observed: 4
            }
        ));
        let table = table(output);
        assert_eq!(table.records().len(), 1);
        assert!(matches!(
            table.records()[0].payload().result(),
            SignatureTypeKey::Tuple(_)
        ));
    });
}
