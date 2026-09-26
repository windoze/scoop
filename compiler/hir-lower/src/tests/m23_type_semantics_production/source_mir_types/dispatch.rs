use super::*;
use scoop_mir::{
    CanonicalMirCallableBindingsV1, CanonicalMirDispatchSchemasV1, MirDispatchSchemaAuthority,
    MirTypeBridgeCallableIndexV1, MirTypeBridgeTypeIndexV1, SingleConeStrongMirInput,
};
use scoop_mir_lower::{SourceMirDispatchProductionError as Error, lower_dispatch_schemas};

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
        &SingleConeStrongMirInput,
        &hir::CrossConeTypeSemanticsProductionV1,
        &CanonicalParamFreeMirTypeExportsV1,
        MirDispatchSchemaAuthority<'_>,
        &CanonicalMirDispatchSchemasV1,
    ) -> R,
) -> R {
    with_production(source, |output, input, hir, graph, types| {
        let unit = dependencies::unit(input, graph);
        let index = MirTypeBridgeTypeIndexV1::try_new(&[types, &unit]).unwrap();
        let callables = scoop_mir_lower::lower_source_callable_bindings(
            output,
            &public_interface(output),
            hir,
            input,
            graph,
            &index,
        )
        .unwrap();
        let boxing = CanonicalMirCallableBindingsV1::from_boxing_adjusts(
            input, types, graph, &index, &callables,
        )
        .unwrap();
        let bindings = MirTypeBridgeCallableIndexV1::try_new(&[&callables, &boxing], &[]).unwrap();
        let authority = MirDispatchSchemaAuthority {
            identities: graph,
            types: &index,
            callables: &bindings,
        };
        let schemas =
            lower_dispatch_schemas(hir, input, types, authority, &[]).unwrap_or_else(|error| {
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
        run(output, input, hir, types, authority, &schemas)
    })
}

#[test]
fn actual_dispatch_schemas_cover_inheritance_defaults_boxes_and_abstract_slots() {
    for name in ["standalone", "combined"] {
        let (directory, source) = fixture(name);
        let (bytes, dump) =
            with_dispatch(&source, |output, input, hir, types, authority, schemas| {
                assertions::actual(input, hir, types, authority, schemas);
                (
                    encode(schemas).unwrap(),
                    assertions::dump(output, input, types, schemas),
                )
            });
        with_dispatch(
            &format!("private struct Unrelated() {{}}\n{source}"),
            |_, _, _, _, _, schemas| {
                assert_eq!(encode(schemas).unwrap(), bytes);
            },
        );
        if let Some(path) = std::env::var_os("SCOOP_MIR_DISPATCH_SNAPSHOT_DIR") {
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
fn actual_dispatch_schemas_require_complete_inputs_and_shared_resources() {
    let (_, source) = fixture("standalone");
    with_dispatch(&source, |_, input, hir, types, authority, _| {
        rejections::check(input, hir, types, authority);
    });
}

#[test]
fn actual_object_overrides_replace_base_slots_and_preserve_distinct_receivers() {
    let (_, source) = fixture("combined");
    with_dispatch(&source, |output, input, _, types, authority, schemas| {
        assertions::object_overrides(output, input, types, authority, schemas);
    });
}
