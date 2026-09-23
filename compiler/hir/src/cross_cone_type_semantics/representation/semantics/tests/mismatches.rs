use super::*;

#[test]
fn source_key_access_origin_and_lexical_owner_mismatches_are_rejected() {
    use NominalRepresentationSourceMismatchV1 as Mismatch;
    let (fixture, owner) = fixtures::structure(unit(), 2);
    let table = fixture.table();
    for (replacement, error) in [
        (
            key("Subject", SourceNominalKind::Struct, 1, vec![]),
            Mismatch::SourceKey,
        ),
        (
            key("Other", SourceNominalKind::Struct, 0, vec![]),
            Mismatch::OwnerIdentity,
        ),
    ] {
        let mut changed = fixture.clone();
        changed.sources.get_mut(&owner).unwrap().key = replacement;
        mismatch(&table, &changed, error);
    }
    let mut changed = fixture.clone();
    let source = changed.sources.get_mut(&owner).unwrap();
    source.shape = NominalRepresentationShapeV1::Interface;
    mismatch(&table, &changed, Mismatch::SourceKind);

    let mut changed = fixture.clone();
    let source = changed.sources.get_mut(&owner).unwrap();
    source.access = DeclarationAccessSourceV1::try_new(
        DeclaredVisibilityV1::Internal,
        vec![],
        source.access.definition_origin().clone(),
    )
    .unwrap();
    mismatch(&table, &changed, Mismatch::Access);
    let mut changed = fixture.clone();
    let source = changed.sources.get_mut(&owner).unwrap();
    source.access = DeclarationAccessSourceV1::try_new(
        DeclaredVisibilityV1::Public,
        vec![SourceNominalId::Concrete(owner)],
        source.access.definition_origin().clone(),
    )
    .unwrap();
    mismatch(&table, &changed, Mismatch::AccessOwners);
    for foreign in [false, true] {
        let mut changed = fixture.clone();
        let source = changed.sources.get_mut(&owner).unwrap();
        let old = source.access.definition_origin().origin().source();
        let cone = if foreign {
            ConeCoordinate::new("example", "elsewhere", "1.0.0")
                .unwrap()
                .identity()
                .unwrap()
        } else {
            old.cone()
        };
        let origin = scoop_identity::SourceIdentity::new(cone, old.logical_path().clone()).unwrap();
        let context = SourceContextKey::File {
            source: origin.clone(),
        };
        let origin = ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(origin, SourceSpan::new(20, 30).unwrap(), &context).unwrap(),
        );
        source.access =
            DeclarationAccessSourceV1::try_new(DeclaredVisibilityV1::Public, vec![], origin)
                .unwrap();
        mismatch(
            &table,
            &changed,
            if foreign {
                Mismatch::AccessSource
            } else {
                Mismatch::Access
            },
        );
    }
}

#[test]
fn shape_sequences_base_backing_and_intrinsic_family_must_match_real_source() {
    let fixture = fixtures::mixed();
    let table = fixture.table();
    for owner in fixture.required.values() {
        let mut changed = fixture.clone();
        let source = changed.sources.get_mut(owner).unwrap();
        match &mut source.shape {
            NominalRepresentationShapeV1::Struct { fields, .. } => fields.reverse(),
            NominalRepresentationShapeV1::Enum { variants } => variants.reverse(),
            NominalRepresentationShapeV1::Class { base, .. } => {
                *base = scoop_identity::OptionalSignatureType::Present(Box::new(
                    SignatureTypeKey::Nominal(CoreBuiltinNominal::Any.identity_record().id()),
                ))
            }
            NominalRepresentationShapeV1::Object { backing_class, .. } => *backing_class = *owner,
            NominalRepresentationShapeV1::Intrinsic { .. } => {
                source.shape = NominalRepresentationShapeV1::Struct {
                    fields: vec![],
                    c_layout_policy: NominalCLayoutPolicyV1::Ordinary,
                }
            }
            NominalRepresentationShapeV1::Interface => continue,
        }
        mismatch(
            &table,
            &changed,
            NominalRepresentationSourceMismatchV1::Shape,
        );
    }
    let mut changed = fixture.clone();
    let source = changed
        .sources
        .values_mut()
        .find(|source| matches!(source.shape, NominalRepresentationShapeV1::Enum { .. }))
        .unwrap();
    let NominalRepresentationShapeV1::Enum { variants } = &mut source.shape else {
        unreachable!()
    };
    variants[0] = EnumRepresentationVariantV1::try_new(
        &EnumVariantIdentityKey::source(&source.key, CanonicalIdentifier::new("Payload").unwrap())
            .unwrap(),
        variants[0].fields().to_vec(),
        ExactTypeGcV1::ContainsManagedReferences,
    )
    .unwrap();
    mismatch(
        &table,
        &changed,
        NominalRepresentationSourceMismatchV1::Shape,
    );
    let (mut changed, owner) = fixtures::structure(unit(), 2);
    let table = changed.table();
    let source = changed.sources.get_mut(&owner).unwrap();
    let NominalRepresentationShapeV1::Struct {
        c_layout_policy, ..
    } = &mut source.shape
    else {
        unreachable!()
    };
    *c_layout_policy = NominalCLayoutPolicyV1::CLayout {
        contract: HirCLayoutContract {
            aligned: HirCLayoutValue::A8,
            packed: HirCLayoutValue::Natural,
        },
    };
    mismatch(
        &table,
        &changed,
        NominalRepresentationSourceMismatchV1::Shape,
    );
    let source = changed.sources.get_mut(&owner).unwrap();
    let NominalRepresentationShapeV1::Struct {
        fields,
        c_layout_policy,
    } = &mut source.shape
    else {
        unreachable!()
    };
    *c_layout_policy = NominalCLayoutPolicyV1::Ordinary;
    fields[0] = StructRepresentationFieldV1::try_new(
        &FieldIdentityKey::source_declared(
            &source.key,
            CanonicalIdentifier::new("field0").unwrap(),
        )
        .unwrap(),
        SignatureTypeKey::RawPointer(Box::new(unit())),
    )
    .unwrap();
    mismatch(
        &table,
        &changed,
        NominalRepresentationSourceMismatchV1::Shape,
    );
}

