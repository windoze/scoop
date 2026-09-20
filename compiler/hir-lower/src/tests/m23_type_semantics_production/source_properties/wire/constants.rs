use super::*;

#[test]
fn inheritance_property_sources_reject_const_payloads_in_builder_and_reader() {
    with_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let constants = hir::CanonicalExportConstValuesV1::from_export_hir(export).unwrap();
        let [value] = constants.records() else {
            panic!("one public object constant");
        };
        let (object, _) = export
            .objects
            .iter()
            .find(|(_, object)| object.name == "Constants")
            .unwrap();
        let owner = hir::SourceNominalId::Concrete(
            export.nominal_identities[object]
                .source()
                .unwrap()
                .concrete_id()
                .unwrap(),
        );
        let record = hir::NominalSupportPropertyInterfaceV1::try_new(
            value.property(),
            hir::DeclarationAccessSourceV1::try_new(
                hir::DeclaredVisibilityV1::Public,
                vec![owner],
                value.definition_origin().clone(),
            )
            .unwrap(),
            Payload::Const {
                value: value.clone(),
            },
        )
        .unwrap();
        assert!(matches!(
            Table::try_new(vec![record.clone()], &mut meter()),
            Err(hir::SourceInventoryError::NonRuntimeProperty(id)) if id == value.property()
        ));
        let decoded: Decoded = decode_canonical(
            &encode(&Records(&[record])).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        let mut identities = source_inventory::identity_closure(output);
        assert!(matches!(
            decoded.resolve(&mut identities, &mut meter()),
            Err(Error::Inventory(hir::SourceInventoryError::NonRuntimeProperty(id)))
                if id == value.property()
        ));
    });
}
