use scoop_hir::{CanonicalHirDependencyCallSitesV1, HirDependencyCallSiteV1};
use scoop_identity::{Effect, ExactCallableSignature};
use scoop_wire::{BudgetMeter, DecodeLimits};

use super::*;

mod selected;
mod support;
use support::Fixture;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn every_actual_call_requires_its_own_strong_mir_root() {
    let fixture = Fixture::new();
    let interface = fixture.interface(vec![fixture.site(0, vec![])]);
    validate_executable_hir_calls(&interface, &fixture.strong, &fixture.bridge, &mut meter())
        .unwrap();
    let absent = StrongCallableBridgeSurfaceV1::try_new(vec![]).unwrap();
    assert!(matches!(
        validate_executable_hir_calls(&interface, &absent, &fixture.bridge, &mut meter()),
        Err(CrossConeMirClosureRelationError::CallRoot { .. })
    ));
}

#[test]
fn later_calls_to_one_target_cannot_hide_incompatible_logical_signatures() {
    let fixture = Fixture::new();
    let sites = vec![fixture.site(0, vec![]), fixture.site(1, vec![fixture.unit])];
    assert!(
        matches!(validate_executable_hir_calls(&fixture.interface(sites), &fixture.strong, &fixture.bridge, &mut meter()), Err(CrossConeMirClosureRelationError::CallSignature { position, .. }) if position.expression_index == 1)
    );
}

#[test]
fn logical_receiver_and_each_unit_argument_survive_the_mir_join() {
    let mut fixture = Fixture::new();
    fixture.set_signature(ExactCallableSignature::new(
        Effect::Ordinary,
        Some(fixture.unit),
        vec![fixture.unit, fixture.unit],
        fixture.unit,
    ));
    let interface = fixture.interface(vec![fixture.site(0, vec![fixture.unit; 3])]);
    validate_executable_hir_calls(&interface, &fixture.strong, &fixture.bridge, &mut meter())
        .unwrap();
    let erased = fixture.interface(vec![fixture.site(0, vec![fixture.unit; 2])]);
    assert!(matches!(
        validate_executable_hir_calls(&erased, &fixture.strong, &fixture.bridge, &mut meter()),
        Err(CrossConeMirClosureRelationError::CallSignature { .. })
    ));
}

#[test]
fn mir_call_join_keeps_the_artifacts_existing_budget() {
    let fixture = Fixture::new();
    let interface = fixture.interface(vec![fixture.site(0, vec![])]);
    let mut measured = meter();
    validate_executable_hir_calls(&interface, &fixture.strong, &fixture.bridge, &mut measured)
        .unwrap();
    let mut shared = BudgetMeter::new(DecodeLimits {
        validation_work_units: measured.usage().validation_work_units,
        ..DecodeLimits::default()
    });
    validate_executable_hir_calls(&interface, &fixture.strong, &fixture.bridge, &mut shared)
        .unwrap();
    assert!(matches!(
        validate_executable_hir_calls(&interface, &fixture.strong, &fixture.bridge, &mut shared),
        Err(CrossConeMirClosureRelationError::Resource(_))
    ));
}
