use scoop_lir::*;

const POINTER: ScalarLayout = ScalarLayout::new(8, 8);
const RAW_POINTER: PointerRepresentation = PointerRepresentation::new(
    POINTER,
    PointerNullEncoding::AllZeroBits,
    InternalPointerCarrier::BitPreservingU64,
);
const SCALARS: [ScalarLayout; 7] = [
    ScalarLayout::new(1, 1),
    ScalarLayout::new(1, 1),
    ScalarLayout::new(2, 2),
    ScalarLayout::new(4, 4),
    ScalarLayout::new(8, 8),
    ScalarLayout::new(4, 4),
    ScalarLayout::new(8, 8),
];

const DARWIN: TargetLayoutDescription = TargetLayoutDescription {
    canonical_triple: "aarch64-apple-darwin",
    llvm_data_layout: "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32",
    object_format: NativeObjectFormat::MachO64,
    native_symbol_normalization: NativeSymbolNormalization::MachOExternalUnderscore,
    byte_order: ByteOrder::Little,
    scalar_layouts: SCALARS,
    managed_pointer: POINTER,
    data_pointer: RAW_POINTER,
    code_pointer: RAW_POINTER,
    metadata_pointer: POINTER,
    stack_alignment: 16,
    maximum_managed_alignment: 16,
    maximum_managed_object_size: i64::MAX as u64,
};
const LINUX: TargetLayoutDescription = TargetLayoutDescription {
    canonical_triple: "x86_64-unknown-linux-gnu",
    llvm_data_layout: "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
    object_format: NativeObjectFormat::Elf64,
    native_symbol_normalization: NativeSymbolNormalization::ElfIdentity,
    ..DARWIN
};
pub const DARWIN_AARCH64: LirTargetProfile =
    LirTargetProfile::new(&crate::DARWIN_AARCH64_ID, &DARWIN);
pub const LINUX_X86_64_GNU: LirTargetProfile =
    LirTargetProfile::new(&crate::LINUX_X86_64_GNU_ID, &LINUX);
pub const LINUX_X86_64_MUSL: LirTargetProfile = LirTargetProfile::new(
    &crate::LINUX_X86_64_MUSL_ID,
    &TargetLayoutDescription {
        canonical_triple: "x86_64-unknown-linux-musl",
        ..LINUX
    },
);

pub const fn layout(id: TargetProfileId) -> Option<LirTargetProfile> {
    match crate::selection(id) {
        Some(selection) => Some(selection.target()),
        None => None,
    }
}
