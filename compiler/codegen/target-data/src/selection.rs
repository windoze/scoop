use scoop_lir::{TargetProfileId, ValidatedLirTargetSelection};

pub const DARWIN_AARCH64_LLVM_22_1: ValidatedLirTargetSelection =
    ValidatedLirTargetSelection::new(crate::DARWIN_AARCH64, crate::LLVM_22_1);
pub const LINUX_X86_64_GNU_LLVM_22_1: ValidatedLirTargetSelection =
    ValidatedLirTargetSelection::new(crate::LINUX_X86_64_GNU, crate::LLVM_22_1_LINUX_X86_64);
pub const LINUX_X86_64_MUSL_LLVM_22_1: ValidatedLirTargetSelection =
    ValidatedLirTargetSelection::new(crate::LINUX_X86_64_MUSL, crate::LLVM_22_1_LINUX_X86_64);

pub const WINDOWS_X86_64_MSVC_UCRT_LLVM_22_1: ValidatedLirTargetSelection =
    ValidatedLirTargetSelection::new(
        crate::WINDOWS_X86_64_MSVC_UCRT,
        crate::LLVM_22_1_WINDOWS_X86_64,
    );
pub const WINDOWS_X86_64_MINGW_MSVCRT_LLVM_22_1: ValidatedLirTargetSelection =
    ValidatedLirTargetSelection::new(
        crate::WINDOWS_X86_64_MINGW_MSVCRT,
        crate::LLVM_22_1_WINDOWS_X86_64,
    );
pub const WINDOWS_X86_64_MINGW_UCRT_LLVM_22_1: ValidatedLirTargetSelection =
    ValidatedLirTargetSelection::new(
        crate::WINDOWS_X86_64_MINGW_UCRT,
        crate::LLVM_22_1_WINDOWS_X86_64,
    );

pub const fn selection(id: TargetProfileId) -> Option<ValidatedLirTargetSelection> {
    match id {
        crate::DARWIN_AARCH64_ID => Some(DARWIN_AARCH64_LLVM_22_1),
        crate::LINUX_X86_64_GNU_ID => Some(LINUX_X86_64_GNU_LLVM_22_1),
        crate::LINUX_X86_64_MUSL_ID => Some(LINUX_X86_64_MUSL_LLVM_22_1),
        crate::WINDOWS_X86_64_MSVC_UCRT_ID => Some(WINDOWS_X86_64_MSVC_UCRT_LLVM_22_1),
        crate::WINDOWS_X86_64_MINGW_MSVCRT_ID => Some(WINDOWS_X86_64_MINGW_MSVCRT_LLVM_22_1),
        crate::WINDOWS_X86_64_MINGW_UCRT_ID => Some(WINDOWS_X86_64_MINGW_UCRT_LLVM_22_1),
        _ => None,
    }
}
