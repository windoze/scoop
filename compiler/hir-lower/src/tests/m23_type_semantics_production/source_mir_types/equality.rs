use super::*;
use scoop_mir::{
    CanonicalMirCallableBindingsV1, MirTypeBridgeTypeIndexV1, SingleConeStrongMirInput,
};
use scoop_mir_lower::{SourceMirEqualityProductionError as Error, lower_derived_equality_bindings};

mod assertions;
mod rejections;
mod support;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

fn fixture(name: &str) -> (std::path::PathBuf, String) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-mir-equality-production");
    let source = std::fs::read_to_string(path.join(format!("{name}.scoop"))).unwrap();
    (path, source)
}

#[test]
fn actual_derived_equality_bindings_cover_nested_values_enum_and_explicit_overloads() {
    for name in ["standalone", "combined"] {
        let (directory, source) = fixture(name);
        let (bytes, dump) = with_production(&source, |output, input, _, graph, types| {
            let boolean = support::boolean(input, graph);
            let index =
                MirTypeBridgeTypeIndexV1::try_new(&[types, &boolean], &mut meter()).unwrap();
            let bindings =
                lower_derived_equality_bindings(output, input, types, graph, &index, &mut meter())
                    .unwrap();
            assertions::actual(input, &bindings);
            let restored: scoop_mir::DecodedCanonicalMirCallableBindingsV1 = decoded(&bindings);
            assert_eq!(
                restored
                    .validate(graph, input.foundation(), &index, &mut meter())
                    .unwrap(),
                bindings
            );
            let dump = assertions::dump(input, &bindings);
            if name == "standalone" {
                assert_eq!(bindings.entries().len(), 1);
                assert!(dump.contains("Token.equals"));
                assert!(!dump.contains("Hidden") && !dump.contains("Unrequested"));
            } else {
                assert_eq!(bindings.entries().len(), 5);
                for member in [
                    "Leaf.equals",
                    "Pair.equals",
                    "Choice.equals",
                    "UsesManual.equals",
                    "Overloaded.equals",
                ] {
                    assert!(dump.contains(member), "{dump}");
                }
                assert!(!dump.lines().any(|line| line.starts_with("Manual.equals")));
                assert!(!dump.contains("NotComparable"));
            }
            (encode(&bindings).unwrap(), dump)
        });
        with_production(
            &format!("private struct Unrelated() {{}}\n{source}"),
            |output, input, _, graph, types| {
                let boolean = support::boolean(input, graph);
                let index =
                    MirTypeBridgeTypeIndexV1::try_new(&[types, &boolean], &mut meter()).unwrap();
                let bindings = lower_derived_equality_bindings(
                    output,
                    input,
                    types,
                    graph,
                    &index,
                    &mut meter(),
                )
                .unwrap();
                assert_eq!(encode(&bindings).unwrap(), bytes);
            },
        );
        if let Some(path) = std::env::var_os("SCOOP_MIR_EQUALITY_SNAPSHOT_DIR") {
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(
                std::path::Path::new(&path).join(format!("{name}.snap")),
                dump,
            )
            .unwrap();
        } else {
            assert_eq!(
                dump,
                std::fs::read_to_string(directory.join(format!("{name}.snap"))).unwrap()
            );
        }
    }
}

#[test]
fn actual_derived_equality_bindings_require_types_actual_roots_and_shared_budget() {
    let (_, source) = fixture("standalone");
    with_production(&source, |output, input, _, graph, types| {
        rejections::check(output, input, graph, types);
    });
}
