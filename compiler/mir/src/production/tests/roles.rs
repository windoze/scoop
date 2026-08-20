use super::*;
use crate::CallableRole;

#[test]
fn reader_requires_a_known_role_on_every_strong_record() {
    let fixture = fixture();
    let record = encode(&fixture.section.strong_callable_bridges.bridges()[0]).unwrap();
    for missing in [true, false] {
        let mut bytes = encode(&fixture.section).unwrap();
        let offset = bytes
            .windows(record.len())
            .position(|part| part == record)
            .unwrap();
        let mut invalid = record.clone();
        if missing {
            invalid[0] = 0xa2;
            invalid.truncate(invalid.len() - 2);
        } else {
            *invalid.last_mut().unwrap() = 3;
        }
        bytes.splice(offset..offset + record.len(), invalid);
        let error = decode_canonical::<DecodedCoreBootstrapBridgeSectionV1>(&bytes).unwrap_err();
        assert_eq!(
            error.kind(),
            &if missing {
                WireErrorKind::InvalidLength {
                    expected: 3,
                    actual: 2,
                }
            } else {
                WireErrorKind::UnknownTag { tag: 3 }
            }
        );
    }
}

#[test]
fn builder_and_reader_reject_two_initialization_roles_in_the_shared_table() {
    let mut fixture = fixture();
    for bridge in &mut fixture.section.strong_callable_bridges.bridges {
        bridge.role = CallableRole::InitializationCycle;
    }
    assert_eq!(
        StrongCallableBridgeSurfaceV1::try_new(
            fixture.section.strong_callable_bridges.bridges.clone()
        ),
        Err(MirProductionBuildError::DuplicateInitializationCycle)
    );
    let (mut identities, foundation) = validate_foundations(&fixture);
    assert_eq!(
        decode(&fixture.section).validate(ConeIdentity::CORE, &mut identities, &foundation),
        Err(MirProductionValidationError::Relation(
            MirProductionBuildError::DuplicateInitializationCycle
        ))
    );
}

#[test]
fn builder_and_reader_require_a_source_function_for_initialization() {
    let mut fixture = fixture();
    let generated = generated_callable_sorting_before(fixture.function.id());
    let implementation = CallableOwner::Generated(generated.id());
    fixture
        .mir
        .set_generated_callables(vec![generated])
        .unwrap();
    let mut signatures = fixture.mir.callable_signatures().to_vec();
    signatures.push(CallableSignatureRecord::new(
        CallableSignatureSubject::Strong(implementation),
        ExactCallableSignature::new(Effect::Ordinary, None, vec![], fixture.exact_unit),
    ));
    fixture.mir.set_callable_signatures(signatures).unwrap();
    let mut strong = StrongCallableBridgeSurfaceV1::from_foundation(&fixture.mir);
    strong
        .bridges
        .iter_mut()
        .find(|bridge| bridge.implementation == implementation)
        .unwrap()
        .role = CallableRole::InitializationCycle;
    assert_eq!(
        StrongCallableBridgeSurfaceV1::try_new(strong.bridges.clone()),
        Err(MirProductionBuildError::InvalidInitializationCycleOwner(
            implementation
        ))
    );
    fixture.section.strong_callable_bridges = strong;
    let (mut identities, foundation) = validate_foundations(&fixture);
    assert_eq!(
        decode(&fixture.section).validate(ConeIdentity::CORE, &mut identities, &foundation),
        Err(MirProductionValidationError::Relation(
            MirProductionBuildError::InvalidInitializationCycleOwner(implementation)
        ))
    );
}

#[test]
fn role_presence_is_independent_of_provider_coordinates_and_output_kinds() {
    let ordinary = scoop_identity::ConeCoordinate::new("test", "protocol-provider", "1.0.0")
        .unwrap()
        .identity()
        .unwrap();
    for provider in [ConeIdentity::CORE, ordinary] {
        let fixture = fixture_at(provider);
        let unmarked = StrongCallableBridgeSurfaceV1::from_foundation(&fixture.mir);
        let executable = EntryMirBridgeBranchV1::Executable(Box::new(
            EntryMirBridgeV1::new(
                entry_source(&fixture.function, fixture.exact_unit),
                CallableOwner::Function(fixture.function.id()),
            )
            .unwrap(),
        ));
        for entry in [EntryMirBridgeBranchV1::Library, executable] {
            for strong in [&unmarked, &fixture.section.strong_callable_bridges] {
                let section =
                    CoreBootstrapBridgeSectionV1::try_new(provider, entry.clone(), strong.clone())
                        .unwrap();
                let (mut identities, foundation) = validate_foundations(&fixture);
                assert_eq!(
                    decode(&section).validate(provider, &mut identities, &foundation),
                    Ok(section)
                );
            }
        }
    }
}
