use scoop_identity::{
    CanonicalIdentifier, CanonicalIdentifierError, CborIdentityRecord, ConeIdentity,
    DeclarationScope, DefinitionOwnerChain, IdentityReferenceError, PackagePath,
    PendingIdentityValidation, PersistentTypeId, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind, ValidatedIdentityGraph,
};
use scoop_wire::{Encoder, WireEncode, WireErrorKind, decode_canonical, encode};

use super::*;

#[test]
fn binder_list_preserves_declaration_order_and_has_fixed_wire() {
    let binders = CanonicalBinderListV1::try_new(vec![
        binder("T", TypeParameterBoundsV1::Unconstrained),
        binder("U", TypeParameterBoundsV1::Value),
        binder("V", TypeParameterBoundsV1::Ref),
    ])
    .unwrap();

    assert_eq!(binders.len_u32(), 3);
    assert_eq!(binders.binders()[0].name().as_str(), "T");
    assert_eq!(binders.binders()[1].name().as_str(), "U");
    assert_eq!(binders.binders()[2].name().as_str(), "V");
    assert_eq!(
        hex(&encode(&binders).unwrap()),
        "83a201615402a10001a201615502a10002a201615602a10003"
    );
}

#[test]
fn producer_canonicalizes_signature_sets_and_rejects_duplicate_names() {
    let fixture = fixture();
    let mut expected = vec![
        SignatureTypeKey::Nominal(fixture.interface_a.id()),
        SignatureTypeKey::Nominal(fixture.interface_b.id()),
    ];
    expected.sort_unstable();

    let signatures =
        CanonicalSignatureTypesV1::try_new(expected.iter().cloned().rev().collect()).unwrap();
    assert_eq!(signatures.values(), expected);

    assert_eq!(
        CanonicalSignatureTypesV1::try_new(vec![expected[0].clone(), expected[0].clone()]),
        Err(SignatureTypeSetBuildError::Duplicate(Box::new(
            expected[0].clone()
        )))
    );

    let duplicate = binder("T", TypeParameterBoundsV1::Unconstrained);
    assert_eq!(
        CanonicalBinderListV1::try_new(vec![duplicate.clone(), duplicate]),
        Err(TypeParameterBinderBuildError::DuplicateName(identifier(
            "T"
        )))
    );
}

#[test]
fn nominal_bounds_require_at_least_one_bound() {
    let interfaces = CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap();

    assert_eq!(
        NominalTypeParameterBoundsV1::try_new(None, interfaces),
        Err(TypeParameterBoundsBuildError::EmptyNominal)
    );
}

#[test]
fn decoded_binders_resolve_typed_nominal_references() {
    let fixture = fixture();
    let interfaces = CanonicalSignatureTypesV1::try_new(vec![
        SignatureTypeKey::Nominal(fixture.interface_b.id()),
        SignatureTypeKey::Nominal(fixture.interface_a.id()),
    ])
    .unwrap();
    let bounds = NominalTypeParameterBoundsV1::try_new(
        Some(SignatureTypeKey::Nominal(fixture.class.id())),
        interfaces,
    )
    .unwrap();
    let expected =
        CanonicalBinderListV1::try_new(vec![binder("T", TypeParameterBoundsV1::Nominal(bounds))])
            .unwrap();
    let decoded = decode_binders(&expected);
    let mut authority = authority(&fixture);

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_signature_sets() {
    let fixture = fixture();
    let mut ordered = vec![
        SignatureTypeKey::Nominal(fixture.interface_a.id()),
        SignatureTypeKey::Nominal(fixture.interface_b.id()),
    ];
    ordered.sort_unstable();

    let duplicate = SignatureSequence(vec![ordered[0].clone(), ordered[0].clone()]);
    let mut duplicate_authority = authority(&fixture);
    assert!(matches!(
        decode_signatures(&duplicate).resolve(&mut duplicate_authority),
        Err(SignatureTypeSetValidationError::Duplicate { index: 1 })
    ));

    let reversed = SignatureSequence(ordered.into_iter().rev().collect());
    let mut reversed_authority = authority(&fixture);
    assert!(matches!(
        decode_signatures(&reversed).resolve(&mut reversed_authority),
        Err(SignatureTypeSetValidationError::NonCanonicalOrder { index: 1 })
    ));
}

#[test]
fn reader_rejects_duplicate_and_noncanonical_binder_names() {
    let duplicate = binder("T", TypeParameterBoundsV1::Unconstrained);
    let decoded = decode_binders(&BinderSequence(vec![duplicate.clone(), duplicate]));
    let mut authority = empty_authority();

    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(BinderListValidationError::DuplicateName { index: 1, name })
            if name == identifier("T")
    ));

    let decoded = decode_binders(&InvalidBinderName);
    let mut authority = empty_authority();
    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(BinderListValidationError::Binder {
            index: 0,
            error: TypeParameterBinderResolutionError::Name(
                CanonicalIdentifierError::InvalidContinuation
            ),
        })
    ));
}

