use super::*;
use PropertyAccessorImplementationV1 as Form;

#[test]
fn source_forms_have_fixed_tags_and_body_requirements() {
    for (form, tag, body) in [
        (Form::Storage, 1, false),
        (Form::Constant, 2, false),
        (Form::Body, 3, true),
        (Form::AbstractSlot, 4, true),
        (Form::StorageBody, 5, true),
    ] {
        assert_eq!(encode(&form).unwrap(), [tag]);
        assert_eq!(decode_canonical::<Form>(&[tag]).unwrap(), form);
        assert_eq!(form.requires_body(), body);
    }
    for tag in [0, 6, 7] {
        let error = decode_canonical::<Form>(&[tag]).unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: tag.into() });
    }
}

#[test]
fn source_accessors_pair_each_identity_with_its_implementation() {
    let (getter, setter) = accessors();
    for (getter_form, setter_form) in [
        (Form::Storage, None),
        (Form::Constant, None),
        (Form::Body, None),
        (Form::AbstractSlot, None),
        (Form::Storage, Some(Form::Body)),
        (Form::Body, Some(Form::Storage)),
        (Form::AbstractSlot, Some(Form::Body)),
        (Form::AbstractSlot, Some(Form::AbstractSlot)),
    ] {
        let get = PropertyAccessorSourceV1::new(getter, getter_form);
        let set = setter_form.map(|form| PropertyAccessorSourceV1::new(setter, form));
        let expected = match set {
            Some(set) => PropertyAccessorsV1::try_read_write(get, set).unwrap(),
            None => PropertyAccessorsV1::read_only(get),
        };
        let bytes = accessor_wire(
            getter,
            getter_form,
            set.map(|s| (s.accessor(), s.implementation())),
        );
        assert_eq!(encode(&expected).unwrap(), bytes);
        let decoded = decode(&bytes);
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(
            decoded
                .resolve(&mut AccessorResolver { getter, setter })
                .unwrap(),
            expected
        );
        assert_eq!(expected.getter_source(), get);
        assert_eq!(expected.setter_source(), set);
    }
}

#[test]
fn source_accessors_reject_retired_tags_and_incomplete_pairs() {
    for tag in [0, 1, 2, 3, 6] {
        let error = decode_canonical::<DecodedPropertyAccessorsV1>(&[0xa1, 0, tag]).unwrap_err();
        assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: tag.into() });
    }
    let (getter, setter) = accessors();
    let original = accessor_wire(getter, Form::Storage, Some((setter, Form::Body)));
    for (index, length, expected) in [(0, 2, 3), (4, 1, 2), (4, 3, 2)] {
        let mut bytes = original.clone();
        bytes[index] = 0xa0 + length;
        let error = decode_canonical::<DecodedPropertyAccessorsV1>(&bytes).unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected,
                actual: length.into()
            }
        );
    }
    let mut incomplete_setter = original;
    incomplete_setter.pop();
    assert!(decode_canonical::<DecodedPropertyAccessorsV1>(&incomplete_setter).is_err());
}

#[test]
fn duplicate_identity_is_rejected_even_when_source_forms_differ() {
    let (getter, setter) = accessors();
    let duplicate = PropertyCapabilityBuildError::DuplicateAccessor(getter);
    assert_eq!(
        PropertyAccessorsV1::try_read_write(
            PropertyAccessorSourceV1::new(getter, Form::Storage),
            PropertyAccessorSourceV1::new(getter, Form::Body),
        ),
        Err(duplicate)
    );
    let decoded = decode(&accessor_wire(
        getter,
        Form::Storage,
        Some((getter, Form::Body)),
    ));
    assert_eq!(
        decoded.resolve(&mut AccessorResolver { getter, setter }),
        Err(PropertyCapabilityResolutionError::Capability(duplicate))
    );
}

#[test]
fn each_source_accessor_must_resolve_through_the_identity_graph() {
    let (getter, setter) = accessors();
    let bytes = accessor_wire(getter, Form::Storage, Some((setter, Form::Body)));
    assert_eq!(
        decode(&bytes).resolve(&mut AccessorResolver {
            getter: setter,
            setter
        }),
        Err(PropertyCapabilityResolutionError::Getter(UnknownAccessor))
    );
    assert_eq!(
        decode(&bytes).resolve(&mut AccessorResolver {
            getter,
            setter: getter
        }),
        Err(PropertyCapabilityResolutionError::Setter(UnknownAccessor))
    );
}

fn decode(bytes: &[u8]) -> DecodedPropertyAccessorsV1 {
    decode_canonical(bytes).unwrap()
}

fn accessor_wire(
    getter: PersistentPropertyAccessorId,
    form: Form,
    setter: Option<(PersistentPropertyAccessorId, Form)>,
) -> Vec<u8> {
    let mut bytes = vec![
        if setter.is_some() { 0xa3 } else { 0xa2 },
        0,
        if setter.is_some() { 5 } else { 4 },
        1,
    ];
    let mut pair = |id: PersistentPropertyAccessorId, form: Form| {
        bytes.extend_from_slice(&[0xa2, 1, 0x58, 0x20]);
        bytes.extend_from_slice(id.as_array());
        bytes.push(2);
        bytes.push(match form {
            Form::Storage => 1,
            Form::Constant => 2,
            Form::Body => 3,
            Form::AbstractSlot => 4,
            Form::StorageBody => 5,
        });
    };
    pair(getter, form);
    if let Some((setter, form)) = setter {
        bytes.push(2);
        bytes.extend_from_slice(&[0xa2, 1, 0x58, 0x20]);
        bytes.extend_from_slice(setter.as_array());
        bytes.extend_from_slice(&[
            2,
            match form {
                Form::Storage => 1,
                Form::Constant => 2,
                Form::Body => 3,
                Form::AbstractSlot => 4,
                Form::StorageBody => 5,
            },
        ]);
    }
    bytes
}
