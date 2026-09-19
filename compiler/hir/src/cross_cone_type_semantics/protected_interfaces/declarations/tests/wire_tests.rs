use super::*;
use scoop_wire::{Encoder, WireEncode};

struct Records<'a>(&'a [&'a ProtectedDeclarationInterfaceV1]);
impl WireEncode for Records<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[test]
fn declaration_reader_keeps_flat_four_field_sums_and_refuses_unknown_shapes() {
    let (mut fixture, table) = complete();
    for record in table.records() {
        let bytes = encode(record).unwrap();
        assert_eq!(bytes[0], 0xa4);
        let decoded: DecodedProtectedDeclarationInterfaceV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(
            decoded.resolve(&mut fixture, &mut meter()).unwrap(),
            *record
        );
        let mut wrong_count = bytes.clone();
        wrong_count[0] = 0xa3;
        assert!(
            decode_canonical::<DecodedProtectedDeclarationInterfaceV1>(
                &wrong_count,
                DecodeLimits::default()
            )
            .is_err()
        );
        let mut unknown_tag = bytes;
        unknown_tag[2] = 5;
        assert!(
            decode_canonical::<DecodedProtectedDeclarationInterfaceV1>(
                &unknown_tag,
                DecodeLimits::default()
            )
            .is_err()
        );
    }
}

#[test]
fn declaration_reader_rejects_duplicate_and_noncanonical_keys_and_exhausted_budget() {
    let (mut fixture, table) = complete();
    let first = &table.records()[0];
    let last = table.records().last().unwrap();
    for (records, expected) in [
        ([first, first], ProtectedDeclarationTableError::Duplicate),
        (
            [last, first],
            ProtectedDeclarationTableError::NonCanonicalOrder,
        ),
    ] {
        let decoded: DecodedCanonicalProtectedDeclarationInterfacesV1 = decode_canonical(
            &encode(&Records(&records)).unwrap(),
            DecodeLimits::default(),
        )
        .unwrap();
        assert!(
            matches!(decoded.resolve(&mut fixture, &mut meter()), Err(ProtectedDeclarationResolutionError::Table(actual)) if actual == expected)
        );
    }
    let decoded: DecodedCanonicalProtectedDeclarationInterfacesV1 =
        decode_canonical(&encode(&table).unwrap(), DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(
            &mut fixture,
            &mut BudgetMeter::new(DecodeLimits {
                decoded_nodes: 0,
                ..DecodeLimits::default()
            })
        ),
        Err(ProtectedDeclarationResolutionError::Callable(
            ProtectedCallableInterfaceResolutionError::Resource(_)
        ))
    ));
    assert!(matches!(
        CanonicalProtectedDeclarationInterfacesV1::try_new(vec![first.clone(), first.clone()]),
        Err(ProtectedDeclarationTableError::Duplicate)
    ));
}

#[test]
fn callable_reference_cannot_reinterpret_a_constructor_as_an_ordinary_member() {
    let (mut fixture, table) = complete();
    let constructor = table
        .records()
        .iter()
        .find_map(|record| match record {
            ProtectedDeclarationInterfaceV1::Constructor(value) => Some(value.declaration()),
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        ProtectedCallableDeclarationRefV1::try_new(CallableTemplateOrigin::Constructor(
            constructor
        )),
        Err(ProtectedDeclarationTableError::CallableKind)
    ));
    let owner_bytes = encode(&CallableTemplateOrigin::Constructor(constructor)).unwrap();
    let mut bytes = vec![0xa2, 0, 1, 1];
    bytes.extend(owner_bytes);
    let decoded: DecodedProtectedDeclarationRefV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture),
        Err(ProtectedDeclarationResolutionError::Table(
            ProtectedDeclarationTableError::CallableKind
        ))
    ));
}
