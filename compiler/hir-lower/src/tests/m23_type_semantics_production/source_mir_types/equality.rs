use super::*;
use scoop_mir::{CanonicalMirCallableBindingsV1, ConeMirInput, MirTypeBridgeTypeIndexV1};
use scoop_mir_lower::{SourceMirEqualityProductionError as Error, lower_derived_equality_bindings};

mod assertions;
mod rejections;

fn fixture(name: &str) -> (std::path::PathBuf, String) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-mir-equality-production");
    let source = std::fs::read_to_string(path.join(format!("{name}.scoop"))).unwrap();
    (path, source)
}

#[test]
fn actual_derived_equality_bindings_cover_nested_values_enum_and_explicit_overloads() {
    for name in ["standalone", "combined"] {
        let (_, source) = fixture(name);
        let bytes = with_production(&source, |output, input, _, graph, types| {
            let boolean = dependencies::boolean(input, graph);
            let index = MirTypeBridgeTypeIndexV1::try_new(&[types, &boolean]).unwrap();
            let bindings =
                lower_derived_equality_bindings(output, input, types, graph, &index).unwrap();
            assertions::actual(input, &bindings);
            let restored: scoop_mir::DecodedCanonicalMirCallableBindingsV1 = decoded(&bindings);
            assert_eq!(
                restored
                    .validate(graph, input.foundation(), &index)
                    .unwrap(),
                bindings
            );
            let calls = assertions::calls(input, &bindings);
            let expected = if name == "standalone" {
                BTreeMap::from([("Token.equals", vec![]), ("Unrequested.equals", vec![])])
            } else {
                BTreeMap::from([
                    ("Leaf.equals", vec![]),
                    ("Pair.equals", vec!["Leaf.equals", "Leaf.equals"]),
                    ("Choice.equals", vec!["Pair.equals"]),
                    ("UsesManual.equals", vec!["Manual.equals"]),
                    ("Overloaded.equals", vec![]),
                ])
            };
            assert_eq!(calls, expected);
            encode(&bindings).unwrap()
        });
        with_production(
            &format!("private struct Unrelated() {{}}\n{source}"),
            |output, input, _, graph, types| {
                let boolean = dependencies::boolean(input, graph);
                let index = MirTypeBridgeTypeIndexV1::try_new(&[types, &boolean]).unwrap();
                let bindings =
                    lower_derived_equality_bindings(output, input, types, graph, &index).unwrap();
                assert_eq!(encode(&bindings).unwrap(), bytes);
            },
        );
    }
}

#[test]
fn actual_derived_equality_bindings_require_types_and_actual_roots() {
    let (_, source) = fixture("standalone");
    with_production(&source, |output, input, _, graph, types| {
        rejections::check(output, input, graph, types);
    });
}