#[test]
fn independent_public_source_shape_is_required_to_agree_field_by_field() {
    let fixture = fixtures::mixed();
    let table = fixture.table();
    for owner in fixture.required.values() {
        let mut changed = fixture.clone();
        let source = changed.sources.get_mut(owner).unwrap();
        match &source.public {
            Some(NominalSourceShapeV1::Struct(shape)) => {
                source.public = Some(NominalSourceShapeV1::Struct(
                    StructSourceShapeV1::try_new(
                        shape.fields().iter().rev().cloned().collect(),
                        crate::NominalCLayoutPolicyV1::Ordinary,
                    )
                    .unwrap(),
                ))
            }
            Some(NominalSourceShapeV1::Enum(shape)) => {
                source.public = Some(NominalSourceShapeV1::Enum(
                    EnumSourceShapeV1::try_new(shape.variants().iter().rev().cloned().collect())
                        .unwrap(),
                ))
            }
            None => {
                source.public = Some(NominalSourceShapeV1::Struct(
                    StructSourceShapeV1::try_new(vec![], crate::NominalCLayoutPolicyV1::Ordinary)
                        .unwrap(),
                ))
            }
            _ => unreachable!(),
        }
        mismatch(
            &table,
            &changed,
            NominalRepresentationSourceMismatchV1::PublicSourceShape,
        );
    }
    let (mut changed, owner) = fixtures::structure(unit(), 1);
    let table = changed.table();
    let source = changed.sources.get_mut(&owner).unwrap();
    let Some(NominalSourceShapeV1::Struct(public)) = &source.public else {
        unreachable!()
    };
    source.public = Some(NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(
            vec![NominalSourceFieldV1::new(
                public.fields()[0].field(),
                SignatureTypeKey::RawPointer(Box::new(unit())),
            )],
            crate::NominalCLayoutPolicyV1::Ordinary,
        )
        .unwrap(),
    ));
    mismatch(
        &table,
        &changed,
        NominalRepresentationSourceMismatchV1::PublicSourceShape,
    );
}

#[test]
fn independent_public_source_policy_must_agree_in_the_metered_reader() {
    let (mut fixture, owner) = fixtures::structure(unit(), 1);
    let table = fixture.table();
    let source = fixture.sources.get_mut(&owner).unwrap();
    let Some(NominalSourceShapeV1::Struct(public)) = &source.public else {
        unreachable!()
    };
    source.public = Some(NominalSourceShapeV1::Struct(
        StructSourceShapeV1::try_new(
            public.fields().to_vec(),
            NominalCLayoutPolicyV1::CLayout {
                contract: HirCLayoutContract {
                    aligned: HirCLayoutValue::A8,
                    packed: HirCLayoutValue::A1,
                },
            },
        )
        .unwrap(),
    ));
    mismatch(
        &table,
        &fixture,
        NominalRepresentationSourceMismatchV1::PublicSourceShape,
    );
}
