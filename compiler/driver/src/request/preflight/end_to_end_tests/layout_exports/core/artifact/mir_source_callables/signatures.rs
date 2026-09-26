use super::*;

pub(super) fn check(replay: &Replay<'_, '_>) {
    for binding in replay
        .section
        .callables()
        .entries()
        .iter()
        .filter(|binding| {
            matches!(
                binding.lowering_role(),
                mir::MirCallableLoweringRoleV1::PureVirtualTrap { .. }
            )
        })
    {
        let declaration = declaration(binding).unwrap();
        let role = match declaration {
            Declaration::Function(_) => mir::MirCallableLoweringRoleV1::Ordinary,
            Declaration::PropertyAccessor(_) => mir::MirCallableLoweringRoleV1::Accessor,
        };
        let changed = replay.binding(binding, binding.semantic_signature().clone(), role);
        component(
            replay.reject(replay.ordinary, replay.replace(changed)),
            Component::LoweringRole,
            declaration,
        );
    }
    let Some(keep) = replay.function("keep") else {
        return;
    };
    let declaration = Declaration::Function(keep);
    let binding = replay.ordinary.export(declaration).unwrap();
    let signature = binding.bridge_signature();
    assert_eq!(signature.gc_effect(), mir::GcEffect::NoGc);
    component(
        reject_ordinary_signature(
            replay,
            declaration,
            &mir::MirBridgeCallableSignatureV1::new(
                signature.exact().clone(),
                mir::GcEffect::Managed,
            ),
        ),
        Component::GcEffect,
        declaration,
    );

    let exact = signature.exact();
    let receiver = exact.receiver().into_option();
    assert!(receiver.is_some());
    assert_eq!(exact.parameters().len(), 1);
    let other = replay
        .section
        .types()
        .records()
        .iter()
        .find(|ty| ty.exact() != exact.result() && ty.facts().gc() == mir::MirGcKindV1::GcFree)
        .unwrap()
        .exact();
    for (expected, changed) in [
        (
            Component::Receiver,
            ExactCallableSignature::new(
                exact.effect(),
                None,
                exact.parameters().to_vec(),
                exact.result(),
            ),
        ),
        (
            Component::ParameterCount,
            ExactCallableSignature::new(exact.effect(), receiver, vec![], exact.result()),
        ),
        (
            Component::Parameter { index: 0 },
            ExactCallableSignature::new(exact.effect(), receiver, vec![other], exact.result()),
        ),
        (
            Component::Result,
            ExactCallableSignature::new(
                exact.effect(),
                receiver,
                exact.parameters().to_vec(),
                other,
            ),
        ),
    ] {
        component(
            reject_ordinary_signature(
                replay,
                declaration,
                &mir::MirBridgeCallableSignatureV1::new(changed, mir::GcEffect::NoGc),
            ),
            expected,
            declaration,
        );
    }
    ordinary_signature(replay);
}

fn ordinary_signature(replay: &Replay<'_, '_>) {
    let id = replay.function("sharedCallablePass").unwrap();
    let declaration = Declaration::Function(id);
    let original = replay.ordinary.export(declaration).unwrap();
    let exact = original.signature();
    let other = replay
        .section
        .types()
        .records()
        .iter()
        .find(|ty| ty.exact() != exact.result())
        .unwrap()
        .exact();
    let changed = ExactCallableSignature::new(
        exact.effect(),
        exact.receiver().into_option(),
        exact.parameters().to_vec(),
        other,
    );
    component(
        reject_ordinary_signature(
            replay,
            declaration,
            &mir::MirBridgeCallableSignatureV1::new(changed, original.gc_effect()),
        ),
        Component::Result,
        declaration,
    );
}

fn reject_ordinary_signature(
    replay: &Replay<'_, '_>,
    declaration: Declaration,
    signature: &mir::MirBridgeCallableSignatureV1,
) -> Error {
    let foundation = replay.changed_foundation(declaration.implementation(), signature.exact());
    let changed = mir::ParamFreeMirCallableExportV1::try_new(
        declaration,
        declaration.implementation(),
        signature.exact().clone(),
        signature.gc_effect(),
    )
    .unwrap();
    let records = replay
        .ordinary
        .exports()
        .iter()
        .map(|record| {
            if record.declaration() == declaration {
                changed.clone()
            } else {
                record.clone()
            }
        })
        .collect();
    let ordinary = mir::CrossConeMirBridgeSectionV1::try_new(
        replay.source.provider(),
        &foundation,
        records,
        replay.ordinary.selected().to_vec(),
    )
    .unwrap();
    replay.reject(&ordinary, replay.section.callables().entries().to_vec())
}
