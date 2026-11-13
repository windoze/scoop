//! The same exported graph must have the same consumer after wire round-trip.

use super::*;
use scoop_identity::{ConeIdentity, PendingIdentityValidation};
use scoop_wire::{decode_canonical, encode};

#[test]
fn exported_graphs_have_identical_direct_and_wire_consumers() {
    for (fixture, case) in [
        ("language/generic-body-consumption", "consumer"),
        ("language/shared-constructor-requests", "both-imported"),
        ("language/imported-options", "construct"),
        ("language/imported-pointers", "defaults"),
    ] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(fixture);
        let provider = std::fs::read_to_string(directory.join("provider.scoop")).unwrap();
        let consumer = std::fs::read_to_string(directory.join(format!("{case}.scoop"))).unwrap();
        compare_consumers(fixture, &provider, &consumer);
    }
}

pub(super) fn compare_consumers(label: &str, provider: &str, consumer: &str) {
    compare_consumers_export_ir(
        label,
        provider,
        consumer,
        hir::IrTemplateExport::Default,
        true,
    )
}

pub(super) fn compare_consumers_export_ir(
    label: &str,
    provider: &str,
    consumer: &str,
    mode: hir::IrTemplateExport,
    include_mir: bool,
) {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "generic-provider", "1.0.0").unwrap();
    let parsed_provider = super::super::m23_ordinary_core_only::support::parsed_ordinary_text_at(
        &coordinate,
        provider,
    );
    let (foundation, interface) =
        super::super::m23_ordinary_dependencies::support::project_parsed_dependency_with_export_ir(
            &core,
            &parsed_provider,
            &["Boolean"],
            false,
            mode,
        );
    let mut indexed = interface.clone();
    let bytes = encode(&indexed.index_for_wire().unwrap()).unwrap();
    let decoded_interface: hir::DecodedCrossConeHirInterfaceSectionV1 =
        decode_canonical(&bytes).unwrap();

    let mut graphs = Vec::new();
    let mut decoded_provider = None;
    for (origin, canonical) in [
        (ConeIdentity::CORE, core.source_foundation.as_ref()),
        (coordinate.identity().unwrap(), &foundation),
    ] {
        let decoded: hir::DecodedHirFoundation =
            decode_canonical(&encode(canonical).unwrap()).unwrap();
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        if origin != ConeIdentity::CORE {
            pending.register_authority(origin).unwrap();
        }
        decoded.register_identities(&mut pending).unwrap();
        for graph in &graphs {
            pending.register_external_graph_authorities(graph).unwrap();
        }
        decoded.resolve_identities(&mut pending).unwrap();
        graphs.push(pending.finish().unwrap());
        if origin != ConeIdentity::CORE {
            decoded_provider = Some(decoded);
        }
    }
    let mut identities = graphs.pop().unwrap();
    let restored_foundation = decoded_provider
        .unwrap()
        .validate_with_dependency_sources(
            &coordinate,
            &mut identities,
            &[core.source_foundation.as_ref()],
        )
        .unwrap()
        .into_canonical();
    let restored_interface = decoded_interface.resolve(&mut identities).unwrap();
    assert_eq!(
        encode(&foundation).unwrap(),
        encode(&restored_foundation).unwrap(),
        "{label}: foundation"
    );
    assert_eq!(
        interface, restored_interface,
        "{label}: declarations and bodies"
    );

    let source = parsed_ordinary_text(consumer);
    let mut results = Vec::new();
    for (foundation, interface) in [
        (&foundation, &interface),
        (&restored_foundation, &restored_interface),
    ] {
        let imported = core.import_dependency_foundation(&coordinate, foundation, 73);
        let aliases = alias_expansions(interface.type_aliases());
        let world = hir::ImportedSemanticWorld::from_dependencies(
            source.cone(),
            vec![
                core.provider(),
                hir::ImportedProviderInput {
                    foundation: &imported,
                    interface,
                    alias_expansions: &aliases,
                },
            ],
            Vec::new(),
        )
        .unwrap();
        let inputs = CurrentConeSources::try_new(
            &source,
            core.foundation.import_core_inputs(&core.interface).unwrap(),
            &world,
        )
        .unwrap();
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &inputs)
            .unwrap_or_else(|errors| panic!("{label}: {errors:?}"));
        let mut published = hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
        let mut authority = hir::CrossConeHirProductionAuthority::new(
            &published,
            &output.output().export.public_export_bindings,
            &world,
        );
        let mut public =
            hir::CrossConeHirInterfaceSectionV1::from_dependency_hir(&output, &[], &mut authority)
                .unwrap();
        published
            .complete_cross_cone_interface_source_points(output.output().export.module(), &public)
            .unwrap();
        let mir = include_mir.then(|| {
            let dependencies = select_library_callables(&output);
            let mir = scoop_mir_lower::lower_current_cone(
                &output,
                dependencies,
                Default::default(),
                None,
                Default::default(),
            )
            .unwrap_or_else(|errors| panic!("{label}: {errors:?}"));
            (
                scoop_mir::dump(mir.module()),
                encode(mir.foundation()).unwrap(),
            )
        });
        results.push((
            hir::dump(&output.output().export),
            mir.as_ref().map(|value| value.0.clone()),
            encode(&published).unwrap(),
            encode(&public.index_for_wire().unwrap()).unwrap(),
            mir.map(|value| value.1),
        ));
    }
    assert_eq!(results[0].0, results[1].0, "{label}: selected HIR");
    assert_eq!(results[0].1, results[1].1, "{label}: concrete MIR");
    assert_eq!(
        results[0].2, results[1].2,
        "{label}: republished identities"
    );
    assert_eq!(
        results[0].3, results[1].3,
        "{label}: republished declarations and bodies"
    );
    assert_eq!(
        results[0].4, results[1].4,
        "{label}: materialized callable identities"
    );
}
