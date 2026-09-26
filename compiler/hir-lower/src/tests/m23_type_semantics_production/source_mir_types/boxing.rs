use super::*;
use scoop_mir::{
    CanonicalMirCallableBindingsV1, MirBoxingCallableProductionError as Error,
    MirTypeBridgeTypeIndexV1, SingleConeStrongMirInput,
};

mod assertions;
mod rejections;

fn fixture(name: &str) -> (std::path::PathBuf, String) {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-mir-boxing-production");
    let source = std::fs::read_to_string(directory.join(format!("{name}.scoop"))).unwrap();
    (directory, source)
}

fn sources(
    output: &hir::DependencyHirOutput,
    hir: &hir::CrossConeTypeSemanticsProductionV1,
    input: &SingleConeStrongMirInput,
    graph: &scoop_identity::ValidatedIdentityGraph,
    types: &dyn scoop_mir::MirTypeBridgeTypeLookupV1,
) -> CanonicalMirCallableBindingsV1 {
    scoop_mir_lower::lower_source_callable_bindings(
        output,
        &public_interface(output),
        hir,
        input,
        graph,
        types,
        &[],
    )
    .unwrap()
}

#[test]
fn actual_boxing_callables_cover_value_members_defaults_and_diamonds() {
    for name in ["standalone", "combined"] {
        let (directory, source) = fixture(name);
        let (bytes, dump) = with_production(&source, |output, input, hir, graph, types| {
            let unit = dependencies::unit(input, graph);
            let index = MirTypeBridgeTypeIndexV1::try_new(&[types, &unit]).unwrap();
            let source = sources(output, hir, input, graph, &index);
            let bindings = CanonicalMirCallableBindingsV1::from_boxing_adjusts(
                input, types, graph, &index, &source,
            )
            .unwrap();
            assertions::actual(input, types, &source, &bindings);
            let restored: scoop_mir::DecodedCanonicalMirCallableBindingsV1 = decoded(&bindings);
            assert_eq!(
                restored
                    .validate(graph, input.foundation(), &index)
                    .unwrap(),
                bindings
            );
            let dump = assertions::dump(input, &bindings);
            if name == "standalone" {
                assert_eq!(bindings.entries().len(), 2);
                assert!(dump.contains("Managed -> Managed"));
                assert!(input.module().meta.boxing_adjusts.len() > bindings.entries().len());
            } else {
                assert!(dump.contains("Diamond.echo"));
                assert!(dump.contains("Root.$get$token"));
                assert!(dump.contains("Choice"));
                assert!(dump.contains("Payload"));
                assert!(!dump.contains("Singleton"));
            }
            (encode(&bindings).unwrap(), dump)
        });
        with_production(
            &format!("private struct Unrelated() {{}}\n{source}"),
            |output, input, hir, graph, types| {
                let unit = dependencies::unit(input, graph);
                let index = MirTypeBridgeTypeIndexV1::try_new(&[types, &unit]).unwrap();
                let source = sources(output, hir, input, graph, &index);
                let bindings = CanonicalMirCallableBindingsV1::from_boxing_adjusts(
                    input, types, graph, &index, &source,
                )
                .unwrap();
                assert_eq!(encode(&bindings).unwrap(), bytes);
            },
        );
        if let Some(path) = std::env::var_os("SCOOP_MIR_BOXING_SNAPSHOT_DIR") {
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
fn actual_boxing_callables_require_target_bindings_and_types() {
    let (_, source) = fixture("standalone");
    with_production(&source, |output, input, hir, graph, types| {
        let source = sources(output, hir, input, graph, types);
        rejections::check(input, graph, types, &source);
    });
    with_production("public struct Empty() {}", |_, input, _, graph, types| {
        let empty = CanonicalMirCallableBindingsV1::try_new(Vec::new()).unwrap();
        assert!(
            CanonicalMirCallableBindingsV1::from_boxing_adjusts(input, types, graph, types, &empty)
                .unwrap()
                .entries()
                .is_empty()
        );
    });
}
