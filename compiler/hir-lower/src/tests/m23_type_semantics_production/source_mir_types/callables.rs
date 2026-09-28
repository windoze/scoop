use super::*;
use scoop_mir::{CanonicalMirCallableBindingsV1, MirTypeBridgeTypeIndexV1};
use scoop_mir_lower::{SourceMirCallableProductionError as Error, lower_source_callable_bindings};

mod assertions;
mod rejections;

#[test]
fn actual_source_callables_cover_functions_members_accessors_and_traps() {
    for name in ["standalone", "combined"] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-mir-callable-production");
        let source = std::fs::read_to_string(directory.join(format!("{name}.scoop"))).unwrap();
        let (bytes, dump) = with_production(&source, |output, input, hir, graph, types| {
            let unit = dependencies::unit(input, graph);
            let index = MirTypeBridgeTypeIndexV1::try_new(&[types, &unit]).unwrap();
            let public = public_interface(output);
            let bindings =
                lower_source_callable_bindings(output, &public, hir, input, graph, &index, &[])
                    .unwrap();
            assertions::actual(output, input, &bindings);
            let decoded: scoop_mir::DecodedCanonicalMirCallableBindingsV1 = decoded(&bindings);
            assert_eq!(
                decoded.validate(graph, input.foundation(), &index).unwrap(),
                bindings
            );
            let dump = assertions::dump(input, &bindings);
            if name == "combined" {
                assertions::combined(input, &bindings, &dump);
                rejections::traps(output, input, graph, types, &unit, &bindings);
            }
            (encode(&bindings).unwrap(), dump)
        });
        with_production(
            &format!("private fun unrelated(value: Token): Token = value\n{source}"),
            |output, input, hir, graph, types| {
                let unit = dependencies::unit(input, graph);
                let index = MirTypeBridgeTypeIndexV1::try_new(&[types, &unit]).unwrap();
                let bindings = lower_source_callable_bindings(
                    output,
                    &public_interface(output),
                    hir,
                    input,
                    graph,
                    &index,
                    &[],
                )
                .unwrap();
                assert_eq!(encode(&bindings).unwrap(), bytes);
            },
        );
        if let Some(path) = std::env::var_os("SCOOP_MIR_CALLABLE_SNAPSHOT_DIR") {
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
fn actual_source_callables_reject_missing_inputs() {
    with_production(
        "public struct Token() {}\n@NoGC public fun pass(value: Token): Token = value",
        |output, input, hir, graph, types| {
            rejections::check(output, hir, input, graph, types);
        },
    );
    with_production(
        "public interface Empty {}",
        |output, input, hir, graph, types| {
            assert!(
                lower_source_callable_bindings(
                    output,
                    &public_interface(output),
                    hir,
                    input,
                    graph,
                    types,
                    &[]
                )
                .unwrap()
                .entries()
                .is_empty()
            );
        },
    );
}

#[test]
fn closed_generic_no_gc_signatures_reuse_exact_facts_without_strong_type_exports() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-mir-callable-production/generic-storage.scoop"
    ));
    with_production(source, |output, input, hir, graph, types| {
        use scoop_mir::{MirGcKindV1, MirTypeBridgeTypeLookupV1};
        let unit = dependencies::unit(input, graph);
        let index = MirTypeBridgeTypeIndexV1::try_new(&[types, &unit]).unwrap();
        let facts = |exact| {
            hir.exact_facts().get(exact).map(|fact| match fact.gc() {
                hir::ExactTypeGcV1::GcFree => MirGcKindV1::GcFree,
                hir::ExactTypeGcV1::ContainsManagedReferences => {
                    MirGcKindV1::ContainsManagedReferences
                }
            })
        };
        let lookup = index.with_gc_facts(&facts);
        let public = public_interface(output);
        let bindings =
            lower_source_callable_bindings(output, &public, hir, input, graph, &lookup, &[])
                .unwrap();
        let signature = bindings
            .entries()
            .iter()
            .find(|binding| binding.semantic_signature().gc_effect() == scoop_mir::GcEffect::NoGc)
            .unwrap()
            .semantic_signature()
            .exact();
        let exact = signature.parameters()[0];
        assert_eq!(signature.result(), exact);
        assert!(index.get(exact).is_none());
        assert_eq!(lookup.gc_kind(exact), Some(MirGcKindV1::GcFree));
        let raw: scoop_mir::DecodedCanonicalMirCallableBindingsV1 = decoded(&bindings);
        assert_eq!(
            raw.validate(graph, input.foundation(), &lookup).unwrap(),
            bindings
        );

        let invalid = |id| (id == exact).then_some(MirGcKindV1::ContainsManagedReferences);
        let lookup = index.with_gc_facts(&invalid);
        let raw: scoop_mir::DecodedCanonicalMirCallableBindingsV1 = decoded(&bindings);
        assert!(matches!(raw.validate(graph, input.foundation(), &lookup),
            Err(scoop_mir::MirCallableBridgeError::NoGcContainsReferences { exact: found })
                if found == exact));
    });
}

#[test]
fn actual_source_callables_reject_changed_source_result_and_gc_effect() {
    let source = "public struct Token() {}\npublic struct Other() {}\n@NoGC public fun pass(value: Token): Token = value";
    with_production(source, |output, _, hir, _, _| {
        let public = public_interface(output);
        with_production(
            &source.replace("@NoGC ", ""),
            |_, input, _, graph, types| {
                assert!(matches!(
                    lower_source_callable_bindings(output, &public, hir, input, graph, types, &[]),
                    Err(Error::Bridge(
                        scoop_mir::MirCallableBridgeError::SignatureMismatch
                    ))
                ));
            },
        );
        with_production(
            &source.replace(": Token = value", ": Other = Other()"),
            |_, input, _, graph, types| {
                assert!(matches!(
                    lower_source_callable_bindings(output, &public, hir, input, graph, types, &[]),
                    Err(Error::SourceSignatureMismatch(_))
                ));
            },
        );
    });
}
