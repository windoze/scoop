use super::super::collector::Collector;
use super::*;
use crate::cross_cone_type_bridge::{
    dispatch::tests::support::Fixture as DispatchFixture,
    objects::tests::support::Fixture as ObjectFixture, tests::support::Fixture as TypeFixture,
};
use scoop_identity::{
    CallableTemplateOwner, CanonicalIdentifier, CborIdentityRecord, DeclarationScope,
    DefinitionOwnerAtom, DefinitionOwnerChain, PackagePath, PendingIdentityValidation,
    PersistentFunctionId, SignatureTypeKey, SourceDeclarationKey, SourceDeclarationSite,
};

#[test]
fn member_dependencies_are_disjoint_from_top_level_and_extension_callables() {
    let dispatch = DispatchFixture::new();

    let mut collector = Collector::new(&dispatch.graph);
    collector
        .member_target(scoop_identity::CallableDefinitionOwner::Strong(
            StrongCallableDefinitionOwner::Function(dispatch.methods[0].id()),
        ))
        .unwrap();
    let objects = ObjectFixture::new();

    let mut collector = Collector::new(&objects.graph);
    collector
        .member_target(scoop_identity::CallableDefinitionOwner::Strong(
            StrongCallableDefinitionOwner::PropertyAccessor(objects.accessors[1].id()),
        ))
        .unwrap();
    for accessor in [objects.accessors[0].id(), objects.accessors[2].id()] {
        let target = StrongCallableDefinitionOwner::PropertyAccessor(accessor);
        assert!(
            matches!(collector.member_target(scoop_identity::CallableDefinitionOwner::Strong(target)), Err(MirTypeBridgeReferenceError::NonMemberCallableTarget(actual)) if actual == scoop_identity::CallableDefinitionOwner::Strong(target))
        );
    }
    let fixture = TypeFixture::new();
    let GeneratedNominalKey::CoroutineFrame { source_callable } = fixture.frame.key() else {
        panic!()
    };
    let CallableTemplateOwner::Function(function) = source_callable.template() else {
        panic!()
    };
    assert!(matches!(
        Collector::new(&fixture.graph).member_target(
            scoop_identity::CallableDefinitionOwner::Strong(
                StrongCallableDefinitionOwner::Function(function)
            )
        ),
        Err(MirTypeBridgeReferenceError::NonMemberCallableTarget(_))
    ));
}

#[test]
fn extension_receiver_is_rejected_even_with_a_nominal_lexical_owner() {
    let fixture = TypeFixture::new();
    let method: CborIdentityRecord<PersistentFunctionId, _> =
        CborIdentityRecord::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                scoop_identity::ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(
                    fixture.empty.id(),
                )]),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("extension").unwrap(),
            0,
            Some(SignatureTypeKey::Nominal(fixture.empty.id())),
            vec![],
        ))
        .unwrap();
    let mut hir = scoop_hir::CanonicalHirFoundation::empty();
    hir.set_functions(vec![method.clone()]).unwrap();
    let hir: scoop_hir::DecodedHirFoundation = decode_canonical(&encode(&hir).unwrap()).unwrap();
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_external_graph_authorities(&fixture.graph)
        .unwrap();
    hir.register_identities(&mut pending).unwrap();
    hir.resolve_identities(&mut pending).unwrap();
    let graph = pending.finish().unwrap();
    assert!(matches!(
        Collector::new(&graph).member_target(scoop_identity::CallableDefinitionOwner::Strong(
            StrongCallableDefinitionOwner::Function(method.id())
        )),
        Err(MirTypeBridgeReferenceError::NonMemberCallableTarget(_))
    ));
}

#[test]
fn initialization_generated_role_cannot_be_a_dispatch_target() {
    let fixture = ObjectFixture::new();
    assert!(matches!(
        Collector::new(&fixture.graph).dispatch_target(
            scoop_identity::CallableDefinitionOwner::Strong(
                StrongCallableDefinitionOwner::GeneratedCallable(fixture.ensures[0].id())
            )
        ),
        Err(MirTypeBridgeReferenceError::GeneratedExecutionGate)
    ));
}
