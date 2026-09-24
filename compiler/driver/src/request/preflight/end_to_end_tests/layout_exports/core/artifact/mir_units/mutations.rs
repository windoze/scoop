use super::*;
use mir::{MirTypeBridgeSectionError as Error, MirTypeBridgeUnitProblemV1 as Problem};
use scoop_identity::{CallableOwner, Effect, ExactCallableSignature};

pub(super) fn check(replay: &Replay<'_>, units: &[mir::MirTypeBridgeInitializationUnitV1]) {
    assert!(matches!(
        mir::replay_source_initialization_units(
            replay.metadata.provider,
            &[],
            replay.foundation,
            replay.strong,
            replay.metadata.identities,
            replay.unit_result,
            &mut meter(),
        ),
        Err(Error::Unit {
            problem: Problem::MissingSource,
            ..
        })
    ));
    assert!(matches!(
        mir::replay_source_initialization_units(
            ConeIdentity::SINGLE_FILE,
            replay.metadata.source_initialization_units(),
            replay.foundation,
            replay.strong,
            replay.metadata.identities,
            replay.unit_result,
            &mut meter(),
        ),
        Err(Error::Unit {
            problem: Problem::WrongProvider,
            ..
        })
    ));
    for unit in units {
        for target in [unit.initializer(), unit.ensure()] {
            let missing = strong_changed(replay.strong, target.callable_owner(), None);
            assert!(
                matches!(Replay { strong: &missing, ..*replay }.run(&mut meter()),
                Err(Error::Unit { unit: actual, problem: Problem::MissingRole }) if actual == unit.unit())
            );
            let wrong = mir::StrongCallableBridgeV1::new(
                target.callable_owner(),
                ExactCallableSignature::new(
                    Effect::Ordinary,
                    None,
                    vec![replay.unit_result],
                    replay.unit_result,
                ),
            );
            let wrong = strong_changed(replay.strong, target.callable_owner(), Some(wrong));
            assert!(
                matches!(Replay { strong: &wrong, ..*replay }.run(&mut meter()),
                Err(Error::Unit { unit: actual, problem: Problem::Signature }) if actual == unit.unit())
            );
        }
    }
    let mut measured = meter();
    replay.run(&mut measured).unwrap();
    let mut shared = scoop_wire::BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    replay.run(&mut shared).unwrap();
    assert!(matches!(replay.run(&mut shared), Err(Error::Resource(_))));
    for limits in [
        DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        },
        DecodeLimits {
            logical_heap_bytes: 0,
            ..DecodeLimits::default()
        },
    ] {
        assert!(matches!(
            replay.run(&mut scoop_wire::BudgetMeter::new(limits)),
            Err(Error::Resource(_))
        ));
    }
}

fn strong_changed(
    source: &mir::StrongCallableBridgeSurfaceV1,
    target: CallableOwner,
    replacement: Option<mir::StrongCallableBridgeV1>,
) -> mir::StrongCallableBridgeSurfaceV1 {
    mir::StrongCallableBridgeSurfaceV1::try_new(
        source
            .bridges()
            .iter()
            .filter_map(|record| {
                if record.implementation() == target {
                    replacement.clone()
                } else {
                    Some(record.clone())
                }
            })
            .collect(),
    )
    .unwrap()
}
