use super::*;

#[test]
fn stored_accessors_cannot_claim_dispatch_slots() {
    let fixture = Fixture::nominal("stored", SourceNominalKind::Class);
    let property = with_source_forms(
        fixture.property(
            None,
            PropertyRepresentationV1::RuntimeAccessor,
            PropertyPublicAccessV1::PublicSlot,
        ),
        AccessorForm::Storage,
        None,
    );
    let mut getter = fixture.accessor(AccessorRole::Getter);
    getter.modality = CallableModalityV1::Open;
    getter.access = PublicLookupAccessV1::PublicSlot;
    assert_eq!(
        validate(vec![property], vec![getter.build()]),
        Err(PropertyAccessorClosureValidationError::SourceForm {
            accessor: fixture.getter,
            implementation: AccessorForm::Storage,
            modality: CallableModalityV1::Open,
        })
    );
}

#[test]
fn abstract_accessor_forms_require_abstract_callable_declarations() {
    let fixture = Fixture::nominal("mixed", SourceNominalKind::Interface);
    let property = with_source_forms(
        fixture.property(
            Some(PropertySetterPublicAccessV1::Public),
            PropertyRepresentationV1::RuntimeAccessor,
            PropertyPublicAccessV1::PublicSlot,
        ),
        AccessorForm::AbstractSlot,
        Some(AccessorForm::Body),
    );
    let mut getter = fixture.accessor(AccessorRole::Getter);
    getter.modality = CallableModalityV1::InterfaceDefault;
    getter.access = PublicLookupAccessV1::PublicSlot;
    let mut setter = fixture.accessor(AccessorRole::Setter);
    setter.modality = CallableModalityV1::InterfaceDefault;
    setter.access = PublicLookupAccessV1::PublicSlot;
    assert_eq!(
        validate(vec![property], vec![getter.build(), setter.build()]),
        Err(PropertyAccessorClosureValidationError::SourceForm {
            accessor: fixture.getter,
            implementation: AccessorForm::AbstractSlot,
            modality: CallableModalityV1::InterfaceDefault,
        })
    );
}

pub(super) fn with_source_forms(
    property: PropertyInterfaceRecordV1,
    getter: AccessorForm,
    setter: Option<AccessorForm>,
) -> PropertyInterfaceRecordV1 {
    let get = AccessorSource::new(property.accessors().getter(), getter);
    let accessors = match (property.accessors().setter(), setter) {
        (Some(id), Some(form)) => {
            Accessors::try_read_write(get, AccessorSource::new(id, form)).unwrap()
        }
        (None, None) => Accessors::read_only(get),
        _ => panic!("test source forms must match the setter presence"),
    };
    PropertyInterfaceRecordV1::try_new(
        property.declaration(),
        property.owner(),
        property.type_parameters().clone(),
        property.receiver().cloned(),
        property.value_type().clone(),
        accessors,
        property.representation(),
        property.access(),
        property
            .capability()
            .setter_access()
            .unwrap_or(PropertySetterPublicAccessV1::Restricted),
    )
    .unwrap()
}
