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
        let error = decode_canonical::<DecodedCoreBootstrapBridgeSectionV1>(
            &bytes,
            DecodeLimits::default(),
        )
        .unwrap_err();
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
    let mut strong = StrongCallableBridgeSurfaceV1::from_odr_free_foundation(
        &OdrFreeMirFoundation::try_new(fixture.mir.clone()).unwrap(),
    );
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
fn reader_requires_the_producer_role_without_a_second_bridge() {
    let fixture = fixture();
    let (mut identities, foundation) = validate_foundations(&fixture);
    assert_eq!(
        decode(&fixture.section).validate(ConeIdentity::SINGLE_FILE, &mut identities, &foundation),
        Err(MirProductionValidationError::Relation(
            MirProductionBuildError::UnexpectedInitializationCycle(ConeIdentity::SINGLE_FILE)
        ))
    );
    let mut ordinary = fixture.section.clone();
    for bridge in &mut ordinary.strong_callable_bridges.bridges {
        bridge.role = CallableRole::Ordinary;
    }
    let (mut identities, foundation) = validate_foundations(&fixture);
    assert_eq!(
        decode(&ordinary).validate(ConeIdentity::CORE, &mut identities, &foundation),
        Err(MirProductionValidationError::Relation(
            MirProductionBuildError::MissingInitializationCycle
        ))
    );
}
