use super::*;
use scoop_identity::{GeneratedNominalKey, PendingIdentityValidation};
use scoop_mir::{CanonicalParamFreeMirTypeExportsV1, MirTypeOriginV1, MirTypeRepresentationV1};
use scoop_wire::{WireDecode, WireEncode, decode_canonical, encode};

mod assertions;
mod rejections;

fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

fn with_production<R>(
    source: &str,
    run: impl FnOnce(
        &scoop_mir::ConeMirInput,
        &mut scoop_identity::ValidatedIdentityGraph,
        &CanonicalParamFreeMirTypeExportsV1,
        &CanonicalParamFreeMirTypeExportsV1,
    ) -> R,
) -> R {
    let sources = sources::core_sources_with(&[("src/finite-types.scoop", source)]);
    let world =
        scoop_hir::ImportedSemanticWorld::from_dependencies(sources.cone(), Vec::new(), Vec::new())
            .unwrap();
    let hir = crate::request::preflight::current_hir::CurrentConeHirArtifacts::lower(
        scoop_identity::RequestedConeKind::Library,
        &sources,
        scoop_hir_lower::CoreProtocolInput::CurrentDeclarations,
        &world,
    )
    .unwrap();
    let input = hir.machine_input();
    let source_foundation =
        scoop_hir::CanonicalHirFoundation::from_type_semantics_output(input.output).unwrap();
    let hir_foundation: scoop_hir::DecodedHirFoundation = decoded(&source_foundation);
    let mir = input
        .lower_selected_mir(scoop_mir::SelectedExternalMirSet::empty(ConeIdentity::CORE))
        .unwrap();
    let mir_foundation: scoop_mir::DecodedMirFoundation = decoded(mir.strong.foundation());
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    hir_foundation.register_identities(&mut pending).unwrap();
    mir_foundation.register_identities(&mut pending).unwrap();
    hir_foundation.resolve_identities(&mut pending).unwrap();
    mir_foundation.resolve_identities(&mut pending).unwrap();
    let mut graph = pending.finish().unwrap();
    let input = hir.machine_input();
    let source = scoop_hir_lower::produce_cross_cone_type_semantics(
        input.output,
        scoop_hir::SharedTypeMetadataV1 {
            provider: input.output.output().export.cone,
            identities: &graph,
            foundation: &source_foundation,
            public: input.public,
        },
        &[],
        &[],
    )
    .unwrap();
    let sources = scoop_mir_lower::lower_source_type_exports(&source, &mir.strong, &graph).unwrap();
    let records = CanonicalParamFreeMirTypeExportsV1::from_finite_shape_support(
        &mir.strong,
        &sources,
        &graph,
    )
    .unwrap();
    run(&mir.strong, &mut graph, &records, &sources)
}

#[test]
fn finite_mir_types_are_produced_from_real_materializations_and_survive_bytes() {
    for (name, count) in [("values", 9), ("combined", 10)] {
        let directory = crate::workspace_root().join("tests/fixtures/m23-finite-mir-types");
        let source = std::fs::read_to_string(directory.join(format!("{name}.scoop"))).unwrap();
        let (bytes, projection) = with_production(&source, |input, graph, records, _| {
            assertions::materializations(input, records);
            if name == "combined" {
                assertions::hidden_box(input, records);
            }
            let restored: scoop_mir::DecodedCanonicalParamFreeMirTypeExportsV1 = decoded(records);
            assert_eq!(
                restored.validate(graph, input.foundation()).unwrap(),
                *records
            );
            let projection = assertions::projection(input, records, count);
            (encode(records).unwrap(), projection)
        });
        with_production(
            &format!("private struct Unrelated() {{}}\n{source}"),
            |_, _, records, _| {
                assert_eq!(encode(records).unwrap(), bytes);
            },
        );
        if let Some(output) = std::env::var_os("SCOOP_FINITE_MIR_SNAPSHOT_DIR") {
            std::fs::create_dir_all(&output).unwrap();
            std::fs::write(
                std::path::Path::new(&output).join(format!("{name}.snap")),
                projection,
            )
            .unwrap();
        } else {
            assert_eq!(
                projection,
                std::fs::read_to_string(directory.join(format!("{name}.snap"))).unwrap()
            );
        }
    }
}
