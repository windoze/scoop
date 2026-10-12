use super::*;
use crate::{
    CanonicalSourceNominalIdsV1, NominalDeclarationDetailsV1, NominalInheritanceModalityV1,
};

fn details() -> NominalDeclarationDetailsV1 {
    NominalDeclarationDetailsV1::new(
        NominalInheritanceModalityV1::Abstract,
        crate::DeclaredVisibilityV1::Public,
        CanonicalPersistentIdsV1::empty(),
        Default::default(),
        Default::default(),
        crate::NominalDispatchOrderV1::empty(PublicNominalKindV1::Class),
        crate::CanonicalNominalDispatchSelectionsV1::empty(),
        None,
        crate::NominalInstantiationConditionsV1::empty(),
        Default::default(),
        None,
    )
}

#[test]
fn sealed_family_preserves_typed_subtypes_and_legacy_encoding() {
    let fixture = Fixture::new();
    let open = details();
    assert_eq!(encode(&open).unwrap()[0], 0xab);
    for subtypes in [
        vec![],
        vec![
            SourceNominalId::Concrete(fixture.superclass.id()),
            SourceNominalId::GenericTemplate(fixture.owner.id()),
        ],
    ] {
        let sealed = open.clone().with_sealed_subtypes(Some(
            CanonicalSourceNominalIdsV1::try_new(subtypes).unwrap(),
        ));
        let bytes = encode(&sealed).unwrap();
        assert_eq!(bytes[0], 0xac);
        let decoded: DecodedNominalDeclarationDetailsV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(
            decoded.resolve(&mut fixture.authority(true)).unwrap(),
            sealed
        );
    }
}

#[test]
fn sealed_family_rejects_noncanonical_subtype_order() {
    let fixture = Fixture::new();
    let mut subtypes = [
        SourceNominalId::Concrete(fixture.superclass.id()),
        SourceNominalId::GenericTemplate(fixture.owner.id()),
    ];
    subtypes.sort();
    let mut bytes = encode(&details()).unwrap();
    bytes[0] = 0xac;
    bytes.extend([12, 0x82]);
    for subtype in subtypes.iter().rev() {
        bytes.extend(encode(subtype).unwrap());
    }
    let decoded: DecodedNominalDeclarationDetailsV1 = decode_canonical(&bytes).unwrap();
    assert!(matches!(
        decoded.resolve(&mut fixture.authority(true)),
        Err(NominalDeclarationDetailsResolutionError::SealedSubtypes(_))
    ));
}

#[test]
fn sealed_family_requires_an_abstract_class_or_interface() {
    let record = Fixture::new().record();
    let details = record
        .declaration_details()
        .clone()
        .with_sealed_subtypes(Some(Default::default()));
    assert!(matches!(
        NominalInterfaceRecordV1::try_new(
            record.declaration(),
            record.kind(),
            record.type_parameters().clone(),
            record.exact_supertypes().clone(),
            record.constructors().clone(),
            record.members().clone(),
            record.nested_bindings().clone(),
            record.source_shape().clone(),
            details
        ),
        Err(NominalInterfaceRecordBuildError::Modality {
            kind: PublicNominalKindV1::Struct,
            ..
        })
    ));
}
