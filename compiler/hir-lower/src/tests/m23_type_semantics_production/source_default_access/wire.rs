use super::*;
use scoop_identity::PersistentGenericTypeId;
use scoop_wire::WireEncode;

fn product(persistent: &[u8], generics: &[u8]) -> Vec<u8> {
    [
        vec![0xa2, 1],
        persistent.to_vec(),
        vec![2],
        generics.to_vec(),
    ]
    .concat()
}
fn generics(export: &hir::ExportHir) -> Vec<PersistentGenericTypeId> {
    let mut ids = ["Generic", "Other"]
        .map(|name| {
            match export.nominal_identities[class(export, name)]
                .source()
                .unwrap()
            {
                hir::HirSourceNominalIdentity::Generic(record) => record.id(),
                _ => panic!("generic source class required"),
            }
        })
        .to_vec();
    ids.sort();
    ids
}
fn array<T: WireEncode>(values: &[T]) -> Vec<u8> {
    struct Rows<'a, T>(&'a [T]);
    impl<T: WireEncode> WireEncode for Rows<'_, T> {
        fn encode(&self, e: &mut scoop_wire::Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
            e.array(self.0.len() as u64)?;
            for value in self.0 {
                value.encode(e)?;
            }
            Ok(())
        }
    }
    encode(&Rows(values)).unwrap()
}

#[test]
fn source_domain_fixed_vectors_preserve_empty_universal_and_required_fields() {
    assert_eq!(
        encode(&Domain::empty()).unwrap(),
        [0xa2, 1, 0xa1, 0, 1, 2, 0x80]
    );
    let universal = vec![0xa2, 1, 0xa2, 0, 2, 1, 0x80, 2, 0x80];
    assert_eq!(encode(&Domain::universal()).unwrap(), universal);
    for header in [0xa0, 0xa1, 0xa3] {
        let mut malformed = universal.clone();
        malformed[0] = header;
        assert!(decode_canonical::<DecodedDomain>(&malformed, DecodeLimits::default()).is_err());
    }
    let mut malformed = universal;
    malformed[7] = 3;
    assert!(decode_canonical::<DecodedDomain>(&malformed, DecodeLimits::default()).is_err());
}

#[test]
fn source_domain_reader_rejects_duplicate_reordered_unknown_and_empty_generic_constraints() {
    with_hir_source(SOURCE, |output, _| {
        let ids = generics(output.output().export.module());
        for bad in [vec![ids[1], ids[0]], vec![ids[0], ids[0]]] {
            let bytes = product(
                &encode(&hir::PersistentAccessDomainV1::universal()).unwrap(),
                &array(&bad),
            );
            let decoded: DecodedDomain = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            assert!(matches!(
                decoded.resolve(&mut identity_closure(output), &mut meter()),
                Err(hir::DefaultSourceAccessResolutionError::GenericSubclasses(
                    _
                ))
            ));
        }
        let bytes = product(
            &encode(&hir::PersistentAccessDomainV1::empty()).unwrap(),
            &array(&ids),
        );
        let decoded: DecodedDomain = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert!(matches!(
            decoded.resolve(&mut identity_closure(output), &mut meter()),
            Err(hir::DefaultSourceAccessResolutionError::Build(
                hir::DefaultSourceAccessBuildError::GenericConstraintsOnEmpty
            ))
        ));
        let bytes = product(
            &encode(&hir::PersistentAccessDomainV1::universal()).unwrap(),
            &array(&ids),
        );
        let decoded: DecodedDomain = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        let mut empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        assert!(decoded.resolve(&mut empty, &mut meter()).is_err());
        assert!(hir::CanonicalPersistentIdsV1::try_new(vec![ids[0], ids[0]]).is_err());
    });
}

#[test]
fn source_slot_absence_is_distinct_from_an_empty_present_domain() {
    let absent = encode(&hir::OptionalDefaultSourceSlotDomainV1::Absent).unwrap();
    let empty = encode(&hir::OptionalDefaultSourceSlotDomainV1::Present(
        Domain::empty(),
    ))
    .unwrap();
    assert_ne!(absent, empty);
    assert_eq!(absent, [0xa1, 0, 1]);
    for bytes in [vec![0xa1, 0, 3], vec![0xa2, 0, 1, 1, 0], vec![0xa1, 0, 2]] {
        assert!(
            decode_canonical::<hir::DecodedOptionalDefaultSourceSlotDomainV1>(
                &bytes,
                DecodeLimits::default()
            )
            .is_err()
        );
    }
}

#[test]
fn source_domain_reader_does_not_repair_persistent_constraint_order() {
    with_hir_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let cone = hir::PersistentAccessConstraintV1::Cone(export.cone);
        let file = hir::PersistentAccessConstraintV1::File(export.source_files[0].identity.clone());
        for constraints in [vec![file, cone.clone()], vec![cone.clone(), cone]] {
            let persistent = [vec![0xa2, 0, 2, 1], array(&constraints)].concat();
            let decoded: DecodedDomain =
                decode_canonical(&product(&persistent, &[0x80]), DecodeLimits::default()).unwrap();
            assert!(matches!(
                decoded.resolve(&mut identity_closure(output), &mut meter()),
                Err(hir::DefaultSourceAccessResolutionError::Domain(
                    hir::PersistentAccessResolutionError::Domain(
                        hir::PersistentAccessDomainError::NonCanonicalOrder { .. }
                    )
                ))
            ));
        }
    });
}
