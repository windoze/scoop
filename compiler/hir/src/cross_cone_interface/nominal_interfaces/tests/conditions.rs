use super::*;
use crate::{CanonicalBinderUseListV1, NominalInstantiationConditionsV1};
use scoop_identity::SignatureTypeKey;

fn with_conditions(
    record: &NominalInterfaceRecordV1,
    conditions: NominalInstantiationConditionsV1,
) -> Result<NominalInterfaceRecordV1, NominalInterfaceRecordBuildError> {
    let details = record.declaration_details();
    NominalInterfaceRecordV1::try_new(
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
            details.declared_visibility(),
            details.constructors().clone(),
            details.members().clone(),
            details.children().clone(),
            details.dispatch_order().clone(),
            details.dispatch_selections().clone(),
            details.primary_value_constructor(),
            conditions,
            Default::default(),
            None,
            None,
        ),
    )
}

#[test]
fn nominal_conditions_keep_original_binders_through_wire() {
    let fixture = Fixture::new();
    let conditions = NominalInstantiationConditionsV1::new(
        true,
        CanonicalBinderUseListV1::try_new(vec![SignatureTypeKey::Binder { depth: 0, index: 0 }])
            .unwrap(),
    );
    let record = with_conditions(&fixture.record(), conditions.clone()).unwrap();
    let restored = decode_record(&record)
        .resolve(&mut fixture.authority(true))
        .unwrap();
    assert_eq!(
        restored.declaration_details().instantiation_conditions(),
        &conditions
    );
    assert_eq!(encode(&restored).unwrap(), encode(&record).unwrap());
}

#[test]
fn nominal_conditions_reject_foreign_duplicate_and_out_of_range_binders() {
    let fixture = Fixture::new();
    for (arguments, position) in [
        (vec![SignatureTypeKey::Binder { depth: 1, index: 0 }], 0),
        (vec![SignatureTypeKey::Binder { depth: 0, index: 1 }], 0),
        (vec![SignatureTypeKey::Binder { depth: 0, index: 0 }; 2], 1),
        (vec![SignatureTypeKey::Nominal(fixture.superclass.id())], 0),
    ] {
        let conditions = NominalInstantiationConditionsV1::new(
            false,
            CanonicalBinderUseListV1::try_new(arguments).unwrap(),
        );
        assert_eq!(
            with_conditions(&fixture.record(), conditions),
            Err(NominalInterfaceRecordBuildError::PointeeConditionBinder { position })
        );
    }
}

#[test]
fn declaration_reader_rejects_missing_nominal_conditions() {
    let record = Fixture::new().record();
    let details = record.declaration_details();
    let conditions = encode(details.instantiation_conditions()).unwrap();
    let release = encode(details.release_policy()).unwrap();
    let mut legacy = encode(details).unwrap();
    legacy[0] = 0xa8;
    legacy.truncate(legacy.len() - conditions.len() - release.len() - 6);
    let error = decode_canonical::<DecodedNominalDeclarationDetailsV1>(&legacy).unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::InvalidLength {
            expected: 12,
            actual: 8
        }
    ));
}
