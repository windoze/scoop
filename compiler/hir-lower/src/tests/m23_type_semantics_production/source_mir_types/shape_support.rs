use super::*;
use scoop_mir::{CanonicalMirShapeSupportsV1, MirBoxedShapeSupportV1, MirShapeSupportError};

mod assertions;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn actual_mir_shape_families_replay_and_match_bound_materializations() {
    for name in ["value", "combined"] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-mir-shape-support");
        let source = std::fs::read_to_string(directory.join(format!("{name}.scoop"))).unwrap();
        let (bytes, projection) = with_production(&source, |output, input, hir, graph, _| {
            let types =
                scoop_mir_lower::lower_type_exports(hir, input, graph, &mut meter()).unwrap();
            let families =
                CanonicalMirShapeSupportsV1::from_strong_input(input, graph, &types, &mut meter())
                    .unwrap();
            assertions::bindings(input, &families);
            let restored: scoop_mir::DecodedCanonicalMirShapeSupportsV1 = decoded(&families);
            assert_eq!(
                restored
                    .validate(input.module().cone, graph, &types, &mut meter())
                    .unwrap(),
                families
            );
            let required = input
                .materialization()
                .shape_support()
                .iter()
                .map(|root| root.shape().source())
                .collect::<Vec<_>>();
            families
                .validate_required_sources(&required, &mut meter())
                .unwrap();
            if name == "combined" {
                assertions::hidden_box(input, &families);
            }
            (
                encode(&families).unwrap(),
                assertions::projection(output, &families, &types),
            )
        });
        with_production(
            &format!("private struct Unrelated() {{}}\n{source}"),
            |_, input, hir, graph, _| {
                let types =
                    scoop_mir_lower::lower_type_exports(hir, input, graph, &mut meter()).unwrap();
                let families = CanonicalMirShapeSupportsV1::from_strong_input(
                    input,
                    graph,
                    &types,
                    &mut meter(),
                )
                .unwrap();
                assert_eq!(encode(&families).unwrap(), bytes);
            },
        );
        assert_eq!(
            projection,
            std::fs::read_to_string(directory.join(format!("{name}.snap"))).unwrap()
        );
    }
}

#[test]
fn actual_mir_shape_family_production_requires_every_type_and_shared_budget() {
    with_production("public struct Empty() {}", |_, input, hir, graph, _| {
        let types = scoop_mir_lower::lower_type_exports(hir, input, graph, &mut meter()).unwrap();
        let produce = |types: &CanonicalParamFreeMirTypeExportsV1, meter: &mut BudgetMeter| {
            CanonicalMirShapeSupportsV1::from_strong_input(input, graph, types, meter)
        };
        for record in types.records() {
            let incomplete = CanonicalParamFreeMirTypeExportsV1::try_new(
                types
                    .records()
                    .iter()
                    .filter(|other| other.exact() != record.exact())
                    .cloned()
                    .collect(),
            )
            .unwrap();
            assert!(
                matches!(produce(&incomplete, &mut meter()), Err(MirShapeSupportError::MissingType { exact }) if exact == record.exact())
            );
        }
        let mut measured = meter();
        produce(&types, &mut measured).unwrap();
        let usage = measured.usage();
        assert!(usage.validation_work_units > 0 && usage.owned_bytes > 0);
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: usage.validation_work_units,
            ..DecodeLimits::default()
        });
        produce(&types, &mut shared).unwrap();
        assert!(matches!(
            produce(&types, &mut shared),
            Err(MirShapeSupportError::Resource(_))
        ));
        assert!(matches!(
            produce(
                &types,
                &mut BudgetMeter::new(DecodeLimits {
                    owned_bytes: 0,
                    ..DecodeLimits::default()
                })
            ),
            Err(MirShapeSupportError::Resource(_))
        ));
        let empty = PendingIdentityValidation::new().finish().unwrap();
        assert!(matches!(
            CanonicalMirShapeSupportsV1::from_strong_input(input, &empty, &types, &mut meter()),
            Err(MirShapeSupportError::Reference(_))
        ));
    });
    with_production(
        "public fun value(): Int = 1",
        |_, input, _, graph, types| {
            assert!(
                CanonicalMirShapeSupportsV1::from_strong_input(input, graph, types, &mut meter())
                    .unwrap()
                    .records()
                    .is_empty()
            );
        },
    );
}
