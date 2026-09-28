//! Mutate real delegate registrations after their normal publication and runtime checks.

use super::*;
use scoop_lir as lir;
use scoop_lir::{
    ConeProductionSectionValidationError::Registrations,
    RegistrationProductionTableV1::{InitializationUnit, StaticStorage},
    StrongRegistrationProductionValidationError::{SurfaceMismatch, TableLength},
};
use scoop_slib as slib;
use scoop_slib::SharedLirStrongProductionError::{InitializationRelation, Replay};
use scoop_wire::encode;

mod wire;

pub(super) fn check(
    target: &scoop_toolchain::ResolvedTargetProfile,
    artifacts: &[&SingleConeProductionSuccess],
    production: &lir::ConeProductionSectionV2,
) {
    let current = artifacts.last().unwrap();
    let current_id = current
        .artifact()
        .summary()
        .coordinate()
        .identity()
        .unwrap();
    let bytes = artifacts
        .iter()
        .map(|artifact| std::fs::read(artifact.artifact().path()).unwrap())
        .collect::<Vec<_>>();
    let mut direct = current
        .artifact()
        .summary()
        .direct_dependencies()
        .iter()
        .map(slib::DependencyRecord::identity)
        .collect::<Vec<_>>();
    direct.sort_unstable();
    let registration = production.registration_production();
    let units = registration.initialization_units().registrations();
    let unit = units.first().unwrap();
    let storages = registration.static_storages().registrations();
    let callables = registration.callables().registrations();
    let encoded = encode(production).unwrap();
    let mut changed_unit = encode(unit).unwrap();
    for (first, second, left, right) in [
        (
            6,
            7,
            encode(&unit.initializer().body()).unwrap(),
            encode(&unit.ensure().body()).unwrap(),
        ),
        (
            21,
            22,
            encode(&wire::CallableReference(unit.initializer())).unwrap(),
            encode(&wire::CallableReference(unit.ensure())).unwrap(),
        ),
    ] {
        changed_unit = wire::replace_once(
            &changed_unit,
            &[&[first], left.as_slice(), &[second], right.as_slice()].concat(),
            &[&[first], right.as_slice(), &[second], left.as_slice()].concat(),
        );
    }
    let mutations = [
        (
            "swapped initializer and ensure",
            wire::replace_once(&encoded, &encode(unit).unwrap(), &changed_unit),
        ),
        ("missing unit", wire::remove_plan(&encoded, units, 0)),
        (
            "missing delegate storage",
            wire::remove_plan(
                &encoded,
                storages,
                storages
                    .iter()
                    .position(|storage| storage.semantic().storage() == unit.storage().storage())
                    .unwrap(),
            ),
        ),
        (
            "missing failure root",
            wire::remove_plan(
                &encoded,
                storages,
                storages
                    .iter()
                    .position(|storage| {
                        storage.semantic().storage() == unit.failure_root().storage()
                    })
                    .unwrap(),
            ),
        ),
        (
            "missing initializer callable",
            wire::remove_plan(
                &encoded,
                callables,
                callables
                    .iter()
                    .position(|callable| callable.body() == unit.initializer().body())
                    .unwrap(),
            ),
        ),
    ];
    for (case, replacement) in mutations {
        let changed = wire::replace_production(target, bytes.last().unwrap(), replacement);
        let error = slib::read_cross_cone_layout_artifact_closure(
            slib::CrossConeArtifactClosureInput::completed(
                current_id,
                target.lir_target_selection(),
                direct.clone(),
                bytes[..bytes.len() - 1].iter().map(Vec::as_slice).collect(),
                &changed,
            ),
            target.c_bridge_toolchain().profile(),
        )
        .err()
        .unwrap_or_else(|| panic!("{case} must be rejected"));
        let slib::CrossConeLayoutArtifactValidationError::Semantic { source } = error else {
            panic!("{case}: expected semantic rejection, found {error:?}");
        };
        let slib::CrossConeLayoutSemanticClosureError::LirStrongProduction(error) = *source else {
            panic!("{case}: expected production rejection, found {source:?}");
        };
        assert_eq!(error.provider, current_id, "{case}");
        let expected = match (case, error.source.as_ref()) {
            (
                "swapped initializer and ensure",
                InitializationRelation {
                    unit: actual,
                    field: "initializer_role",
                },
            ) => *actual == unit.semantic().unit(),
            (
                "missing unit",
                Replay(Registrations(TableLength {
                    table: InitializationUnit,
                    expected,
                    actual,
                })),
            ) => *expected == units.len() && *actual + 1 == *expected,
            (
                "missing delegate storage" | "missing failure root",
                Replay(Registrations(TableLength {
                    table: StaticStorage,
                    expected,
                    actual,
                })),
            ) => *expected == storages.len() && *actual + 1 == *expected,
            ("missing initializer callable", Replay(Registrations(SurfaceMismatch))) => true,
            _ => false,
        };
        assert!(expected, "{case}: unexpected rejection: {error:?}");
    }
}
