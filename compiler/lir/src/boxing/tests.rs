use super::*;
use scoop_identity::{ConeCoordinate, ConeIdentity, ExactTypeKey, GeneratedNominalKey};

#[test]
fn external_boxed_references_preserve_zst_nonzero_and_recursive_rooting() {
    for (storage, ty) in cases() {
        let (external, proof) = fixture(ConeIdentity::CORE, storage, ty);
        proof.validate_reference(&Arena::new(), &external).unwrap();
        match proof {
            BoxedValueDescriptor::ZeroSized(value) => {
                assert!(matches!(value.descriptor(), TypeDescriptorRef::External(_)));
                let payload = BoxPayload::ZeroSized(value.clone());
                assert_eq!(payload.source(), None);
                assert_eq!(payload.descriptor(), value.descriptor());
            }
            BoxedValueDescriptor::NonZero(value) => {
                let expected = value.value().scan().contains_reference();
                let mut locals = Arena::new();
                let local = locals.alloc(Local::new(
                    "payload",
                    LocalStorage::NonZero(value.value().clone()),
                ));
                let place = value.bind_place(&locals, local).unwrap();
                assert_eq!(
                    matches!(place.rooting(), BoxPayloadRooting::RecursiveRegion(_)),
                    expected
                );
                assert!(matches!(
                    place.descriptor().descriptor(),
                    TypeDescriptorRef::External(_)
                ));
            }
        }
    }
}

#[test]
fn external_boxed_references_reject_rebound_missing_and_wrong_exact_entries() {
    let other = ConeCoordinate::new("test", "external-box", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    for provider in [ConeIdentity::CORE, other] {
        for (storage, ty) in cases() {
            let (mut external, proof) = fixture(provider, storage, ty);
            let (id, original) = external
                .iter()
                .next()
                .map(|(id, value)| (id, *value))
                .unwrap();
            let wrong_provider = if provider == other {
                ConeIdentity::CORE
            } else {
                other
            };
            external[id] = ExternalTypeDescriptor::new(wrong_provider, original.target()).unwrap();
            assert_eq!(
                proof.validate_reference(&Arena::new(), &external),
                Err(BoxDescriptorError::InvalidDescriptor)
            );
            external[id] = ExternalTypeDescriptor::new(provider, payload()).unwrap();
            assert_eq!(
                proof.validate_reference(&Arena::new(), &external),
                Err(BoxDescriptorError::InvalidDescriptor)
            );
            external[id] = original;
            proof.validate_reference(&Arena::new(), &external).unwrap();
            external.clear();
            assert_eq!(
                proof.validate_reference(&Arena::new(), &external),
                Err(BoxDescriptorError::InvalidDescriptor)
            );
        }
    }
}

fn cases() -> Vec<(ValueStorageLayoutV1, LirType)> {
    vec![
        (
            ValueStorageLayoutV1::zero_sized(1).unwrap(),
            LirType::Aggregate(Vec::new()),
        ),
        (
            ValueStorageLayoutV1::inline(8, 8, RefScan::None).unwrap(),
            LirType::I64,
        ),
        (
            ValueStorageLayoutV1::inline(16, 8, RefScan::References(vec![0])).unwrap(),
            LirType::Aggregate(vec![MANAGED_PTR, LirType::I64]),
        ),
    ]
}

fn payload() -> scoop_identity::PersistentExactTypeId {
    use scoop_identity::*;
    // This fixture exercises the private operand invariant. Public external
    // construction additionally requires the complete source shape selection.
    let source = SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("Payload").unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&source).unwrap(),
    ))
    .unwrap()
}

fn fixture(
    provider: ConeIdentity,
    storage: ValueStorageLayoutV1,
    ty: LirType,
) -> (Arena<ExternalTypeDescriptor>, BoxedValueDescriptor) {
    let nominal =
        scoop_identity::PersistentTypeId::from_generated_key(&GeneratedNominalKey::BoxedValue {
            payload: payload(),
        })
        .unwrap();
    let exact =
        scoop_identity::PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap();
    let definition = ExternalTypeDescriptor::new(provider, exact).unwrap();
    let mut external = Arena::new();
    let id = external.alloc(definition);
    let shape =
        TypeInstanceShapeV1::boxed_value(LirTargetProfile::DARWIN_AARCH64, storage).unwrap();
    let proof = BoxedValueDescriptor::from_shape(
        BoxedDescriptorReference::External { id, definition },
        exact,
        &shape,
        payload(),
        ty,
    )
    .unwrap();
    (external, proof)
}
