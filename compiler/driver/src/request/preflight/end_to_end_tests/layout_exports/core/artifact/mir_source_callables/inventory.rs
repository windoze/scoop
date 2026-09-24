use super::*;

pub(super) fn check(replay: &Replay<'_, '_>) {
    let records = replay.section.callables().entries();
    let binding = records
        .iter()
        .find(|binding| declaration(binding).is_some())
        .unwrap();
    let required = declaration(binding).unwrap();
    let mut missing = records.to_vec();
    missing.retain(|binding| binding.implementation() != required.implementation());
    assert!(matches!(replay.reject(replay.ordinary, missing),
        Error::Missing { declaration, partition: Partition::TypeBridge } if declaration == required));

    let binding = replay.ordinary.exports().first().unwrap();
    let required = binding.declaration();
    let mut missing = replay.ordinary.exports().to_vec();
    missing.retain(|binding| binding.declaration() != required);
    assert!(
        matches!(replay.reject(&replay.ordinary(missing.clone()), records.to_vec()),
        Error::Missing { declaration, partition: Partition::Ordinary } if declaration == required)
    );

    let mut duplicate = records.to_vec();
    duplicate.push(replay.source_binding(required, binding.signature().clone()));
    component(
        replay.reject(replay.ordinary, duplicate.clone()),
        Component::Partition,
        required,
    );
    component(
        replay.reject(&replay.ordinary(missing), duplicate),
        Component::Partition,
        required,
    );

    let foreign = mir::CrossConeMirBridgeSectionV1::try_new(
        ConeIdentity::SINGLE_FILE,
        replay.foundation,
        vec![],
        vec![],
    )
    .unwrap();
    assert!(matches!(replay.reject(&foreign, vec![]),
        Error::Provider { expected, actual } if expected == replay.source.provider() && actual == ConeIdentity::SINGLE_FILE));

    let Some(hidden) = replay
        .function("sharedCallableHidden")
        .or_else(|| replay.function("privateHelper"))
    else {
        return;
    };
    let declaration = Declaration::Function(hidden);
    let actual = mir::StrongCallableBridgeSurfaceV1::from_odr_free_foundation(replay.foundation);
    let signature = actual
        .get(declaration.implementation().callable_owner())
        .unwrap()
        .signature();
    let mut extra = records.to_vec();
    extra.push(replay.source_binding(declaration, signature.clone()));
    assert!(matches!(replay.reject(replay.ordinary, extra),
        Error::Unexpected { declaration: actual, partition: Partition::TypeBridge } if actual == declaration));
    let mut extra = replay.ordinary.exports().to_vec();
    extra.push(
        mir::ParamFreeMirCallableExportV1::try_new(
            declaration,
            declaration.implementation(),
            signature.clone(),
        )
        .unwrap(),
    );
    assert!(
        matches!(replay.reject(&replay.ordinary(extra), records.to_vec()),
        Error::Unexpected { declaration: actual, partition: Partition::Ordinary } if actual == declaration)
    );
}
