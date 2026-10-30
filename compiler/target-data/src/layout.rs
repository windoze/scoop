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

#[derive(Clone, Copy)]
pub(crate) enum LayoutKind {
    DarwinAarch64,
    LinuxX86_64,
    WindowsX86_64,
}

pub(crate) const fn describe(
    triple: &'static str,
    kind: LayoutKind,
    procedure_abi: ProcedureAbi,
    scoop_abi_classifier: ScoopAbiClassifier,
    exception_handling: TargetExceptionHandling,
) -> TargetLayoutDescription {
    let (llvm_data_layout, object_format, native_symbol_normalization) = match kind {
        LayoutKind::DarwinAarch64 => (
            "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32",
            NativeObjectFormat::MachO64,
            NativeSymbolNormalization::MachOExternalUnderscore,
        ),
        LayoutKind::LinuxX86_64 => (
            "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
            NativeObjectFormat::Elf64,
            NativeSymbolNormalization::ElfIdentity,
        ),
        LayoutKind::WindowsX86_64 => (
            "e-m:w-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128",
            NativeObjectFormat::Coff64,
            NativeSymbolNormalization::CoffIdentity,
        ),
    };
    TargetLayoutDescription {
        canonical_triple: triple,
        llvm_data_layout,
        object_format,
        architecture: match kind {
            LayoutKind::DarwinAarch64 => TargetArchitecture::Aarch64,
            LayoutKind::LinuxX86_64 | LayoutKind::WindowsX86_64 => TargetArchitecture::X86_64,
        },
        procedure_abi,
        scoop_abi_classifier,
        exception_handling,
        native_symbol_normalization,
        byte_order: ByteOrder::Little,
        scalar_layouts: SCALARS,
        managed_pointer: POINTER,
        data_pointer: RAW_POINTER,
        code_pointer: RAW_POINTER,
        metadata_pointer: POINTER,
        stack_alignment: 16,
        maximum_managed_alignment: 16,
        maximum_managed_object_size: i64::MAX as u64,
    }
}
