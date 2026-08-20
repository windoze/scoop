use super::*;

#[test]
fn instance_reader_checks_prefix_payload_and_array_element_relationships() {
    let class = fixtures::class();
    roundtrip(&class);
    reject(&class, |raw| {
        let RawInstance::Class {
            base: RawBase::Prefix { size, .. },
            ..
        } = instance(raw)
        else {
            panic!("prefix")
        };
        *size += 8;
    });
    reject(&class, |raw| {
        let RawInstance::Class { base, .. } = instance(raw) else {
            panic!("class")
        };
        *base = RawBase::NoBase;
    });
    let boxed = fixtures::boxed();
    roundtrip(&boxed);
    reject(&boxed, |raw| {
        let RawInstance::Box { exact, .. } = instance(raw) else {
            panic!("box")
        };
        *exact = decode(&ExactLayoutExportV1::from(unit())).semantic.exact;
    });
    for zst in [false, true] {
        let array = fixtures::array(zst);
        roundtrip(&array);
        reject(&array, |raw| {
            let RawInstance::InlineArray { storage, .. } = instance(raw) else {
                panic!("array")
            };
            let RawBody::Instance {
                representation: RawInstance::InlineArray { storage: wrong, .. },
                ..
            } = decode(&fixtures::array(!zst)).semantic.body
            else {
                panic!("array")
            };
            *storage = wrong;
        });
    }
}

#[test]
fn instance_reader_compares_the_complete_shape_and_branch() {
    let bytes = fixtures::bytes();
    roundtrip(&bytes);
    let array = fixtures::array(false);
    reject(&array, |raw| {
        let RawBody::Instance { shape, .. } = &mut raw.semantic.body else {
            panic!("shape")
        };
        let RawBody::Instance { shape: wrong, .. } = decode(&bytes).semantic.body else {
            panic!("bytes")
        };
        *shape = wrong;
    });
    reject(&bytes, |raw| {
        *instance(raw) = RawInstance::AbstractReference
    });
    let bound = Bound::instance(exact(&source("Interface", SourceNominalKind::Interface, 0)));
    let abstract_ref =
        ExactInstanceLayoutV1::abstract_reference(bound.identity, &bound.foundation).unwrap();
    roundtrip(&abstract_ref.into());
}
