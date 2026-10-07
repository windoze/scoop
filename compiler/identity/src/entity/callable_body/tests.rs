use scoop_wire::{
    RuntimeDecodeErrorKind, decode_canonical, decode_runtime, encode, encode_runtime,
};

use super::{
    CallableBodyKey, CallableBodyKeyKind, DecodedCallableBodyKey, DecodedCallableBodyKeyKind,
    DecodedStrongCallableDefinitionOwner, ExecutableSourceEntryIdentity,
    ExecutableSourceEntryIdentityError, MainCallableBodyId, StrongCallableDefinitionOwner,
};
use crate::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    ExactCallableSignature, OdrMemberId, PackagePath, PersistentCallableBodyId,
    PersistentExactTypeId, PersistentFunctionId, PersistentGeneratedCallableId,
    PersistentInitializationUnitId, SourceDeclarationKey, SourceDeclarationSite,
};

#[test]
fn executable_source_entry_proves_main_shape_before_refining_the_body() {
    let declaration = source_function("main", None, Vec::new());
    let signature = ExactCallableSignature::new(
        crate::Effect::Ordinary,
        None,
        Vec::new(),
        PersistentExactTypeId([7; 32]),
    );
    let entry = ExecutableSourceEntryIdentity::try_new(&declaration, signature.clone()).unwrap();

    assert_eq!(entry.root_cone(), ConeIdentity::SINGLE_FILE);
    assert_eq!(entry.declaration(), declaration.id());
    assert_eq!(entry.source_signature(), &signature);
    assert_eq!(
        entry.main().body(),
        PersistentCallableBodyId::from_key(&CallableBodyKey::strong(
            StrongCallableDefinitionOwner::Function(declaration.id())
        ))
        .unwrap()
    );

    let wrong_name = source_function("start", None, Vec::new());
    assert_eq!(
        ExecutableSourceEntryIdentity::try_new(&wrong_name, signature.clone()),
        Err(ExecutableSourceEntryIdentityError::NotMain)
    );
    let receiver = source_function(
        "main",
        Some(crate::SignatureTypeKey::Nominal(crate::PersistentTypeId(
            [8; 32],
        ))),
        Vec::new(),
    );
    assert_eq!(
        ExecutableSourceEntryIdentity::try_new(&receiver, signature),
        Err(ExecutableSourceEntryIdentityError::InvalidDeclarationShape)
    );
}

#[test]
fn strong_callable_body_has_fixed_runtime_bytes_and_identity() {
    let function = PersistentFunctionId(ConeIdentity::CORE.0);
    let key = CallableBodyKey::strong(StrongCallableDefinitionOwner::Function(function));

    assert_eq!(
        encode_runtime(&key).unwrap(),
        [b"\x01\0\0\0\x01\0\0\0".as_slice(), function.as_array()].concat()
    );
    assert_eq!(
        PersistentCallableBodyId::from_key(&key)
            .unwrap()
            .to_string(),
        "636b6be175963f052db8a83269bd46e5ad1f75749e1c86a0cfd40dff5522fd49"
    );
}

#[test]
fn strong_callable_owner_has_its_own_closed_wire_sum() {
    let generated = PersistentGeneratedCallableId(ConeIdentity::CORE.0);
    let owner = StrongCallableDefinitionOwner::GeneratedCallable(generated);
    let encoded = encode(&owner).unwrap();
    assert_eq!(
        encoded,
        [b"\xa2\0\x04\x01\x58\x20".as_slice(), generated.as_array()].concat()
    );
    assert_eq!(
        decode_canonical::<DecodedStrongCallableDefinitionOwner>(&encoded,).unwrap(),
        DecodedStrongCallableDefinitionOwner::GeneratedCallable(
            crate::DecodedPersistentId::from_unvalidated_bytes(ConeIdentity::CORE.0),
        )
    );

    let mut obsolete_callable_owner_tag = encoded;
    obsolete_callable_owner_tag[2] = 6;
    assert!(
        decode_canonical::<DecodedStrongCallableDefinitionOwner>(&obsolete_callable_owner_tag,)
            .is_err()
    );
}

#[test]
fn root_gateway_keeps_root_and_main_as_separate_typed_fields() {
    let main_body = PersistentCallableBodyId(ConeIdentity::CORE.0);
    let main = MainCallableBodyId(main_body);
    let key = CallableBodyKey::root_gateway(ConeIdentity::SINGLE_FILE, main);
    let encoded = encode_runtime(&key).unwrap();

    assert_eq!(&encoded[..4], b"\x03\0\0\0");
    assert_eq!(&encoded[4..36], ConeIdentity::SINGLE_FILE.as_array());
    assert_eq!(&encoded[36..], main_body.as_array());
    assert_eq!(
        key.kind(),
        CallableBodyKeyKind::RootGateway {
            root_cone: ConeIdentity::SINGLE_FILE,
            main,
        }
    );
}

