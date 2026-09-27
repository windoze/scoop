use super::*;

pub(super) fn check(replay: &Replay<'_>, combined: bool) {
    assert_eq!(
        replay.fixture_bindings().count(),
        if combined { 12 } else { 4 }
    );
    for binding in replay.fixture_bindings() {
        let records = replay
            .section
            .callables()
            .entries()
            .iter()
            .filter(|entry| entry.implementation() != binding.implementation())
            .cloned()
            .collect();
        assert!(matches!(replay.reject(records), Error::Missing(id) if id == declaration(binding)));
    }
    if !combined {
        return;
    }
    let surface = mir::StrongCallableBridgeSurfaceV1::from_foundation(replay.foundation);
    let mut extras = 0;
    for source in replay
        .source
        .metadata()
        .public
        .callable_interfaces()
        .all_declarations()
    {
        let scoop_identity::CallableTemplateOrigin::Constructor(id) = source.declaration() else {
            continue;
        };
        if !matches!(
            source.declared_visibility(),
            hir::DeclaredVisibilityV1::Private | hir::DeclaredVisibilityV1::Internal
        ) {
            continue;
        }
        let target = StrongCallableDefinitionOwner::Constructor(id);
        let Some(actual) = surface
            .bridges()
            .iter()
            .find(|bridge| bridge.implementation() == target.callable_owner())
        else {
            continue;
        };
        let lowered = actual.signature();
        let Some(owner) = lowered.receiver().into_option() else {
            continue;
        };
        let semantic = ExactCallableSignature::new(
            lowered.effect(),
            None,
            lowered.parameters().to_vec(),
            owner,
        );
        let binding = mir::ParamFreeMirCallableBindingV1::try_new(
            mir::MirCallableBridgeAuthority {
                identities: replay.source.metadata().identities,
                foundation: replay.foundation,
                types: replay.section.types(),
            },
            mir::MirCallableOriginV1::Constructor(id),
            target,
            mir::MirBridgeCallableSignatureV1::new(semantic, mir::GcEffect::Managed),
            mir::MirBridgeCallableSignatureV1::new(lowered.clone(), mir::GcEffect::Managed),
            mir::MirCallableLoweringRoleV1::ClassInitializer { owner },
        )
        .unwrap();
        let mut records = replay.section.callables().entries().to_vec();
        records.push(binding);
        assert!(matches!(replay.reject(records), Error::Unexpected(actual) if actual == id));
        extras += 1;
    }
    assert!(
        extras >= 3,
        "private and internal constructors have actual local bodies"
    );
}
