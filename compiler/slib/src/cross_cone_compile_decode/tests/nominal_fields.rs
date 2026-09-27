use super::*;

#[test]
fn ordinary_reader_rejects_missing_source_fields_after_canonical_bytes_are_restored() {
    let cone = cone();
    let (foundation, interface, owner, _, _, _, _, _) =
        nominal_surface(cone.identity(), true, true, true);
    let original =
        cross_cone_artifact_for_with_hir_foundation(cone.clone(), vec![], &foundation, interface);
    let valid = front(&original).validate_nominal_surface(vec![]).unwrap();
    let interface = valid.hir_interface();
    let nominal = interface
        .nominal_interfaces()
        .get(SourceNominalId::Concrete(owner))
        .unwrap();
    let missing = nominal.source_shape().declared_fields()[0].field();
    let empty = NominalInterfaceRecordV1::try_new(
        nominal.declaration(),
        nominal.kind(),
        nominal.type_parameters().clone(),
        nominal.exact_supertypes().clone(),
        nominal.constructors().clone(),
        nominal.members().clone(),
        nominal.nested_bindings().clone(),
        NominalSourceShapeV1::Struct(
            StructSourceShapeV1::try_new(
                vec![],
                scoop_hir::NominalCLayoutPolicyV1::Ordinary,
                false,
            )
            .unwrap(),
        ),
        nominal.declaration_details().clone(),
    )
    .unwrap();
    let mut corrupt = CrossConeHirInterfaceSectionV1::new(
        interface.public_bindings().clone(),
        CanonicalNominalInterfacesV1::try_new(vec![empty]).unwrap(),
        interface.callable_interfaces().clone(),
        interface.property_interfaces().clone(),
        interface.type_aliases().clone(),
        interface.source_interfaces().clone(),
        interface.default_templates().clone(),
        interface.constants().clone(),
        interface.definition_sources().clone(),
        interface.external_references().clone(),
        interface.generic_callable_bodies().clone(),
        interface.generic_initializations().clone(),
    );
    let bytes = encode(&corrupt.index_for_wire().unwrap()).unwrap();
    let artifact = cross_cone_artifact_for_with_hir_foundation(cone, vec![], &foundation, bytes);
    let Err(CrossConeHirNominalSurfaceError::Fields(error)) =
        front(&artifact).validate_nominal_surface(vec![])
    else {
        panic!("a restored nominal cannot omit one of its canonical declaration fields");
    };
    assert!(
        matches!(error, scoop_hir::NominalSourceFieldInventoryError::Missing { owner: actual, field } if actual == SourceNominalId::Concrete(owner) && field == missing)
    );
    assert!(error.to_string().contains("omits declared field"));
}

pub(super) fn front(bytes: &[u8]) -> crate::DefinitionSourceValidatedCrossConeHirFrontSections<'_> {
    declaration_front(bytes)
        .validate_internal_hir_closures()
        .unwrap()
        .validate_definition_sources(&[])
        .unwrap()
}
