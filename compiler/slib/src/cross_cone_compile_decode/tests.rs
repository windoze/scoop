use scoop_wire::encode;

pub(crate) fn empty_cross_cone_hir_interface() -> Vec<u8> {
    let mut section = scoop_hir::CrossConeHirInterfaceSectionV1::empty();
    encode(&section.index_for_wire().unwrap()).unwrap()
}
