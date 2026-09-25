use super::*;
use scoop_wire::{ResourceKind, WireErrorKind, WirePath};

#[test]
fn layout_link_shared_handoff_preserves_all_payloads_and_original_budget() {
    let bytes = layout_artifact(false, None, empty_link_closure(0x22), false);
    let link = open_graph(&bytes)
        .decode_cross_cone_layout_link_sections()
        .unwrap();
    let usage = link.decode_usage();
    let fingerprint = link.artifact_fingerprint();
    let common = [
        encode(link.hir_foundation_wire()).unwrap(),
        encode(link.hir_core_production_wire()).unwrap(),
        encode(link.hir_interface_wire()).unwrap(),
        encode(link.hir_type_semantics_wire()).unwrap(),
        encode(link.mir_foundation_wire()).unwrap(),
        encode(link.mir_core_production_wire()).unwrap(),
        encode(link.mir_cross_cone_bridge_wire()).unwrap(),
        encode(link.mir_type_bridge_wire()).unwrap(),
        encode(link.lir_foundation_wire()).unwrap(),
        encode(link.lir_strong_production_wire()).unwrap(),
        encode(link.lir_cross_cone_bridge_wire()).unwrap(),
        encode(link.lir_layout_abi_wire()).unwrap(),
    ];
    let remaining = [
        encode(link.production_manifest_wire()).unwrap(),
        encode(link.link_identity_closure_wire()).unwrap(),
        encode(link.cross_cone_link_closure_wire()).unwrap(),
        encode(link.layout_link_closure_wire()).unwrap(),
    ];
    let shared = link.into_shared_sections().unwrap();
    assert_eq!(shared.coordinate(), cone().coordinate());
    assert_eq!(shared.identity(), cone().identity());
    assert_eq!(shared.artifact_fingerprint(), fingerprint);
    let after = shared.decode_usage();
    assert!(after.owned_bytes > usage.owned_bytes);
    assert_eq!(after.validation_work_units, usage.validation_work_units);
    assert_eq!(after.decoded_nodes, usage.decoded_nodes);
    assert_eq!(after.decoded_edges, usage.decoded_edges);
    assert_eq!(
        common,
        [
            encode(shared.hir_foundation_wire()).unwrap(),
            encode(shared.hir_core_production_wire()).unwrap(),
            encode(shared.hir_interface_wire()).unwrap(),
            encode(shared.hir_type_semantics_wire()).unwrap(),
            encode(shared.mir_foundation_wire()).unwrap(),
            encode(shared.mir_core_production_wire()).unwrap(),
            encode(shared.mir_cross_cone_bridge_wire()).unwrap(),
            encode(shared.mir_type_bridge_wire()).unwrap(),
            encode(shared.lir_foundation_wire()).unwrap(),
            encode(shared.lir_strong_production_wire()).unwrap(),
            encode(shared.lir_cross_cone_bridge_wire()).unwrap(),
            encode(shared.lir_layout_abi_wire()).unwrap(),
        ]
    );
    let link = shared.link_sections().unwrap();
    assert_eq!(
        remaining,
        [
            encode(link.production_manifest_wire()).unwrap(),
            encode(link.link_identity_closure_wire()).unwrap(),
            encode(link.cross_cone_link_closure_wire()).unwrap(),
            encode(link.layout_link_closure_wire()).unwrap(),
        ]
    );
    assert!(
        open_graph(&bytes)
            .decode_cross_cone_layout_compile_sections()
            .unwrap()
            .link_sections()
            .is_none()
    );
}

#[test]
fn layout_link_shared_handoff_cannot_reset_an_exhausted_artifact_budget() {
    let bytes = layout_artifact(false, None, empty_link_closure(0x22), false);
    let mut link = open_graph(&bytes)
        .decode_cross_cone_layout_link_sections()
        .unwrap();
    let meter = link.graph.envelope.meter_mut();
    meter
        .charge_owned_bytes(
            meter.limits().owned_bytes - meter.usage().owned_bytes,
            &WirePath::root(),
        )
        .unwrap();
    assert!(matches!(
        link.into_shared_sections().unwrap_err().kind(),
        WireErrorKind::LimitExceeded {
            resource: ResourceKind::OwnedBytes,
            ..
        }
    ));
}
