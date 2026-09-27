use super::*;

pub(super) fn check(replay: &Replay<'_>) {
    let mut count = 0;
    for binding in replay.section.callables().entries() {
        let Some(callable) = callable(binding) else {
            continue;
        };
        let remaining = replay.resolve(
            replay
                .section
                .callables()
                .entries()
                .iter()
                .filter(|candidate| candidate.implementation() != binding.implementation())
                .cloned()
                .collect(),
        );
        assert!(matches!(
            replay.validate(&remaining),
            Err(Error::MissingCallable(id)) if id == callable
        ));
        let strong = mir::StrongCallableBridgeSurfaceV1::try_new(
            replay
                .strong
                .bridges()
                .iter()
                .filter(|record| record.implementation() != CallableOwner::Generated(callable))
                .cloned()
                .collect(),
        )
        .unwrap();
        assert!(matches!(
            scoop_slib::validate_shared_mir_equality(
                replay.source,
                &[],
                &strong,
                replay.section.callables()
            ),
            Err(Error::UnexpectedCallable(id)) if id == callable
        ));
        gc_effect(replay, binding, callable);
        count += 1;
    }
    assert!(count > 0);
    let metadata = replay.source.metadata();
    let mut missing = metadata.foundation.clone();
    missing.set_generated_callables(vec![]).unwrap();
    let missing = hir::OdrFreeHirFoundation::try_new(missing).unwrap();
    let missing = replay
        .source
        .section()
        .validate_shared_foundation(
            hir::SharedTypeMetadataV1 {
                foundation: &missing,
                ..metadata
            },
            &[],
        )
        .unwrap();
    assert!(matches!(
        scoop_slib::validate_shared_mir_equality(
            missing,
            &[],
            replay.strong,
            replay.section.callables()
        ),
        Err(Error::MissingSource(_))
    ));
}

fn gc_effect(
    replay: &Replay<'_>,
    binding: &mir::ParamFreeMirCallableBindingV1,
    callable: PersistentGeneratedCallableId,
) {
    let signature = binding.semantic_signature().exact();
    let owner = signature.receiver().into_option().unwrap();
    if replay.section.types().get(owner).unwrap().facts().gc() != mir::MirGcKindV1::GcFree {
        return;
    }
    let signature = mir::MirBridgeCallableSignatureV1::new(signature.clone(), mir::GcEffect::NoGc);
    let changed = mir::ParamFreeMirCallableBindingV1::try_new(
        mir::MirCallableBridgeAuthority {
            identities: replay.source.metadata().identities,
            foundation: replay.foundation,
            types: replay.section.types(),
        },
        binding.origin().clone(),
        binding.implementation(),
        signature.clone(),
        signature,
        *binding.lowering_role(),
    )
    .unwrap();
    let changed = replay.resolve(
        replay
            .section
            .callables()
            .entries()
            .iter()
            .map(|candidate| {
                if candidate.implementation() == binding.implementation() {
                    changed.clone()
                } else {
                    candidate.clone()
                }
            })
            .collect(),
    );
    assert!(matches!(
        replay.validate(&changed),
        Err(Error::Signature(id)) if id == callable
    ));
}
