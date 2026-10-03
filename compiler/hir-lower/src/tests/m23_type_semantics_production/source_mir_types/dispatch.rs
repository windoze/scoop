use super::*;
use scoop_mir::{
    CanonicalMirCallableBindingsV1, CanonicalMirDispatchSchemasV1, ConeMirInput,
    MirDispatchSchemaAuthority, MirTypeBridgeCallableIndexV1, MirTypeBridgeTypeIndexV1,
};
use scoop_mir_lower::{SourceMirDispatchProductionError as Error, lower_dispatch_schemas};

mod applications;
mod assertions;
mod rejections;

fn fixture(name: &str) -> (std::path::PathBuf, String) {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-mir-dispatch-production");
    let source = std::fs::read_to_string(directory.join(format!("{name}.scoop"))).unwrap();
    (directory, source)
}
fn with_dispatch<R>(
    source: &str,
    run: impl FnOnce(
        &hir::DependencyHirOutput,
        &ConeMirInput,
        &hir::CrossConeTypeSemanticsSectionV1,
        &CanonicalParamFreeMirTypeExportsV1,
        MirDispatchSchemaAuthority<'_>,
        &CanonicalMirDispatchSchemasV1,
    ) -> R,
) -> R {
    with_production(source, |output, input, hir, graph, sources| {
        let actual =
            scoop_mir_lower::lower_type_exports(output.output().local.module(), hir, input, graph)
                .unwrap();
        let types = CanonicalParamFreeMirTypeExportsV1::try_new(
            actual
                .records()
                .iter()
                .filter(|record| {
                    sources.get(record.exact()).is_some()
                        || matches!(
                            record.origin(),
                            scoop_mir::MirTypeOriginV1::NominalApplication(_)
                        )
                })
                .cloned()
                .collect(),
        )
        .unwrap();
        let unit = dependencies::unit(input, graph);
        let index = MirTypeBridgeTypeIndexV1::try_new(&[&actual, &unit]).unwrap();
        let callables = scoop_mir_lower::lower_source_callable_bindings(
            output,
            &public_interface(output),
            hir,
            input,
            graph,
            &index,
            &[],
        )
        .unwrap();
        let restored: scoop_mir::DecodedCanonicalMirCallableBindingsV1 = decoded(&callables);
        assert_eq!(
            restored
                .validate(graph, input.foundation(), &index)
                .unwrap(),
            callables
        );
        let boxing = CanonicalMirCallableBindingsV1::from_boxing_adjusts(
            input, &types, graph, &index, &callables,
        )
        .unwrap();
        let bindings = MirTypeBridgeCallableIndexV1::try_new(&[&callables, &boxing], &[]).unwrap();
        let authority = MirDispatchSchemaAuthority {
            identities: graph,
            types: &index,
            callables: &bindings,
        };
        let schemas = lower_dispatch_schemas(
            output.output().local.module(),
            hir,
            input,
            &types,
            authority,
            &[],
        )
        .unwrap_or_else(|error| {
            panic!(
                "{error:?}; source owners: {:?}",
                source_dispatch::owners(output)
            )
        });
        let restored: scoop_mir::DecodedCanonicalMirDispatchSchemasV1 = decoded(&schemas);
        assert_eq!(
            restored.validate(graph, &index, &bindings).unwrap(),
            schemas
        );
        let authority = MirDispatchSchemaAuthority {
            identities: graph,
            types: &index,
            callables: &bindings,
        };
        run(output, input, hir, &types, authority, &schemas)
    })
}

#[test]
fn actual_dispatch_schemas_cover_inheritance_defaults_boxes_and_abstract_slots() {
    for name in ["standalone", "combined"] {
        let (_, source) = fixture(name);
        let bytes = with_dispatch(&source, |_, input, hir, types, authority, schemas| {
            assertions::actual(input, hir, types, authority, schemas);
            encode(schemas).unwrap()
        });
        with_dispatch(
            &format!("private struct Unrelated() {{}}\n{source}"),
            |_, _, _, _, _, schemas| {
                assert_eq!(encode(schemas).unwrap(), bytes);
            },
        );
    }
}

#[test]
fn actual_dispatch_schemas_require_complete_inputs_and_shared_resources() {
    let (_, source) = fixture("standalone");
    with_dispatch(&source, |output, input, hir, types, authority, _| {
        rejections::check(output.output().local.module(), input, hir, types, authority);
    });
}

#[test]
fn actual_object_overrides_replace_base_slots_and_preserve_distinct_receivers() {
    let (_, source) = fixture("combined");
    with_dispatch(&source, |output, input, _, types, authority, schemas| {
        assertions::object_overrides(output, input, types, authority, schemas);
    });
}
