//! Initialization source and role references are checked independently of digests.

use super::*;
use scoop_identity::{
    CallableOdrMemberId, CanonicalIdentifier, CborIdentityRecord, ConeIdentity, CoreBuiltinNominal,
    DeclarationScope, DefinitionOwnerChain, IdentityLayer, NonEmptyVec, OdrMemberRole, PackagePath,
    PendingIdentityValidation, RuntimeIdentityRecord, SignatureTypeKey, SourceDeclarationKey,
    SourceDeclarationSite,
};

#[test]
fn generic_initialization_rejects_swapped_roles_and_another_property_source() {
    let mut pending = PendingIdentityValidation::new();
    let nominal = CoreBuiltinNominal::Unit.identity_record();
    let exact = CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal.id())).unwrap();
    pending
        .register_canonical(IdentityLayer::Hir, nominal.clone())
        .unwrap();
    pending
        .register_canonical(IdentityLayer::Mir, exact.clone())
        .unwrap();
    let mut units = Vec::new();
    let mut bodies = Vec::new();
    for name in ["value", "other"] {
        let property = CborIdentityRecord::from_key(SourceDeclarationKey::extension_property(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::from_segments(Vec::new()),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            1,
            SignatureTypeKey::Nominal(nominal.id()),
        ))
        .unwrap();
        let source =
            CborIdentityRecord::from_key(InitializationUnitKey::ExtensionProperty(property.id()))
                .unwrap();
        let unit = InitializationUnitKey::GenericDelegatedExtensionApplication {
            property: property.id(),
            receiver_arguments: NonEmptyVec::from_first(exact.id(), []),
        };
        let group = CborIdentityRecord::from_key(unit.specialization_key().unwrap()).unwrap();
        pending
            .register_canonical(IdentityLayer::Hir, property)
            .unwrap();
        pending
            .register_canonical(IdentityLayer::Mir, source.clone())
            .unwrap();
        pending
            .register_canonical(IdentityLayer::Mir, group.clone())
            .unwrap();
        for role in [
            InitializationCallableRole::Initializer,
            InitializationCallableRole::Ensure,
        ] {
            let generated = CborIdentityRecord::from_key(GeneratedCallableKey::Initialization {
                unit: source.id(),
                role,
            })
            .unwrap();
            let member = CborIdentityRecord::from_key(
                OdrMemberKey::new(
                    group.id(),
                    OdrMemberRole::CallableBody,
                    OdrMemberDiscriminator::GeneratedCallable(generated.id()),
                )
                .unwrap(),
            )
            .unwrap();
            let body = RuntimeIdentityRecord::from_key(&CallableBodyKey::odr(
                CallableOdrMemberId::from_key(member.key()).unwrap(),
            ))
            .unwrap();
            pending
                .register_canonical(IdentityLayer::Mir, generated)
                .unwrap();
            pending
                .register_canonical(IdentityLayer::Lir, member)
                .unwrap();
            pending
                .register_canonical_callable_body(IdentityLayer::Lir, &body)
                .unwrap();
            bodies.push(body.id());
        }
        units.push(unit);
    }
    let graph = pending.finish().unwrap();
    for (index, expected_role) in [
        InitializationCallableRole::Initializer,
        InitializationCallableRole::Ensure,
    ]
    .into_iter()
    .enumerate()
    {
        assert!(callable_role(&graph, &units[0], bodies[index], expected_role).unwrap());
        assert!(!callable_role(&graph, &units[0], bodies[1 - index], expected_role).unwrap());
        assert!(!callable_role(&graph, &units[0], bodies[index + 2], expected_role).unwrap());
    }
}
