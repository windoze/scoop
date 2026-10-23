use super::*;

pub fn lir_link_support_capability() -> CapabilityId {
    known_capability("org.scoop-lang.lir", "link-support")
}

pub fn hir_identity_foundation_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.hir", "identity-foundation", 10)
        .expect("built-in capability id is valid")
}

pub fn mir_identity_foundation_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.mir", "identity-foundation", 8)
        .expect("built-in capability id is valid")
}

pub fn lir_identity_foundation_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.lir", "identity-foundation", 9)
        .expect("built-in capability id is valid")
}

pub fn manifest_single_cone_production_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.manifest", "single-cone-production", 9)
        .expect("built-in capability id is valid")
}

pub fn manifest_test_directory_capability() -> CapabilityId {
    known_capability("org.scoop-lang.manifest", "test-directory")
}

pub fn hir_core_bootstrap_interface_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.hir", "core-bootstrap-interface", 16)
        .expect("built-in capability id is valid")
}

pub fn hir_cross_cone_interface_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.hir", "cross-cone-interface", 81)
        .expect("built-in capability id is valid")
}

pub fn mir_core_bootstrap_bridge_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.mir", "core-bootstrap-bridge", 2)
        .expect("built-in capability id is valid")
}

pub fn mir_cross_cone_param_free_bridge_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.mir", "cross-cone-param-free-bridge", 2)
        .expect("built-in capability id is valid")
}

pub fn lir_cross_cone_param_free_bridge_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.lir", "cross-cone-param-free-bridge", 3)
        .expect("built-in capability id is valid")
}

pub fn lir_cross_cone_link_closure_capability() -> CapabilityId {
    known_capability("org.scoop-lang.lir", "cross-cone-link-closure")
}

pub fn lir_strong_production_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.lir", "strong-production", 24)
        .expect("a fixed capability identity is valid")
}

pub fn hir_cross_cone_type_semantics_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.hir", "cross-cone-type-semantics", 30)
        .expect("built-in capability id is valid")
}

pub fn mir_cross_cone_type_bridge_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.mir", "cross-cone-type-bridge", 19)
        .expect("built-in capability id is valid")
}

pub fn lir_cross_cone_layout_abi_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.lir", "cross-cone-layout-abi", 17)
        .expect("built-in capability id is valid")
}

pub fn lir_cross_cone_layout_link_closure_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.lir", "cross-cone-layout-link-closure", 8)
        .expect("built-in capability id is valid")
}

pub fn lir_cone_production_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.lir", "cone-production", 14)
        .expect("built-in capability id is valid")
}

pub fn lir_link_identity_closure_capability() -> CapabilityId {
    CapabilityId::new("org.scoop-lang.lir", "link-identity-closure", 18)
        .expect("built-in capability id is valid")
}

pub(super) fn known_capability(namespace: &str, name: &str) -> CapabilityId {
    CapabilityId::new(namespace, name, 1).expect("built-in capability id is valid")
}
