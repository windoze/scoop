use super::*;
use crate::{DeclaredVisibilityV1, NominalDeclarationDetailsV1};

#[test]
fn shared_table_rejects_legacy_arrays_and_records_without_declaration_details() {
    let record = fixture("Legacy").record;
    let legacy_table = [vec![0x81], encode(&record).unwrap()].concat();
    assert!(matches!(
        decode_canonical::<DecodedCanonicalNominalInterfacesV1>(&legacy_table)
            .unwrap_err()
            .kind(),
        scoop_wire::WireErrorKind::WrongType {
            expected: scoop_wire::WireType::Map
        }
    ));
    let mut legacy = encode(&record).unwrap();
    legacy[0] = 0xa8;
    legacy.truncate(legacy.len() - encode(record.declaration_details()).unwrap().len() - 1);
    assert!(matches!(
        decode_canonical::<super::super::super::DecodedNominalInterfaceRecordV1>(&legacy)
            .unwrap_err()
            .kind(),
        scoop_wire::WireErrorKind::InvalidLength {
            expected: 9,
            actual: 8
        }
    ));
}

#[test]
fn shared_table_keeps_support_out_of_public_lookup_and_rejects_cross_partition_duplicates() {
    let first = fixture("Public");
    let second = fixture("Support");
    let table = CanonicalNominalInterfacesV1::with_support(
        vec![first.record.clone()],
        vec![second.record.clone()],
    )
    .unwrap();
    assert_eq!(table.declaration_count(), 2);
    assert!(table.get(second.record.declaration()).is_none());
    assert_eq!(
        table.declaration(second.record.declaration()),
        Some(&second.record)
    );
    assert_eq!(
        decode_table(&table)
            .resolve(&mut authority(&[first.identity.clone(), second.identity]))
            .unwrap(),
        table
    );
    assert!(matches!(
        CanonicalNominalInterfacesV1::with_support(
            vec![first.record.clone()],
            vec![first.record.clone()]
        ),
        Err(NominalInterfaceSetBuildError::DuplicateDeclaration(_))
    ));
    let invalid = PartitionBytes(first.record);
    assert!(matches!(
        decode_table(&invalid).resolve(&mut authority(&[first.identity])),
        Err(NominalInterfaceSetValidationError::Partition(
            NominalInterfaceSetBuildError::DuplicateDeclaration(_)
        ))
    ));
}

#[test]
fn restricted_nominal_cannot_be_promoted_to_public_partition() {
    let record = fixture("Restricted").record;
    let details = record.declaration_details();
    let restricted = NominalInterfaceRecordV1::try_new(
        record.declaration(),
        record.kind(),
        record.type_parameters().clone(),
        record.exact_supertypes().clone(),
        record.constructors().clone(),
        record.members().clone(),
        record.nested_bindings().clone(),
        record.source_shape().clone(),
        NominalDeclarationDetailsV1::new(
            details.modality(),
            DeclaredVisibilityV1::Private,
            details.constructors().clone(),
            details.members().clone(),
            details.children().clone(),
            details.dispatch_order().clone(),
            details.dispatch_selections().clone(),
            details.primary_value_constructor(),
            details.instantiation_conditions().clone(),
            Default::default(),
            None,
            None,
        ),
    )
    .unwrap();
    assert!(matches!(
        CanonicalNominalInterfacesV1::try_new(vec![restricted.clone()]),
        Err(NominalInterfaceSetBuildError::NonPublicDeclaration(_))
    ));
    assert!(CanonicalNominalInterfacesV1::with_support(vec![], vec![restricted]).is_ok());
}

struct PartitionBytes(NominalInterfaceRecordV1);
impl WireEncode for PartitionBytes {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        for field in [1, 2] {
            encoder.field(field)?;
            encoder.array(1)?;
            self.0.encode(encoder)?;
        }
        Ok(())
    }
}
