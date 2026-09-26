use super::*;
use crate::{
    CanonicalBinderListV1, CanonicalNestedMemberRefsV1, CanonicalNestedNominalRefsV1,
    CanonicalNestedSourceSupportV1, CanonicalPersistentIdsV1, CanonicalPublicMemberRefsV1,
    CanonicalSignatureTypesV1, IntrinsicTypeParameters, NominalInheritanceModalityV1,
    NominalInterfaceRecordBuildError, NominalInterfaceRecordV1, ProtectedNestedSourceInterfaceV1,
    SourceNominalId, TypeParameterBinderV1, TypeParameterBoundsV1,
};

#[test]
fn all_intrinsic_families_share_public_source_contract_and_nested_binder_validation() {
    for family in families() {
        let expected = match family.parameters() {
            IntrinsicTypeParameters::None => vec![],
            IntrinsicTypeParameters::OneInvariantUnconstrained => {
                vec![TypeParameterBoundsV1::Unconstrained]
            }
            IntrinsicTypeParameters::OneInvariantValue => vec![TypeParameterBoundsV1::Value],
        };
        check_builders(family, &expected, true);
        let mut extra = expected.clone();
        extra.push(TypeParameterBoundsV1::Unconstrained);
        check_builders(family, &extra, false);
        if !expected.is_empty() {
            check_builders(family, &[], false);
            check_builders(family, &[TypeParameterBoundsV1::Ref], false);
            let other = if expected[0] == TypeParameterBoundsV1::Value {
                TypeParameterBoundsV1::Unconstrained
            } else {
                TypeParameterBoundsV1::Value
            };
            check_builders(family, &[other], false);
        }
    }
}

#[test]
fn public_intrinsic_record_rejects_a_disagreeing_source_kind() {
    let family = IntrinsicTypeKind::Boolean;
    assert!(matches!(
        public_record(family, PublicNominalKindV1::Class, binder_list(&[])),
        Err(NominalInterfaceRecordBuildError::SourceShapeKind {
            expected: PublicNominalKindV1::Class,
            actual: PublicNominalKindV1::Struct,
        })
    ));
}

fn check_builders(family: IntrinsicTypeKind, bounds: &[TypeParameterBoundsV1], valid: bool) {
    let shape = NominalSourceShapeV1::Intrinsic(NominalIntrinsicRepresentationV1::new(family));
    let binders = binder_list(bounds);
    assert_eq!(
        public_record(family, shape.kind(), binders.clone()).is_ok(),
        valid,
        "public {family:?} {bounds:?}"
    );
    assert_eq!(
        ProtectedNestedSourceInterfaceV1::try_new(
            shape.kind(),
            NominalInheritanceModalityV1::Final,
            binders,
            CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
            CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
            CanonicalNestedMemberRefsV1::try_new(vec![]).unwrap(),
            CanonicalNestedNominalRefsV1::try_new(vec![]).unwrap(),
            shape,
            CanonicalNestedSourceSupportV1::try_new(vec![]).unwrap(),
        )
        .is_ok(),
        valid,
        "nested support {family:?} {bounds:?}"
    );
}

fn public_record(
    family: IntrinsicTypeKind,
    kind: PublicNominalKindV1,
    binders: CanonicalBinderListV1,
) -> Result<NominalInterfaceRecordV1, NominalInterfaceRecordBuildError> {
    crate::nominal_interface_fixture::public_record(
        owner(family),
        kind,
        binders,
        CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        CanonicalPublicMemberRefsV1::try_new(vec![]).unwrap(),
        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        NominalSourceShapeV1::Intrinsic(NominalIntrinsicRepresentationV1::new(family)),
    )
}

fn owner(family: IntrinsicTypeKind) -> SourceNominalId {
    let kind = match family.target() {
        IntrinsicTypeTarget::Class => SourceNominalKind::Class,
        IntrinsicTypeTarget::Struct => SourceNominalKind::Struct,
    };
    let key = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Subject").unwrap(),
        kind,
        u32::from(family.parameters() != IntrinsicTypeParameters::None),
    );
    SourceNominalId::from_source_declaration(&key).unwrap()
}

fn binder_list(bounds: &[TypeParameterBoundsV1]) -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(
        bounds
            .iter()
            .enumerate()
            .map(|(index, bound)| {
                TypeParameterBinderV1::new(
                    CanonicalIdentifier::new(&format!("T{index}")).unwrap(),
                    bound.clone(),
                )
            })
            .collect(),
    )
    .unwrap()
}
