use super::*;
use crate::{DeclaredVisibilityV1, PropertyDeclarationRecordV1};

#[test]
fn each_property_representation_requires_its_actual_accessor_forms() {
    let fixture = Fixture::new();
    use AccessorForm as Form;
    use PropertyRepresentationV1 as Representation;
    for (representation, getter, setter, valid) in [
        (Representation::Const, Form::Constant, None, true),
        (Representation::Const, Form::Body, None, false),
        (Representation::Const, Form::Storage, None, false),
        (Representation::AbstractSlot, Form::AbstractSlot, None, true),
        (Representation::AbstractSlot, Form::Storage, None, false),
        (
            Representation::AbstractSlot,
            Form::AbstractSlot,
            Some(Form::Body),
            false,
        ),
        (Representation::RuntimeAccessor, Form::Storage, None, true),
        (Representation::RuntimeAccessor, Form::Body, None, true),
        (
            Representation::RuntimeAccessor,
            Form::AbstractSlot,
            None,
            false,
        ),
        (
            Representation::RuntimeAccessor,
            Form::Body,
            Some(Form::Constant),
            false,
        ),
        (
            Representation::RuntimeAccessor,
            Form::AbstractSlot,
            Some(Form::AbstractSlot),
            false,
        ),
        (
            Representation::RuntimeAccessor,
            Form::AbstractSlot,
            Some(Form::Body),
            true,
        ),
        (
            Representation::RuntimeAccessor,
            Form::Storage,
            Some(Form::Body),
            true,
        ),
        (
            Representation::RuntimeAccessor,
            Form::Body,
            Some(Form::Storage),
            true,
        ),
    ] {
        let get = AccessorSource::new(fixture.ordinary_getter.id(), getter);
        let accessors = match setter {
            None => Accessors::read_only(get),
            Some(form) => {
                Accessors::try_read_write(get, AccessorSource::new(fixture.setter.id(), form))
                    .unwrap()
            }
        };
        let actual = PropertyDeclarationRecordV1::try_new(
            fixture.ordinary_declaration(),
            fixture.nominal_owner(),
            empty_binders(),
            None,
            SignatureTypeKey::Nominal(fixture.nominal.id()),
            accessors,
            representation,
            DeclaredVisibilityV1::Public,
        );
        if valid {
            let record = actual.unwrap();
            let decoded: DecodedPropertyDeclarationRecordV1 =
                decode_canonical(&encode(&record).unwrap()).unwrap();
            assert_eq!(decoded.resolve(&mut fixture.authority()).unwrap(), record);
        } else {
            assert_eq!(
                actual,
                Err(PropertyInterfaceRecordBuildError::AccessorImplementations(
                    fixture.ordinary_declaration()
                ))
            );
        }
    }
}

#[test]
fn reader_rejects_a_changed_representation_after_decoding_source_forms() {
    let fixture = Fixture::new();
    let mut property = fixture.nominal_record().declaration_data().clone();
    property.representation = PropertyRepresentationV1::RuntimeAccessor;
    let bytes = encode(&property).unwrap();
    let decoded: DecodedPropertyDeclarationRecordV1 = decode_canonical(&bytes).unwrap();
    assert!(
        matches!(decoded.resolve(&mut fixture.authority()), Err(PropertyInterfaceRecordResolutionError::Record(PropertyInterfaceRecordBuildError::AccessorImplementations(id))) if id == fixture.ordinary_declaration())
    );
}
