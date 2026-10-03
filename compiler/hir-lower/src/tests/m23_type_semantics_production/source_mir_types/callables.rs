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
        let bytes = with_production(&source, |output, input, hir, graph, _| {
            let unit = dependencies::unit(input, graph);
            let actual = complete_type_exports(output, input, hir, graph);
            let index = MirTypeBridgeTypeIndexV1::try_new(&[&actual, &unit]).unwrap();
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
            if name == "combined" {
                assertions::combined(input, &bindings);
                rejections::traps(output, input, graph, &actual, &unit, &bindings);
            }
            encode(&bindings).unwrap()
        });
        with_production(
            &format!("private fun unrelated(value: Token): Token = value\n{source}"),
            |output, input, hir, graph, _| {
                let unit = dependencies::unit(input, graph);
                let actual = complete_type_exports(output, input, hir, graph);
                let index = MirTypeBridgeTypeIndexV1::try_new(&[&actual, &unit]).unwrap();
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
        |output, input, hir, graph, _| {
            let actual = complete_type_exports(output, input, hir, graph);
            let unit = dependencies::unit(input, graph);
            let index = MirTypeBridgeTypeIndexV1::try_new(&[&actual, &unit]).unwrap();
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
            assert!(!bindings.entries().iter().any(assertions::is_source));
        },
    );
}

#[test]
fn closed_generic_no_gc_signatures_use_actual_application_type_exports() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-mir-callable-production/generic-storage.scoop"
    ));
    with_production(source, |output, input, hir, graph, _| {
        use scoop_mir::{MirGcKindV1, MirTypeBridgeTypeLookupV1};
        let types =
            scoop_mir_lower::lower_type_exports(output.output().local.module(), hir, input, graph)
                .unwrap();
        let unit = dependencies::unit(input, graph);
        let index = MirTypeBridgeTypeIndexV1::try_new(&[&types, &unit]).unwrap();
        let public = public_interface(output);
        let bindings =
            lower_source_callable_bindings(output, &public, hir, input, graph, &index, &[])
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
        assert!(matches!(
            index.get(exact).unwrap().origin(),
            scoop_mir::MirTypeOriginV1::NominalApplication(_)
        ));
        assert_eq!(index.gc_kind(exact), Some(MirGcKindV1::GcFree));
        let raw: scoop_mir::DecodedCanonicalMirCallableBindingsV1 = decoded(&bindings);
        assert_eq!(
            raw.validate(graph, input.foundation(), &index).unwrap(),
            bindings
        );

        let invalid = types
            .records()
            .iter()
            .map(|record| {
                if record.exact() != exact {
                    return record.clone();
                }
                scoop_mir::ParamFreeMirTypeExportV1::try_new(
                    scoop_mir::MirTypeBridgeAuthority {
                        identities: graph,
                        foundation: input.foundation(),
                    },
                    exact,
                    record.origin().clone(),
                    scoop_mir::MirTypeFactsV1::try_new(
                        record.facts().kind(),
                        MirGcKindV1::ContainsManagedReferences,
                    )
                    .unwrap(),
                    record.representation().clone(),
                    record.base_and_interfaces().clone(),
                )
                .unwrap()
            })
            .collect();
        let invalid = CanonicalParamFreeMirTypeExportsV1::try_new(invalid).unwrap();
        let index = MirTypeBridgeTypeIndexV1::try_new(&[&invalid, &unit]).unwrap();
        let raw: scoop_mir::DecodedCanonicalMirCallableBindingsV1 = decoded(&bindings);
        assert!(matches!(raw.validate(graph, input.foundation(), &index),
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
