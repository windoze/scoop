use super::*;
use mir::{
    MirDispatchImplementationV1 as Implementation, MirDispatchReceiverAdaptationV1 as Receiver,
};

pub(super) fn check(replay: &Replay<'_>, combined: bool) {
    if combined {
        default_target(replay);
        boxing_target(replay);
    } else {
        order_and_target(replay);
        abstract_target(replay);
    }
}

fn order_and_target(replay: &Replay<'_>) {
    let owner = replay.owner("SharedDispatchOrder");
    let entries = replay
        .section
        .dispatch()
        .get(owner)
        .unwrap()
        .vtable()
        .entries();
    assert_eq!(entries.len(), 2);
    let changed = entries
        .iter()
        .rev()
        .enumerate()
        .map(|(position, entry)| {
            mir::MirDispatchEntryV1::new(
                entry.slot(),
                mir::MirDispatchPositionV1::new(position as u32),
                entry.signature().clone(),
                entry.implementation(),
            )
        })
        .collect();
    component(
        replay.reject(replay.replace(replay.vtable(owner, changed))),
        Component::SlotIdentity,
    );
    let missing = replay.vtable(owner, entries[..1].to_vec());
    component(replay.reject(replay.replace(missing)), Component::Slots);
    let mut wrong = entries.to_vec();
    wrong[0] = mir::MirDispatchEntryV1::new(
        entries[0].slot(),
        entries[0].position(),
        entries[0].signature().clone(),
        Implementation::DirectStrongTarget {
            target: entries[1].implementation().target(),
            receiver: Receiver::Identity,
        },
    );
    component(
        replay.reject(replay.replace(replay.vtable(owner, wrong))),
        Component::Implementation,
    );
}

fn abstract_target(replay: &Replay<'_>) {
    let base = replay
        .section
        .dispatch()
        .get(replay.owner("SharedDispatchAbstract"))
        .unwrap();
    let base = base.vtable().entries()[0].implementation().target();
    for name in ["SharedDispatchAgain", "SharedDispatchInherited"] {
        let owner = replay.owner(name);
        let entries = replay
            .section
            .dispatch()
            .get(owner)
            .unwrap()
            .vtable()
            .entries();
        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        let Implementation::AbstractObligation {
            declaration,
            trap_target,
            ..
        } = entry.implementation()
        else {
            panic!("the fixture has an abstract obligation")
        };
        assert_ne!(trap_target, base);
        let changed = mir::MirDispatchEntryV1::new(
            entry.slot(),
            entry.position(),
            entry.signature().clone(),
            Implementation::AbstractObligation {
                declaration,
                trap_target: base,
                receiver: Receiver::Identity,
            },
        );
        component(
            replay.reject(replay.replace(replay.vtable(owner, vec![changed]))),
            Component::Implementation,
        );
    }
}

fn default_target(replay: &Replay<'_>) {
    let owner = replay.owner("SharedDispatchDerived");
    let record = replay.section.dispatch().get(owner).unwrap();
    let mut count = 0;
    let tables = record
        .itables()
        .iter()
        .map(|table| {
            let entries = table
                .entries()
                .iter()
                .map(|entry| {
                    if let Implementation::InterfaceDefaultTarget { target, receiver } =
                        entry.implementation()
                    {
                        count += 1;
                        mir::MirDispatchEntryV1::new(
                            entry.slot(),
                            entry.position(),
                            entry.signature().clone(),
                            Implementation::DirectStrongTarget { target, receiver },
                        )
                    } else {
                        entry.clone()
                    }
                })
                .collect();
            mir::MirInterfaceDispatchTableV1::new(table.interface(), entries)
        })
        .collect();
    assert!(count > 0);
    let changed = mir::ParamFreeMirDispatchSchemaV1::try_new(
        replay.authority(),
        owner,
        record.vtable().clone(),
        tables,
        &mut meter(),
    )
    .unwrap();
    component(
        replay.reject(replay.replace(changed)),
        Component::Implementation,
    );
}

fn boxing_target(replay: &Replay<'_>) {
    let owner = replay.owner("SharedDispatchValue");
    let original = replay.section.callables().entries().iter().find(|binding| matches!(binding.origin(), mir::MirCallableOriginV1::Generated { role: GeneratedCallableKey::BoxingAdjust { payload, .. }, .. } if *payload == owner) && binding.semantic_signature().exact().receiver().into_option() == Some(owner) && binding.semantic_signature().exact().parameters().len() == 1).unwrap();
    let spare = replay
        .section
        .callables()
        .entries()
        .iter()
        .find_map(|binding| {
            let mir::MirCallableOriginV1::Function(id) = binding.origin() else {
                return None;
            };
            let key = replay
                .source
                .metadata()
                .identities
                .canonical_key::<_, SourceDeclarationKey>(*id)
                .unwrap();
            matches!(key.name(), DeclarationName::Named(name) if name.as_str() == "spare")
                .then_some(binding.implementation())
        })
        .unwrap();
    let changed = mir::ParamFreeMirCallableBindingV1::try_new(
        mir::MirCallableBridgeAuthority {
            identities: replay.source.metadata().identities,
            foundation: replay.foundation,
            types: replay.section.types(),
        },
        original.origin().clone(),
        original.implementation(),
        original.semantic_signature().clone(),
        original.lowered_signature().clone(),
        mir::MirCallableLoweringRoleV1::BoxingAdjust { target: spare },
    )
    .unwrap();
    let callables = mir::CanonicalMirCallableBindingsV1::try_new(
        replay
            .section
            .callables()
            .entries()
            .iter()
            .map(|binding| {
                if binding.implementation() == changed.implementation() {
                    changed.clone()
                } else {
                    binding.clone()
                }
            })
            .collect(),
    )
    .unwrap();
    let dispatch = mir::CanonicalMirDispatchSchemasV1::try_new(
        mir::MirDispatchSchemaAuthority {
            callables: &callables,
            ..replay.authority()
        },
        replay.section.dispatch().records().to_vec(),
        &mut meter(),
    )
    .unwrap();
    component(
        replay
            .validate(&dispatch, &callables, &mut meter())
            .unwrap_err(),
        Component::CallableRole,
    );
    let missing = mir::CanonicalMirCallableBindingsV1::try_new(
        replay
            .section
            .callables()
            .entries()
            .iter()
            .filter(|binding| binding.implementation() != original.implementation())
            .cloned()
            .collect(),
    )
    .unwrap();
    assert!(
        matches!(replay.validate(replay.section.dispatch(), &missing, &mut meter()), Err(Error::MissingCallable(target)) if target == original.implementation())
    );
}
