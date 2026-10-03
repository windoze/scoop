use super::*;
use scoop_hir::{
    NativeBoundaryCAbiV1, NativeBoundaryVariantDefinition, NativeBoundaryVariantFieldDefinition,
};
use scoop_identity::{
    EnumVariantFieldKey, EnumVariantFieldSelector, EnumVariantIdentityKey, PersistentEnumVariantId,
};

fn nullable(fixture: &mut Fixture, payload: PersistentExactTypeId, project: bool) {
    let source = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("MaybeNative").unwrap(),
        SourceNominalKind::Enum,
        1,
    );
    let variant = |name| {
        EnumVariantIdentityKey::source(&source, CanonicalIdentifier::new(name).unwrap()).unwrap()
    };
    let empty = variant("Empty");
    let present = variant("Present");
    let field = EnumVariantFieldKey::new(
        PersistentEnumVariantId::from_key(&present).unwrap(),
        EnumVariantFieldSelector::Named(CanonicalIdentifier::new("payload").unwrap()),
    );
    let payload_field = NativeBoundaryVariantFieldDefinition::new(
        &field,
        SignatureTypeKey::Binder { depth: 0, index: 0 },
    )
    .unwrap();
    let projection = NativeBoundaryCAbiV1::NullablePointer {
        none: PersistentEnumVariantId::from_key(&empty).unwrap(),
        payload: payload_field.field(),
    };
    let record = NativeBoundaryTypeDefinitionRecord::new(
        &source,
        &[1],
        NativeBoundaryNominalShape::Enum {
            variants: vec![
                NativeBoundaryVariantDefinition::new(&present, vec![payload_field]).unwrap(),
                NativeBoundaryVariantDefinition::new(&empty, Vec::new()).unwrap(),
            ],
        },
    )
    .unwrap();
    fixture.records[0] = if project {
        record.with_c_abi(projection).unwrap()
    } else {
        record
    };
    fixture.exact = insert(
        fixture,
        ExactTypeKey::NominalApplication {
            origin: PersistentGenericTypeId::from_source_declaration(&source).unwrap(),
            arguments: NonEmptyVec::from_first(payload, []),
        },
    );
}

fn insert(fixture: &mut Fixture, key: ExactTypeKey) -> PersistentExactTypeId {
    let record = CborIdentityRecord::from_key(key).unwrap();
    let id = record.id();
    fixture.exact_types.insert(id, record.into_shared_key());
    id
}

fn fixture() -> Fixture {
    Fixture::new(
        ConeIdentity::SINGLE_FILE,
        "ResourceToken",
        scoop_hir::IntegerKind::UNSIGNED_64,
    )
}

#[test]
fn nullable_projection_replays_typed_payload_and_preserves_raw_or_code_pointer_niches() {
    for code in [false, true] {
        let mut fixture = fixture();
        let key = if code {
            ExactTypeKey::NativeFunctionPointer {
                calling_convention: scoop_identity::CallingConvention::C,
                parameters: Vec::new(),
                result: fixture.unit,
            }
        } else {
            ExactTypeKey::RawPointer(fixture.unit)
        };
        let pointer = insert(&mut fixture, key);
        nullable(&mut fixture, pointer, true);
        fixture.with_normalizer(|normalizer| {
            let storage = normalizer.c_storage(fixture.exact).unwrap();
            if code {
                assert!(matches!(storage, CanonicalCStorageType::CodePointer { exact_type, storage: CPointerStorage::NullableWrapper(wrapper) } if exact_type == fixture.exact && wrapper == fixture.exact));
            } else {
                assert!(matches!(storage, CanonicalCStorageType::DataPointer { exact_type, pointee: CDataPointee::OpaqueUnit, storage: CPointerStorage::NullableWrapper(wrapper) } if exact_type == fixture.exact && wrapper == fixture.exact));
            }
            let layout = normalizer.scoop_layout(fixture.exact).unwrap();
            assert_eq!(layout.size, 8);
            assert!(layout.gc_free);
        });
    }
}

#[test]
fn nullable_projection_rejects_nonpointer_and_nested_nullable_payloads() {
    for kind in 0..3 {
        let mut fixture = fixture();
        let payload = match kind {
            0 => fixture.scalar,
            1 => insert(
                &mut fixture,
                ExactTypeKey::Nominal(
                    scoop_identity::CoreBuiltinNominal::Any
                        .identity_record()
                        .id(),
                ),
            ),
            _ => {
                let unit = fixture.unit;
                let pointer = insert(&mut fixture, ExactTypeKey::RawPointer(unit));
                nullable(&mut fixture, pointer, true);
                fixture.exact
            }
        };
        nullable(&mut fixture, payload, true);
        fixture.with_normalizer(|normalizer| assert!(matches!(normalizer.c_storage(fixture.exact), Err(NativeBoundaryCompileError::Target(NativeBoundaryTargetError::NotCAbiSafe { exact })) if exact == fixture.exact)));
    }
}

#[test]
fn a_pointer_niche_does_not_grant_an_implicit_c_projection() {
    let mut fixture = fixture();
    let unit = fixture.unit;
    let pointer = insert(&mut fixture, ExactTypeKey::RawPointer(unit));
    nullable(&mut fixture, pointer, false);
    fixture.with_normalizer(|normalizer| {
        assert_eq!(normalizer.scoop_layout(fixture.exact).unwrap().size, 8);
        assert!(matches!(normalizer.c_storage(fixture.exact), Err(NativeBoundaryCompileError::Target(NativeBoundaryTargetError::NotCAbiSafe { exact })) if exact == fixture.exact));
    });
}
