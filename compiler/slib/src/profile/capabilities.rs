use super::*;

pub fn hir_identity_foundation_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.hir", "identity-foundation", 3)
        .expect("built-in capability id is valid")
}

pub fn mir_identity_foundation_capability() -> CapabilityId {
    known_capability("org.scoop-lang.mir", "identity-foundation")
}

pub fn lir_identity_foundation_capability() -> CapabilityId {
    known_capability("org.scoop-lang.lir", "identity-foundation")
}

pub fn manifest_single_cone_production_capability() -> CapabilityId {
    known_capability("org.scoop-lang.manifest", "single-cone-production")
}

pub fn hir_core_bootstrap_interface_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.hir", "core-bootstrap-interface", 3)
        .expect("built-in capability id is valid")
}

pub fn hir_cross_cone_interface_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.hir", "cross-cone-interface", 14)
        .expect("built-in capability id is valid")
}

pub fn mir_core_bootstrap_bridge_capability() -> CapabilityId {
    known_capability("org.scoop-lang.mir", "core-bootstrap-bridge")
}

pub fn mir_cross_cone_param_free_bridge_capability() -> CapabilityId {
    known_capability("org.scoop-lang.mir", "cross-cone-param-free-bridge")
}

pub fn lir_cross_cone_param_free_bridge_capability() -> CapabilityId {
    known_capability("org.scoop-lang.lir", "cross-cone-param-free-bridge")
}

pub fn lir_cross_cone_link_closure_capability() -> CapabilityId {
    known_capability("org.scoop-lang.lir", "cross-cone-link-closure")
}

pub fn lir_strong_production_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.lir", "strong-production", 5)
        .expect("a fixed capability identity is valid")
}

pub fn hir_cross_cone_type_semantics_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.hir", "cross-cone-type-semantics", 2)
        .expect("built-in capability id is valid")
}

pub fn mir_cross_cone_type_bridge_capability() -> CapabilityId {
    known_capability("org.scoop-lang.mir", "cross-cone-type-bridge")
}

pub fn lir_cross_cone_layout_abi_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.lir", "cross-cone-layout-abi", 2)
        .expect("built-in capability id is valid")
}

pub fn lir_cross_cone_layout_link_closure_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.lir", "cross-cone-layout-link-closure", 2)
        .expect("built-in capability id is valid")
}

pub fn lir_strong_production_v2_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.lir", "strong-production", 6)
        .expect("built-in capability id is valid")
}

pub fn lir_link_identity_closure_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.lir", "link-identity-closure", 2)
        .expect("built-in capability id is valid")
}

pub(super) fn known_capability(namespace: &str, name: &str) -> CapabilityId {
    CapabilityId::new(namespace, name, 1).expect("built-in capability id is valid")
}
