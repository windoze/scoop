use super::*;

pub(super) fn check(replay: &Replay<'_, '_>) {
    let records = replay.section.callables().entries();
    let binding = records
        .iter()
        .find(|binding| declaration(binding).is_some())
        .unwrap();
    let required = declaration(binding).unwrap();
    let mut missing = records.to_vec();
    missing.retain(|binding| {
        binding.implementation()
            != scoop_identity::CallableDefinitionOwner::Strong(required.implementation())
    });
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
}
