use super::*;
use scoop_mir::{CanonicalMirCallableBindingsV1, MirTypeBridgeTypeIndexV1};
use scoop_mir_lower::{SourceMirConstructorProductionError as Error, lower_constructor_bindings};

mod assertions;
mod rejections;

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

#[test]
fn actual_constructor_bindings_preserve_value_and_class_signatures() {
    for name in ["standalone", "combined"] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-mir-constructor-production");
        let source = std::fs::read_to_string(directory.join(format!("{name}.scoop"))).unwrap();
        let (bytes, dump) = with_production(&source, |output, input, hir, graph, types| {
            let unit = dependencies::unit(input, graph);
            let index = MirTypeBridgeTypeIndexV1::try_new(&[types, &unit], &mut meter()).unwrap();
            let bindings =
                lower_constructor_bindings(output, hir, input, graph, &index, &mut meter())
                    .unwrap();
            assertions::actual(output, hir, input, &bindings);
            rejections::primary_effects(input, graph, &index, &bindings);
            let restored: scoop_mir::DecodedCanonicalMirCallableBindingsV1 = decoded(&bindings);
            assert_eq!(
                restored
                    .validate(graph, input.foundation(), &index, &mut meter())
                    .unwrap(),
                bindings
            );
            (
                encode(&bindings).unwrap(),
                assertions::dump(output, hir, &bindings),
            )
        });
        with_production(
            &format!("private class Unrelated() {{}}\n{source}"),
            |output, input, hir, graph, types| {
                let unit = dependencies::unit(input, graph);
                let index =
                    MirTypeBridgeTypeIndexV1::try_new(&[types, &unit], &mut meter()).unwrap();
                let bindings =
                    lower_constructor_bindings(output, hir, input, graph, &index, &mut meter())
                        .unwrap();
                assert_eq!(encode(&bindings).unwrap(), bytes);
            },
        );
        if let Some(path) = std::env::var_os("SCOOP_MIR_CONSTRUCTOR_SNAPSHOT_DIR") {
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
fn actual_constructor_bindings_reject_missing_inputs_and_exhausted_budgets() {
    with_production(
        "public class Container public constructor()",
        |output, input, hir, graph, types| {
            let unit = dependencies::unit(input, graph);
            rejections::check(output, hir, input, graph, types, &unit);
        },
    );
    with_production(
        "public interface Empty {}",
        |output, input, hir, graph, types| {
            assert!(
                lower_constructor_bindings(output, hir, input, graph, types, &mut meter())
                    .unwrap()
                    .entries()
                    .is_empty()
            );
        },
    );
}

#[test]
fn actual_constructor_bindings_reject_source_and_body_gc_disagreement() {
    let source = "public struct Empty() { @NoGC public constructor(value: Empty): this() {} }";
    with_production(source, |output, input, hir, graph, types| {
        let bindings =
            lower_constructor_bindings(output, hir, input, graph, types, &mut meter()).unwrap();
        rejections::secondary_is_not_field_assembly(input, graph, types, &bindings);
        with_production(
            &source.replace("@NoGC ", ""),
            |_, input, _, graph, types| {
                assert!(matches!(
                    lower_constructor_bindings(output, hir, input, graph, types, &mut meter()),
                    Err(Error::Bridge(
                        scoop_mir::MirCallableBridgeError::SignatureMismatch
                    ))
                ));
            },
        );
    });
}
