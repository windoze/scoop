use super::*;
use crate::{CallableDeclarationRecordV1, DeclaredVisibilityV1};

#[test]
fn support_wire_preserves_declared_visibility_without_public_lookup() {
    let fixture = fixture("Hidden");
    let record = source(&fixture.record, DeclaredVisibilityV1::Private);
    let table = CanonicalCallableInterfacesV1::with_support(vec![], vec![record.clone()]).unwrap();
    let decoded = decode_table(&table);
    assert_eq!(
        decoded
            .resolve(&mut identity_authority(&[&fixture]))
            .unwrap(),
        table
    );
    assert!(table.get(record.declaration()).is_none());
    assert_eq!(table.declaration(record.declaration()), Some(&record));
    assert_eq!(
        encode(&table).unwrap(),
        [
            b"\xa2\x01\x80\x02\x81".as_slice(),
            encode(&record).unwrap().as_slice()
        ]
        .concat()
    );
}

#[test]
fn reader_rejects_noncanonical_support_and_duplicates_across_lookup_groups() {
    let first = fixture("First");
    let second = fixture("Second");
    let (low, high) = if first.record.declaration() < second.record.declaration() {
        (&first.record, &second.record)
    } else {
        (&second.record, &first.record)
    };
    let hidden = source(low, DeclaredVisibilityV1::Private);
    let resolve =
        |raw: &Groups| decode_table(raw).resolve(&mut identity_authority(&[&first, &second]));
    assert!(matches!(
        resolve(&Groups(vec![], vec![hidden.clone(), hidden.clone()])),
        Err(CallableInterfaceSetValidationError::DuplicateDeclaration { index: 1, .. })
    ));
    assert!(matches!(
        resolve(&Groups(
            vec![],
            vec![source(high, DeclaredVisibilityV1::Private), hidden]
        )),
        Err(CallableInterfaceSetValidationError::NonCanonicalOrder { index: 1 })
    ));
    assert!(
        matches!(resolve(&Groups(vec![low.clone()], vec![low.declaration_data().clone()])),
        Err(CallableInterfaceSetValidationError::DuplicateSupport(id)) if id == low.declaration())
    );
}

#[test]
fn reader_rejects_legacy_table_and_missing_visibility_or_slot_fields() {
    use scoop_wire::{WireErrorKind, WirePath, WireType};
    let fixture = fixture("Legacy");
    let mut eight_fields = encode(fixture.record.declaration_data()).unwrap();
    let suffix = [
        b"\x09".as_slice(),
        encode(&fixture.record.declared_visibility())
            .unwrap()
            .as_slice(),
        b"\x0a\x80".as_slice(),
    ]
    .concat();
    assert!(eight_fields.ends_with(&suffix));
    eight_fields.truncate(eight_fields.len() - suffix.len());
    eight_fields[0] = 0xa8;
    let mut legacy_record = eight_fields.clone();
    legacy_record[0] = 0xa9;
    legacy_record.push(9);
    legacy_record.extend(encode(&fixture.record.access()).unwrap());
    let legacy_table = [b"\x81".as_slice(), &legacy_record].concat();
    let error = decode_canonical::<DecodedCanonicalCallableInterfacesV1>(
        &legacy_table,
        DecodeLimits::default(),
    )
    .unwrap_err();
    assert_eq!(
        error.kind(),
        &WireErrorKind::WrongType {
            expected: WireType::Map
        }
    );
    assert_eq!(error.path(), &WirePath::root());
    for (actual, bytes) in [(9, legacy_record), (8, eight_fields)] {
        let error = decode_canonical::<crate::DecodedCallableDeclarationRecordV1>(
            &bytes,
            DecodeLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error.kind(),
            &WireErrorKind::InvalidLength {
                expected: 10,
                actual
            }
        );
        assert_eq!(error.path(), &WirePath::root());
    }
}

fn source(
    record: &CallableInterfaceRecordV1,
    visibility: DeclaredVisibilityV1,
) -> CallableDeclarationRecordV1 {
    CallableDeclarationRecordV1::try_new(
        record.declaration(),
        record.owner(),
        record.type_parameters().clone(),
        record.receiver().cloned(),
        record.parameters().clone(),
        record.result().clone(),
        record.effects(),
        record.modality(),
        visibility,
        record.slot_relations().clone(),
    )
    .unwrap()
}

struct Groups(
    Vec<CallableInterfaceRecordV1>,
    Vec<CallableDeclarationRecordV1>,
);
impl WireEncode for Groups {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.array(self.0.len() as u64)?;
        for record in &self.0 {
            record.encode(encoder)?;
        }
        encoder.field(2)?;
        encoder.array(self.1.len() as u64)?;
        for record in &self.1 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}
