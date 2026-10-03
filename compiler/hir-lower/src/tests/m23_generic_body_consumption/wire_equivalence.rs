//! The same exported graph must have the same consumer after wire round-trip.

use super::*;
use scoop_identity::{ConeIdentity, PendingIdentityValidation};
use scoop_wire::{decode_canonical, encode};

#[test]
fn exported_graphs_have_identical_direct_and_wire_consumers() {
    for (fixture, case) in [
        ("m23-generic-body-consumption", "consumer"),
        ("m23-shared-constructor-requests", "both-imported"),
        ("m23-imported-options", "construct"),
        ("m23-imported-pointers", "defaults"),
    ] {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(fixture);
        let provider = std::fs::read_to_string(directory.join("provider.scoop")).unwrap();
        let consumer = std::fs::read_to_string(directory.join(format!("{case}.scoop"))).unwrap();
        compare_consumers(fixture, &provider, &consumer);
    }
}

fn compare_consumers(label: &str, provider: &str, consumer: &str) {
    let mut core = trusted_core();
    let coordinate = ConeCoordinate::new("test", "generic-provider", "1.0.0").unwrap();
    let (foundation, interface) =
        project_dependency_text(&core, &coordinate, provider, &["Boolean"]);
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
        let dependencies =
            scoop_mir::SelectedExternalMirSet::try_from_callables(source.cone(), Vec::new())
                .unwrap();
        let mir = scoop_mir_lower::lower_current_cone(&output, dependencies)
            .unwrap_or_else(|errors| panic!("{label}: {errors:?}"));
        results.push((
            hir::dump(&output.output().export),
            scoop_mir::dump(mir.module()),
            encode(&published).unwrap(),
            encode(&public.index_for_wire().unwrap()).unwrap(),
            encode(mir.foundation()).unwrap(),
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
