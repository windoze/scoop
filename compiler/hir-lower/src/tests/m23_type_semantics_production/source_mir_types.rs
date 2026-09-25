use super::*;
use scoop_identity::PendingIdentityValidation;
use scoop_mir::{CanonicalParamFreeMirTypeExportsV1, MirTypeOriginV1};
use scoop_wire::{WireDecode, WireEncode, decode_canonical, encode};

mod assertions;
mod boxing;
mod callables;
mod constructors;
mod dependencies;
mod dispatch;
mod equality;
mod exports;
mod finite;
mod identities;
mod interior_mutability;
mod objects;
mod shape_support;

fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

fn with_production<R>(
    source: &str,
    run: impl FnOnce(
        &hir::DependencyHirOutput,
        &scoop_mir::SingleConeStrongMirInput,
        &hir::CrossConeTypeSemanticsProductionV1,
        &mut scoop_identity::ValidatedIdentityGraph,
        &CanonicalParamFreeMirTypeExportsV1,
    ) -> R,
) -> R {
    source_dispatch::with_hir_source(source, |output, core| {
        let hir_types =
            produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
        let local = output.output().local.module();
        let mut selected = scoop_mir::SelectedExternalMirSet::empty(local.cone);
        if !local.initialization_units.is_empty() {
            selected = selected
                .with_initialization_cycle(core.project_initialization_cycle_to_mir())
                .unwrap();
        }
        let (module, selected) = scoop_mir_lower::lower_current_cone(output, selected)
            .unwrap()
            .into_parts();
        let foundation = scoop_mir::OdrFreeMirFoundation::from_module(&module).unwrap();
        let production = scoop_mir_lower::lower_production_section(
            module.cone,
            &hir::CoreBootstrapInterfaceSectionV1::from_export(&output.output().export).unwrap(),
            &foundation,
        )
        .unwrap();
        let shapes = output
            .output()
            .local
            .materialization()
            .roots()
            .iter()
            .map(|root| root.declaration().clone())
            .collect();
        let strong = scoop_mir::SingleConeStrongMirInput::try_new(
            module,
            foundation,
            production,
            shapes,
            scoop_mir::StrongExternalCallableInput::Selected(&selected),
        )
        .unwrap();
        let local_hir: hir::DecodedHirFoundation =
            decoded(&hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap());
        let mir: scoop_mir::DecodedMirFoundation = decoded(strong.foundation().as_canonical());
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(local.cone).unwrap();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        local_hir.register_identities(&mut pending).unwrap();
        mir.register_identities(&mut pending).unwrap();
        pending
            .register_external_graph_authorities(&source_inventory::core_identity_closure())
            .unwrap();
        local_hir.resolve_identities(&mut pending).unwrap();
        mir.resolve_identities(&mut pending).unwrap();
        let mut graph = pending.finish().unwrap();
        let table =
            scoop_mir_lower::lower_source_type_exports(&hir_types, &strong, &graph).unwrap();
        run(output, &strong, &hir_types, &mut graph, &table)
    })
}

#[test]
fn source_mir_types_cover_actual_source_representations_and_finite_helpers() {
    for name in ["values", "combined"] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-source-mir-types");
        let source = std::fs::read_to_string(directory.join(format!("{name}.scoop"))).unwrap();
        let (bytes, dump) = with_production(&source, |output, strong, hir_types, graph, table| {
            identities::source_members(output, table);
            let finite =
                CanonicalParamFreeMirTypeExportsV1::from_finite_shape_support(strong, table, graph)
                    .unwrap();
            let mut records = table.clone().into_records();
            records.extend(finite.into_records());
            let combined = CanonicalParamFreeMirTypeExportsV1::try_new(records).unwrap();
            assert_eq!(
                scoop_mir_lower::lower_type_exports(hir_types, strong, graph).unwrap(),
                combined
            );
            let restored: scoop_mir::DecodedCanonicalParamFreeMirTypeExportsV1 = decoded(&combined);
            assert_eq!(
                restored.validate(graph, strong.foundation()).unwrap(),
                combined
            );
            assert!(combined.records().len() > table.records().len());
            (encode(table).unwrap(), assertions::dump(output, table))
        });
        with_production(
            &format!("private struct Unrelated() {{}}\n{source}"),
            |_, _, _, _, table| {
                assert_eq!(encode(table).unwrap(), bytes);
            },
        );
        if let Some(path) = std::env::var_os("SCOOP_SOURCE_MIR_SNAPSHOT_DIR") {
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
fn source_mir_types_enforce_resources_and_complete_source_membership() {
    with_production(
        "public struct Empty() {}",
        |_, strong, source, graph, table| {
            assertions::rejections(strong, source, graph, table);
            source_dispatch::with_hir_source("public struct Different() {}", |output, _| {
                let other =
                    produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
                assert!(matches!(
                    scoop_mir_lower::lower_source_type_exports(&other, strong, graph),
                    Err(
                        scoop_mir_lower::SourceMirTypeProductionError::IncompleteSurface {
                            expected: 1,
                            actual: 0
                        }
                    )
                ));
            });
        },
    );
}
