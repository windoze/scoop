use super::*;
use scoop_mir::{CanonicalMirShapeSupportsV1, MirBoxedShapeSupportV1};

mod assertions;

#[test]
fn actual_mir_shape_families_replay_and_match_bound_materializations() {
    for name in ["value", "combined"] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-mir-shape-support");
        let source = std::fs::read_to_string(directory.join(format!("{name}.scoop"))).unwrap();
        let (bytes, projection) = with_production(&source, |output, input, hir, graph, _| {
            let types = scoop_mir_lower::lower_type_exports(hir, input, graph).unwrap();
            let families =
                CanonicalMirShapeSupportsV1::from_strong_input(input, graph, &types).unwrap();
            assertions::bindings(input, &families);
            let restored: scoop_mir::DecodedCanonicalMirShapeSupportsV1 = decoded(&families);
            assert_eq!(
                restored
                    .validate(input.module().cone, graph, &types)
                    .unwrap(),
                families
            );
            let required = input
                .materialization()
                .shape_support()
                .iter()
                .map(|root| root.shape().source())
                .collect::<Vec<_>>();
            families.validate_required_sources(&required).unwrap();
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
                let types = scoop_mir_lower::lower_type_exports(hir, input, graph).unwrap();
                let families =
                    CanonicalMirShapeSupportsV1::from_strong_input(input, graph, &types).unwrap();
                assert_eq!(encode(&families).unwrap(), bytes);
            },
        );
        assert_eq!(
            projection,
            std::fs::read_to_string(directory.join(format!("{name}.snap"))).unwrap()
        );
    }
}