#[test]
fn decoded_body_keys_round_trip_every_runtime_variant() {
    let bytes = ConeIdentity::CORE.0;
    let encoded = [
        [b"\x01\0\0\0\x01\0\0\0".as_slice(), bytes.as_slice()].concat(),
        [b"\x01\0\0\0\x02\0\0\0".as_slice(), bytes.as_slice()].concat(),
        [b"\x01\0\0\0\x03\0\0\0".as_slice(), bytes.as_slice()].concat(),
        [b"\x01\0\0\0\x04\0\0\0".as_slice(), bytes.as_slice()].concat(),
        [b"\x02\0\0\0".as_slice(), bytes.as_slice()].concat(),
        [
            b"\x03\0\0\0".as_slice(),
            ConeIdentity::SINGLE_FILE.as_array(),
            bytes.as_slice(),
        ]
        .concat(),
        [b"\x04\0\0\0".as_slice(), bytes.as_slice()].concat(),
        [b"\x05\0\0\0".as_slice(), bytes.as_slice()].concat(),
    ];

    for bytes in encoded {
        let decoded = decode_runtime::<DecodedCallableBodyKey>(&bytes).unwrap();
        assert_eq!(encode_runtime(&decoded).unwrap(), bytes);
    }

    let strong = decode_runtime::<DecodedCallableBodyKey>(&encoded_strong(bytes)).unwrap();
    assert!(matches!(
        strong.kind(),
        DecodedCallableBodyKeyKind::Strong(
            DecodedStrongCallableDefinitionOwner::Function(id)
        ) if id.as_array() == &bytes
    ));

    let odr = decode_runtime::<DecodedCallableBodyKey>(
        &[b"\x02\0\0\0".as_slice(), bytes.as_slice()].concat(),
    )
    .unwrap();
    assert!(matches!(
        odr.kind(),
        DecodedCallableBodyKeyKind::Odr(id) if id.as_array() == OdrMemberId(bytes).as_array()
    ));

    let unit = decode_runtime::<DecodedCallableBodyKey>(
        &[b"\x04\0\0\0".as_slice(), bytes.as_slice()].concat(),
    )
    .unwrap();
    assert!(matches!(
        unit.kind(),
        DecodedCallableBodyKeyKind::InitializationStartupGateway(id)
            if id.as_array() == PersistentInitializationUnitId(bytes).as_array()
    ));
}

#[test]
fn release_hook_body_is_keyed_by_its_exact_owner() {
    let owner = PersistentExactTypeId(ConeIdentity::CORE.0);
    let key = CallableBodyKey::release_hook(owner);
    let bytes = encode_runtime(&key).unwrap();
    assert_eq!(bytes, [b"\x05\0\0\0".as_slice(), owner.as_array()].concat());
    let decoded = decode_runtime::<DecodedCallableBodyKey>(&bytes).unwrap();
    assert!(
        matches!(decoded.kind(), DecodedCallableBodyKeyKind::ReleaseHook { owner: actual } if actual.as_array() == owner.as_array())
    );
    assert_ne!(
        PersistentCallableBodyId::from_key(&key).unwrap(),
        PersistentCallableBodyId::from_key(&CallableBodyKey::release_hook(PersistentExactTypeId(
            [1; 32]
        )))
        .unwrap()
    );
}

fn source_function(
    name: &str,
    receiver: Option<crate::SignatureTypeKey>,
    parameters: Vec<crate::SignatureTypeKey>,
) -> CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        receiver,
        parameters,
    ))
    .unwrap()
}

#[test]
fn decoded_body_key_rejects_unknown_and_incomplete_variants() {
    let unknown = decode_runtime::<DecodedCallableBodyKey>(b"\x06\0\0\0").unwrap_err();
    assert_eq!(
        unknown.kind(),
        RuntimeDecodeErrorKind::UnknownTag { tag: 6 }
    );

    let unknown_owner =
        decode_runtime::<DecodedCallableBodyKey>(b"\x01\0\0\0\x05\0\0\0").unwrap_err();
    assert_eq!(
        unknown_owner.kind(),
        RuntimeDecodeErrorKind::UnknownTag { tag: 5 }
    );

    let incomplete = decode_runtime::<DecodedCallableBodyKey>(b"\x02\0\0\0").unwrap_err();
    assert!(matches!(
        incomplete.kind(),
        RuntimeDecodeErrorKind::UnexpectedEnd { .. }
    ));
}

fn encoded_strong(bytes: [u8; 32]) -> Vec<u8> {
    [b"\x01\0\0\0\x01\0\0\0".as_slice(), bytes.as_slice()].concat()
}