#[test]
fn reader_rejects_unknown_bound_tags() {
    let error = decode_canonical::<DecodedTypeParameterBoundsV1>(&[0xa1, 0x00, 0x05]).unwrap_err();

    assert!(matches!(error.kind(), WireErrorKind::UnknownTag { tag: 5 }));
}

#[test]
fn reader_rejects_missing_typed_nominal_authority() {
    let fixture = fixture();
    let interfaces = CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap();
    let bounds = NominalTypeParameterBoundsV1::try_new(
        Some(SignatureTypeKey::Nominal(fixture.class.id())),
        interfaces,
    )
    .unwrap();
    let expected =
        CanonicalBinderListV1::try_new(vec![binder("T", TypeParameterBoundsV1::Nominal(bounds))])
            .unwrap();
    let decoded = decode_binders(&expected);
    let mut authority = empty_authority();

    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(BinderListValidationError::Binder {
            index: 0,
            error: TypeParameterBinderResolutionError::Bounds(
                TypeParameterBoundsResolutionError::Reference(
                    IdentityReferenceError::Missing { .. }
                )
            ),
        })
    ));
}

struct Fixture {
    class: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    interface_a: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
    interface_b: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>,
}

fn fixture() -> Fixture {
    Fixture {
        class: nominal("Base", SourceNominalKind::Class),
        interface_a: nominal("Readable", SourceNominalKind::Interface),
        interface_b: nominal("Writable", SourceNominalKind::Interface),
    }
}

fn nominal(
    name: &str,
    kind: SourceNominalKind,
) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site,
        identifier(name),
        kind,
        0,
    ))
    .unwrap()
}

fn authority(fixture: &Fixture) -> ValidatedIdentityGraph {
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_authority(ConeIdentity::SINGLE_FILE)
        .unwrap();
    pending
        .register_external_canonical_authority(fixture.class.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(fixture.interface_a.clone())
        .unwrap();
    pending
        .register_external_canonical_authority(fixture.interface_b.clone())
        .unwrap();
    pending.finish().unwrap()
}

fn empty_authority() -> ValidatedIdentityGraph {
    PendingIdentityValidation::new().finish().unwrap()
}

fn identifier(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}

fn binder(name: &str, bounds: TypeParameterBoundsV1) -> TypeParameterBinderV1 {
    TypeParameterBinderV1::new(identifier(name), bounds)
}

fn decode_binders<T: WireEncode>(value: &T) -> DecodedCanonicalBinderListV1 {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

fn decode_signatures<T: WireEncode>(value: &T) -> DecodedCanonicalSignatureTypesV1 {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

struct SignatureSequence(Vec<SignatureTypeKey>);

impl WireEncode for SignatureSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for signature in &self.0 {
            signature.encode(encoder)?;
        }
        Ok(())
    }
}

struct BinderSequence(Vec<TypeParameterBinderV1>);

impl WireEncode for BinderSequence {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for binder in &self.0 {
            binder.encode(encoder)?;
        }
        Ok(())
    }
}

struct InvalidBinderName;

impl WireEncode for InvalidBinderName {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(1)?;
        encoder.map(2)?;
        encoder.field(1)?;
        encoder.text("bad-name")?;
        encoder.field(2)?;
        TypeParameterBoundsV1::Unconstrained.encode(encoder)
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
