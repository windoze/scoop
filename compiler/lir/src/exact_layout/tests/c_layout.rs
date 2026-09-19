use super::*;
use std::num::NonZeroU64;

fn contract(
    owner: PersistentExactTypeId,
    fields: &[CborIdentityRecord<PersistentFieldId, FieldIdentityKey>],
    values: &[&ExactValueLayoutV1],
    offset: u64,
) -> CanonicalCAbiLayoutFingerprintRecord {
    let fields = fields
        .iter()
        .zip(values)
        .enumerate()
        .map(|(index, (field, value))| {
            CanonicalCAbiLayoutField::new(
                field.id(),
                if index == 0 { 0 } else { offset },
                CanonicalCStorageType::Integer {
                    exact_type: value.identity().exact(),
                    signedness: Signedness::Signed,
                    bit_width: if index == 0 {
                        IntegerBitWidth::Bits8
                    } else {
                        IntegerBitWidth::Bits64
                    },
                },
            )
        })
        .collect();
    CanonicalCAbiLayoutFingerprintRecord::new(CanonicalCAbiLayout::new(
        owner,
        16,
        NonZeroU64::new(16).unwrap(),
        CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes16),
        CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes1),
        fields,
    ))
    .unwrap()
}

#[test]
fn c_layout_record_replays_packing_and_requires_its_canonical_foundation_contract() {
    let owner = source("Packed", SourceNominalKind::Struct, 0);
    let fields = [field(&owner, "byte"), field(&owner, "long")];
    let byte = integer("Byte", IntegerKind::SIGNED_8);
    let long = integer("Long", IntegerKind::SIGNED_64);
    let values = [&byte, &long];
    let inputs: Vec<_> = fields
        .iter()
        .zip(values)
        .map(|(field, value)| NominalLayoutFieldInputV1 { field, value })
        .collect();
    let bound = Bound::value(exact(&owner));
    let correct = contract(bound.identity.exact(), &fields, &values, 1);
    assert!(matches!(
        ExactValueLayoutV1::c_struct(
            bound.identity.clone(),
            false,
            &inputs,
            &correct,
            &bound.foundation,
            &mut meter()
        ),
        Err(ExactLayoutReplayError::MissingCLayout)
    ));
    for (offset, success) in [(1, true), (2, false)] {
        let contract = contract(bound.identity.exact(), &fields, &values, offset);
        let mut canonical = bound.foundation.as_canonical().clone();
        canonical.set_c_abi_layouts(vec![contract.clone()]).unwrap();
        let foundation =
            OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap();
        let result = ExactValueLayoutV1::c_struct(
            bound.identity.clone(),
            false,
            &inputs,
            &contract,
            &foundation,
            &mut meter(),
        );
        if success {
            let result = result.unwrap();
            assert_eq!(result.value().storage().alignment().get(), 16);
            let ExactRepresentationKindV1::Struct(representation) = result.representation().kind()
            else {
                panic!("struct");
            };
            assert_eq!(representation.fields()[1].storage().offset().get(), 1);
            assert_eq!(representation.fields()[1].access_alignment().get(), 1);
            assert_eq!(encode(result.representation()).unwrap()[2], 3);
            assert_wire_roundtrip(result);
        } else {
            assert!(matches!(
                result,
                Err(ExactLayoutReplayError::Storage(
                    StorageReplayError::CLayoutMismatch
                ))
            ));
        }
    }
}
