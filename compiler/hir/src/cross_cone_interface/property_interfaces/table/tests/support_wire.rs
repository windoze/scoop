use super::*;
use crate::{DeclaredVisibilityV1, PropertyDeclarationRecordV1};

#[test]
fn support_properties_round_trip_without_public_lookup() {
    let f = fixture("Hidden");
    let source = hidden(&f.record);
    let table = CanonicalPropertyInterfacesV1::with_support(vec![], vec![source.clone()]).unwrap();
    assert!(table.get(source.declaration()).is_none());
    assert_eq!(table.declaration(source.declaration()), Some(&source));
    assert_eq!(
        decode_table(&table)
            .resolve(&mut identity_authority(&[&f]))
            .unwrap(),
        table
    );
    assert_eq!(
        encode(&table).unwrap(),
        [
            b"\xa2\x01\x80\x02\x81".as_slice(),
            &encode(&source).unwrap()
        ]
        .concat()
    );
}

#[test]
fn support_property_wire_rejects_reordering_and_duplicates_in_or_between_groups() {
    let first = fixture("First");
    let second = fixture("Second");
    let (low, high) = if first.record.declaration() < second.record.declaration() {
        (&first.record, &second.record)
    } else {
        (&second.record, &first.record)
    };
    let resolve =
        |raw: &Groups| decode_table(raw).resolve(&mut identity_authority(&[&first, &second]));
    assert!(matches!(
        resolve(&Groups(vec![], vec![hidden(high), hidden(low)])),
        Err(PropertyInterfaceSetValidationError::NonCanonicalOrder { index: 1 })
    ));
    assert!(matches!(
        resolve(&Groups(vec![], vec![hidden(low), hidden(low)])),
        Err(PropertyInterfaceSetValidationError::DuplicateDeclaration { index: 1, .. })
    ));
    assert!(
        matches!(resolve(&Groups(vec![low.clone()], vec![low.declaration_data().clone()])), Err(PropertyInterfaceSetValidationError::DuplicateSupport(id)) if id == low.declaration())
    );
}

#[test]
fn reader_rejects_old_property_records_tables_and_lookup_bearing_accessor_sets() {
    use scoop_wire::{WireErrorKind, WirePath, WireType};
    let f = fixture("Legacy");
    let mut seven_fields = encode(f.record.declaration_data()).unwrap();
    let suffix = [
        b"\x08".as_slice(),
        &encode(&f.record.declared_visibility()).unwrap(),
    ]
    .concat();
    assert!(seven_fields.ends_with(&suffix));
    seven_fields.truncate(seven_fields.len() - suffix.len());
    seven_fields[0] = 0xa7;
    let mut old_record = seven_fields.clone();
    old_record[0] = 0xa8;
    old_record.push(8);
    old_record.extend(encode(&f.record.access()).unwrap());
    let error = decode_canonical::<crate::DecodedPropertyInterfaceRecordV1>(
        &old_record,
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 3,
            actual: 8
        }
    );
    assert_eq!(error.path(), &WirePath::root());
    let error = decode_canonical::<crate::DecodedPropertyDeclarationRecordV1>(
        &seven_fields,
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::InvalidLength {
            expected: 8,
            actual: 7
        }
    );
    let old_table = [b"\x81".as_slice(), &old_record].concat();
    let error = decode_canonical::<DecodedCanonicalPropertyInterfacesV1>(
        &old_table,
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::WrongType {
            expected: WireType::Map
        }
    );
    let other = fixture("Other");
    let old_set = PropertyCapabilityV1::try_read_write(
        f.record.capability().getter(),
        other.record.capability().getter(),
        crate::PropertySetterPublicAccessV1::Restricted,
    )
    .unwrap();
    let error = decode_canonical::<crate::DecodedPropertyAccessorsV1>(
        &encode(&old_set).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind(), &WireErrorKind::UnknownTag { tag: 2 });
}

fn hidden(record: &PropertyInterfaceRecordV1) -> PropertyDeclarationRecordV1 {
    PropertyDeclarationRecordV1::try_new(
        record.declaration(),
        record.owner(),
        record.type_parameters().clone(),
        record.receiver().cloned(),
        record.value_type().clone(),
        record.accessors(),
        record.representation(),
        DeclaredVisibilityV1::Private,
    )
    .unwrap()
}

struct Groups(
    Vec<PropertyInterfaceRecordV1>,
    Vec<PropertyDeclarationRecordV1>,
);
impl WireEncode for Groups {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(2)?;
        e.field(1)?;
        e.array(self.0.len() as u64)?;
        for r in &self.0 {
            r.encode(e)?;
        }
        e.field(2)?;
        e.array(self.1.len() as u64)?;
        for r in &self.1 {
            r.encode(e)?;
        }
        Ok(())
    }
}
