use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, ConeIdentity, DeclarationScope, DefinitionOwnerChain,
    IdentityReferenceError, PackagePath, PendingIdentityValidation, PersistentTypeId,
    SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{decode_canonical, encode};

use super::*;

#[test]
fn binder_uses_preserve_declaration_order_and_duplicates_with_fixed_wire() {
    let binder = SignatureTypeKey::Binder { depth: 0, index: 0 };
    let uses = CanonicalBinderUseListV1::try_new(vec![binder.clone(), binder.clone()]).unwrap();

    assert_eq!(uses.len_u32(), 2);
    assert!(!uses.is_empty());
    assert_eq!(uses.arguments(), &[binder.clone(), binder]);
    assert_eq!(
        hex(&encode(&uses).unwrap()),
        "82a3000701000200a3000701000200"
    );

    let decoded: DecodedCanonicalBinderUseListV1 =
        decode_canonical(&encode(&uses).unwrap()).unwrap();
    let mut authority = PendingIdentityValidation::new().finish().unwrap();
    assert_eq!(decoded.resolve(&mut authority).unwrap(), uses);
}

#[test]
fn decoded_uses_resolve_each_nominal_through_typed_authority() {
    let nominal = nominal("Token");
    let uses =
        CanonicalBinderUseListV1::try_new(vec![SignatureTypeKey::Nominal(nominal.id())]).unwrap();
    let decoded: DecodedCanonicalBinderUseListV1 =
        decode_canonical(&encode(&uses).unwrap()).unwrap();
    let mut authority = authority(&nominal);

    assert_eq!(decoded.resolve(&mut authority).unwrap(), uses);
}

#[test]
fn decoded_uses_report_the_failing_argument_position() {
    let nominal = nominal("Missing");
    let uses = CanonicalBinderUseListV1::try_new(vec![
        SignatureTypeKey::Binder { depth: 0, index: 0 },
        SignatureTypeKey::Nominal(nominal.id()),
    ])
    .unwrap();
    let decoded: DecodedCanonicalBinderUseListV1 =
        decode_canonical(&encode(&uses).unwrap()).unwrap();
    let mut authority = PendingIdentityValidation::new().finish().unwrap();

    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(BinderUseListValidationError::Reference {
            index: 1,
            error: IdentityReferenceError::Missing { .. },
        })
    ));
}

fn nominal(name: &str) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Struct,
        0,
    ))
    .unwrap()
}

fn authority(
    nominal: &CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
) -> scoop_identity::ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_authority(ConeIdentity::SINGLE_FILE)
        .unwrap();
    pending
        .register_external_canonical_authority(nominal.clone())
        .unwrap();
    pending.finish().unwrap()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
