use super::*;
use scoop_identity::{PendingIdentityValidation, ValidatedIdentityGraph};
use scoop_wire::{WireDecode, WireEncode, decode_canonical, encode};

fn decoded<T: WireDecode>(value: &impl WireEncode) -> T {
    decode_canonical(&encode(value).unwrap()).unwrap()
}

pub(super) fn identity_closure(output: &hir::DependencyHirOutput) -> ValidatedIdentityGraph {
    let mut foundation = hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap();
    let interface = public_interface(output);
    foundation
        .complete_cross_cone_interface_source_points(output.output().export.module(), &interface)
        .unwrap();
    identity_closure_for_foundation(output, foundation)
}

pub(super) fn identity_closure_for_foundation(
    output: &hir::DependencyHirOutput,
    foundation: hir::CanonicalHirFoundation,
) -> ValidatedIdentityGraph {
    let core_graph = core_identity_closure();
    let foundation: hir::DecodedHirFoundation = decoded(&foundation);
    let mut pending = PendingIdentityValidation::new();
    pending
        .register_authority(output.output().export.cone)
        .unwrap();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    foundation.register_identities(&mut pending).unwrap();
    pending
        .register_external_graph_authorities(&core_graph)
        .unwrap();
    foundation.resolve_identities(&mut pending).unwrap();
    pending.finish().unwrap()
}

pub(super) fn core_identity_closure() -> ValidatedIdentityGraph {
    let core = trusted_core();
    let foundation: hir::DecodedHirFoundation = decoded(&core.foundation);
    let mut pending = PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    foundation.register_identities(&mut pending).unwrap();
    foundation.resolve_identities(&mut pending).unwrap();
    pending.finish().unwrap()
}
