use super::*;

#[test]
fn wire_preserves_all_four_dispatch_implementation_branches() {
    let fixture = DirectFixture::new(4);
    let declaration = match fixture.target {
        scoop_identity::StrongCallableDefinitionOwner::Function(function) => {
            scoop_identity::DispatchDeclarationOwner::Function(function)
        }
        _ => unreachable!(),
    };
    let implementations = [
        ExactDispatchImplementationV1::AbstractObligation {
            declaration,
            trap_target: scoop_identity::CallableDefinitionOwner::Strong(fixture.target),
            receiver: ExactDispatchReceiverAdaptationV1::Identity,
        },
        ExactDispatchImplementationV1::DirectStrongTarget {
            target: scoop_identity::CallableDefinitionOwner::Strong(fixture.target),
            receiver: ExactDispatchReceiverAdaptationV1::Identity,
        },
        ExactDispatchImplementationV1::InterfaceDefaultTarget {
            target: scoop_identity::CallableDefinitionOwner::Strong(fixture.target),
            receiver: ExactDispatchReceiverAdaptationV1::Identity,
        },
        ExactDispatchImplementationV1::AdjustThunkTarget(
            scoop_identity::CallableDefinitionOwner::Strong(fixture.target),
        ),
    ];
    let entries: Vec<_> = implementations
        .into_iter()
        .enumerate()
        .map(|(index, implementation)| {
            let mut entry = fixture.identity_input();
            entry.position = ExactDispatchPositionV1::from_u32(index as u32);
            entry.slot = named_dispatch_slot(&format!("branch{index}"));
            if matches!(
                implementation,
                ExactDispatchImplementationV1::AdjustThunkTarget(_)
            ) {
                entry.slot_receiver_layout = Some(&fixture.owner);
            }
            entry.implementation = implementation;
            entry
        })
        .collect();
    let mut resolver = fixture.local_resolver();
    let record = ExactDispatchExportV1::replay(
        TARGET,
        (&fixture.vtable).into(),
        &entries,
        &fixture.foundation,
        &mut resolver,
    )
    .unwrap();
    assert!(matches!(
        record.entries()[0].implementation(),
        ExactDispatchImplementationV1::AbstractObligation { .. }
    ));
    assert!(matches!(
        record.entries()[1].implementation(),
        ExactDispatchImplementationV1::DirectStrongTarget { .. }
    ));
    assert!(matches!(
        record.entries()[2].implementation(),
        ExactDispatchImplementationV1::InterfaceDefaultTarget { .. }
    ));
    assert!(matches!(
        record.entries()[3].implementation(),
        ExactDispatchImplementationV1::AdjustThunkTarget(_)
    ));
    let bytes = encode(&record).unwrap();
    let decoded = decode_canonical::<DecodedExactDispatchExportV1>(&bytes).unwrap();
    assert_eq!(encode(&decoded).unwrap(), bytes);
    decoded.validate_against(&record).unwrap();
}
