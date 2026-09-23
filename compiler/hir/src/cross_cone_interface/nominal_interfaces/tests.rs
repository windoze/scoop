use scoop_identity::{CallableTemplateOrigin, IdentityReferenceError};
use scoop_wire::{DecodeLimits, WireErrorKind, decode_canonical, encode};

use super::*;
use crate::{
    CanonicalBinderListV1, CanonicalPersistentIdsV1, CanonicalPublicMemberRefsV1,
    CanonicalSignatureTypesV1, NominalSourceShapeV1, PublicMemberRefV1, PublicNominalKindV1,
    StructSourceShapeV1,
};

mod support;

use support::*;

#[test]
fn nominal_record_has_fixed_field_wire_and_accessors() {
    let fixture = Fixture::new();
    let record = fixture.record();
    let expected = [
        b"\xa8\x01".as_slice(),
        encode(&record.declaration()).unwrap().as_slice(),
        b"\x02".as_slice(),
        encode(&record.kind()).unwrap().as_slice(),
        b"\x03".as_slice(),
        encode(record.type_parameters()).unwrap().as_slice(),
        b"\x04".as_slice(),
        encode(record.exact_supertypes()).unwrap().as_slice(),
        b"\x05".as_slice(),
        encode(record.constructors()).unwrap().as_slice(),
        b"\x06".as_slice(),
        encode(record.members()).unwrap().as_slice(),
        b"\x07".as_slice(),
        encode(record.nested_bindings()).unwrap().as_slice(),
        b"\x08".as_slice(),
        encode(record.source_shape()).unwrap().as_slice(),
    ]
    .concat();

    assert_eq!(encode(&record).unwrap(), expected);
    assert_eq!(
        record.declaration(),
        scoop_identity::NominalDeclarationOwner::GenericTemplate(fixture.owner.id())
    );
    assert_eq!(record.kind(), PublicNominalKindV1::Struct);
    assert_eq!(record.type_parameters().len_u32(), 1);
    assert_eq!(record.exact_supertypes().values().len(), 1);
    assert_eq!(record.constructors().values(), &[fixture.constructor.id()]);
    assert_eq!(record.members().members().len(), 2);
    assert_eq!(
        record.nested_bindings().values(),
        &[fixture.nested_binding.id()]
    );
    assert_eq!(record.source_shape().kind(), PublicNominalKindV1::Struct);
}

#[test]
fn decoded_record_resolves_every_typed_constituent() {
    let fixture = Fixture::new();
    let expected = fixture.record();
    let decoded = decode_record(&expected);
    let mut authority = fixture.authority(true);

    assert_eq!(decoded.resolve(&mut authority).unwrap(), expected);
}

#[test]
fn producer_rejects_shape_kind_and_member_partition_violations() {
    let fixture = Fixture::new();

    assert_eq!(
        minimal_record(
            &fixture,
            PublicNominalKindV1::Class,
            CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
            empty_struct_shape(),
        ),
        Err(NominalInterfaceRecordBuildError::SourceShapeKind {
            expected: PublicNominalKindV1::Class,
            actual: PublicNominalKindV1::Struct,
        })
    );

    assert_eq!(
        NominalInterfaceRecordV1::try_new(
            scoop_identity::NominalDeclarationOwner::GenericTemplate(fixture.owner.id()),
            PublicNominalKindV1::Interface,
            CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(vec![fixture.constructor.id()]).unwrap(),
            CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
            CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
            NominalSourceShapeV1::Interface,
        ),
        Err(NominalInterfaceRecordBuildError::ConstructorsNotAllowed(
            PublicNominalKindV1::Interface
        ))
    );

    let constructor = PublicMemberRefV1::Callable(CallableTemplateOrigin::Constructor(
        fixture.constructor.id(),
    ));
    assert_eq!(
        minimal_record(
            &fixture,
            PublicNominalKindV1::Struct,
            CanonicalPublicMemberRefsV1::try_new(vec![constructor]).unwrap(),
            empty_struct_shape(),
        ),
        Err(NominalInterfaceRecordBuildError::ConstructorMember(
            fixture.constructor.id()
        ))
    );

    let variant = PublicMemberRefV1::Callable(CallableTemplateOrigin::VariantConstructor(
        fixture.variant.id(),
    ));
    assert_eq!(
        minimal_record(
            &fixture,
            PublicNominalKindV1::Struct,
            CanonicalPublicMemberRefsV1::try_new(vec![variant]).unwrap(),
            empty_struct_shape(),
        ),
        Err(NominalInterfaceRecordBuildError::VariantConstructorMember(
            fixture.variant.id()
        ))
    );
}

#[test]
fn reader_replays_record_invariants_after_typed_resolution() {
    let fixture = Fixture::new();
    let mut decoded = decode_record(&fixture.record());
    decoded.kind = PublicNominalKindV1::Class;
    let mut authority = fixture.authority(true);

    assert!(matches!(
        decoded.resolve(&mut authority),
        Err(NominalInterfaceRecordResolutionError::Record(
            NominalInterfaceRecordBuildError::SourceShapeKind {
                expected: PublicNominalKindV1::Class,
                actual: PublicNominalKindV1::Struct,
            }
        ))
    ));
}

#[test]
fn reader_reports_the_constituent_that_lacks_typed_authority() {
    let fixture = Fixture::new();
    let mut empty = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();
    assert!(matches!(
        decode_record(&fixture.record()).resolve(&mut empty),
        Err(NominalInterfaceRecordResolutionError::Declaration(
            IdentityReferenceError::Missing { .. }
        ))
    ));

    let mut without_field = fixture.authority(false);
    assert!(matches!(
        decode_record(&fixture.record()).resolve(&mut without_field),
        Err(NominalInterfaceRecordResolutionError::SourceShape(
            NominalSourceShapeResolutionError::NominalField {
                index: 0,
                error: crate::NominalSourceFieldResolutionError::Field(
                    IdentityReferenceError::Missing { .. }
                ),
            }
        ))
    ));
}

#[test]
fn reader_requires_the_exact_record_map_shape() {
    let error =
        decode_canonical::<DecodedNominalInterfaceRecordV1>(&[0xa0], DecodeLimits::default())
            .unwrap_err();

    assert!(matches!(
        error.kind(),
        WireErrorKind::InvalidLength {
            expected: 8,
            actual: 0,
        }
    ));
}

fn minimal_record(
    fixture: &Fixture,
    kind: PublicNominalKindV1,
    members: CanonicalPublicMemberRefsV1,
    source_shape: NominalSourceShapeV1,
) -> Result<NominalInterfaceRecordV1, NominalInterfaceRecordBuildError> {
    NominalInterfaceRecordV1::try_new(
        scoop_identity::NominalDeclarationOwner::GenericTemplate(fixture.owner.id()),
        kind,
        CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        members,
        CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        source_shape,
    )
}

fn empty_struct_shape() -> NominalSourceShapeV1 {
    NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(Vec::new(), crate::NominalCLayoutPolicyV1::Ordinary).unwrap(),
    )
}

fn decode_record(value: &NominalInterfaceRecordV1) -> DecodedNominalInterfaceRecordV1 {
    decode_canonical(&encode(value).unwrap(), DecodeLimits::default()).unwrap()
}
