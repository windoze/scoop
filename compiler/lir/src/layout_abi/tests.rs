use super::*;
use scoop_identity::{
    CanonicalIdentifier, ConeCoordinate, DeclarationScope, DefinitionOwnerChain, DispatchTableKey,
    ExactTypeKey, LayoutKey, PackagePath, PendingIdentityValidation, PersistentExactTypeId,
    PersistentFunctionId, PersistentLayoutId, PersistentTypeId, RepresentationRole,
    SourceDeclarationKey, SourceDeclarationSite,
};
use scoop_wire::{decode_canonical, encode};

fn fixture() -> (
    ConeIdentity,
    [LayoutAbiSemanticTargetV1; 5],
    ValidatedIdentityGraph,
) {
    let provider = ConeCoordinate::new("test", "layout-abi-dependency", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    let site = SourceDeclarationSite::new(
        provider,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let source = SourceDeclarationKey::nominal(
        site.clone(),
        CanonicalIdentifier::new("Value").unwrap(),
        scoop_identity::SourceNominalKind::Struct,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&source).unwrap();
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap();
    let layout = PersistentLayoutId::from_key(&LayoutKey::new(
        exact,
        crate::LirTargetProfile::DARWIN_AARCH64.wire_id(),
        RepresentationRole::ManagedValue,
    ))
    .unwrap();
    let dispatch =
        scoop_identity::PersistentDispatchTableId::from_key(&DispatchTableKey::vtable(exact))
            .unwrap();
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        site,
        CanonicalIdentifier::new("run").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let targets = [
        LayoutAbiSemanticTargetV1::Layout(layout),
        LayoutAbiSemanticTargetV1::Descriptor(exact),
        LayoutAbiSemanticTargetV1::Dispatch(dispatch),
        LayoutAbiSemanticTargetV1::Callable(StrongCallableDefinitionOwner::Function(function)),
        LayoutAbiSemanticTargetV1::ShapeSupport(nominal),
    ];
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(provider).unwrap();
    pending.register_authority(layout).unwrap();
    pending.register_authority(exact).unwrap();
    pending.register_authority(dispatch).unwrap();
    pending.register_authority(function).unwrap();
    pending.register_authority(nominal).unwrap();
    (provider, targets, pending.finish().unwrap())
}

#[test]
fn five_targets_and_dependency_have_closed_canonical_wire() {
    let (provider, targets, mut identities) = fixture();
    for (index, target) in targets.into_iter().enumerate() {
        let bytes = encode(&target).unwrap();
        assert_eq!(bytes[..4], [0xa2, 0, index as u8 + 1, 1]);
        let decoded: DecodedLayoutAbiSemanticTargetV1 = decode_canonical(&bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.resolve(&mut identities).unwrap(), target);

        let relation = LayoutAbiDependencyV1::new(provider, target);
        let relation_bytes = encode(&relation).unwrap();
        let decoded: DecodedLayoutAbiDependencyV1 = decode_canonical(&relation_bytes).unwrap();
        assert_eq!(encode(&decoded).unwrap(), relation_bytes);
        assert_eq!(decoded.resolve(&mut identities).unwrap(), relation);
    }
}

#[test]
fn reader_rejects_open_products_unknown_tags_and_wrong_identity_kinds() {
    let (_, targets, mut identities) = fixture();
    let original = encode(&targets[0]).unwrap();
    for prefix in [[0xa1, 0, 1], [0xa3, 0, 1], [0xa2, 0, 6]] {
        let mut bytes = original.clone();
        bytes[..3].copy_from_slice(&prefix);
        assert!(decode_canonical::<DecodedLayoutAbiSemanticTargetV1>(&bytes).is_err());
    }
    let mut wrong = original;
    wrong[2] = 2;
    let decoded: DecodedLayoutAbiSemanticTargetV1 = decode_canonical(&wrong).unwrap();
    assert!(matches!(
        decoded.resolve(&mut identities),
        Err(LayoutAbiDependencyError::Identity(_))
    ));
}
