use super::*;
use crate::{CoreBuiltinNominal, DecodedGeneratedCallableKey, Effect, ExactTypeKey};
use scoop_wire::{decode_canonical, encode};

#[test]
fn test_entry_preserves_its_cone_role_signature_and_generated_body() {
    let record = CborIdentityRecord::from_key(GeneratedCallableKey::TestEntry {
        cone: ConeIdentity::SINGLE_FILE,
    })
    .unwrap();
    let signature = signature();
    let entry = ExecutableEntryIdentity::for_test(&record, signature.clone()).unwrap();
    assert_eq!(entry.root_cone(), ConeIdentity::SINGLE_FILE);
    assert_eq!(entry.source_signature(), &signature);
    assert_eq!(
        entry.declaration(),
        ExecutableEntryDeclaration::Generated(record.id())
    );
    assert_eq!(
        entry.main().body(),
        PersistentCallableBodyId::from_key(&CallableBodyKey::strong(
            crate::StrongCallableDefinitionOwner::GeneratedCallable(record.id())
        ))
        .unwrap()
    );
    let bytes = encode(&entry).unwrap();
    let decoded: DecodedExecutableEntryIdentity = decode_canonical(&bytes).unwrap();
    assert!(
        matches!(decoded.declaration(), DecodedExecutableEntryDeclaration::Generated(id)
        if id.as_array() == record.id().as_array())
    );
    assert_eq!(encode(&decoded).unwrap(), bytes);

    let other = CborIdentityRecord::from_key(GeneratedCallableKey::TestEntry {
        cone: ConeIdentity::CORE,
    })
    .unwrap();
    assert_ne!(
        entry.main(),
        ExecutableEntryIdentity::for_test(&other, signature)
            .unwrap()
            .main()
    );
    assert_eq!(
        encode(record.key()).unwrap(),
        [
            b"\xa2\x00\x11\x01\x58\x20".as_slice(),
            ConeIdentity::SINGLE_FILE.as_array()
        ]
        .concat()
    );
    let decoded: DecodedGeneratedCallableKey =
        decode_canonical(&encode(record.key()).unwrap()).unwrap();
    assert_eq!(encode(&decoded).unwrap(), encode(record.key()).unwrap());
}

#[test]
fn test_entry_rejects_other_roles_and_non_entry_signatures() {
    let signature = signature();
    let other = CborIdentityRecord::from_key(GeneratedCallableKey::DerivedEquality {
        exact_owner: signature.result(),
    })
    .unwrap();
    assert_eq!(
        ExecutableEntryIdentity::for_test(&other, signature.clone()),
        Err(ExecutableEntryIdentityError::NotTestEntry)
    );
    let entry = CborIdentityRecord::from_key(GeneratedCallableKey::TestEntry {
        cone: ConeIdentity::SINGLE_FILE,
    })
    .unwrap();
    let unit = signature.result();
    for invalid in [
        ExactCallableSignature::new(Effect::Suspend, None, vec![], unit),
        ExactCallableSignature::new(Effect::Ordinary, Some(unit), vec![], unit),
        ExactCallableSignature::new(Effect::Ordinary, None, vec![unit, unit], unit),
    ] {
        assert_eq!(
            ExecutableEntryIdentity::for_test(&entry, invalid),
            Err(ExecutableEntryIdentityError::InvalidDeclarationShape)
        );
    }
}

#[test]
fn entry_declaration_wire_distinguishes_identity_domains() {
    let bytes = [7; 32];
    let source = ExecutableEntryDeclaration::Source(PersistentFunctionId(bytes));
    let generated = ExecutableEntryDeclaration::Generated(PersistentGeneratedCallableId(bytes));
    assert_ne!(encode(&source).unwrap(), encode(&generated).unwrap());
    for declaration in [source, generated] {
        let bytes = encode(&declaration).unwrap();
        let decoded: DecodedExecutableEntryDeclaration = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
    }
    assert!(
        decode_canonical::<DecodedExecutableEntryDeclaration>(
            &encode(&PersistentFunctionId(bytes)).unwrap()
        )
        .is_err()
    );
    assert!(
        decode_canonical::<DecodedExecutableEntryDeclaration>(b"\xa2\x00\x03\x01\x00").is_err()
    );
}

fn signature() -> ExactCallableSignature {
    let unit = crate::PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        CoreBuiltinNominal::Unit.identity_record().id(),
    ))
    .unwrap();
    ExactCallableSignature::new(Effect::Ordinary, None, vec![], unit)
}
