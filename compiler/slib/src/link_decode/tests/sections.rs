use super::*;

pub(super) fn empty_hir_library_section() -> Vec<u8> {
    vec![0xa3, 0x02, 0xa1, 0x00, 0x01, 0x03, 0x80, 0x05, 0x80]
}

pub(super) fn empty_mir_library_section() -> Vec<u8> {
    encode(
        &scoop_mir::CoreBootstrapBridgeSectionV1::try_new(
            cone().identity(),
            scoop_mir::EntryMirBridgeBranchV1::Library,
            scoop_mir::StrongCallableBridgeSurfaceV1::try_new(Vec::new()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
