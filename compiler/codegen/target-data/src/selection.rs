use scoop_lir::{TargetProfileId, ValidatedLirTargetSelection};

pub const DARWIN_AARCH64_LLVM_22_1: ValidatedLirTargetSelection =
    ValidatedLirTargetSelection::new(crate::DARWIN_AARCH64, crate::LLVM_22_1);
pub const LINUX_X86_64_GNU_LLVM_22_1: ValidatedLirTargetSelection =
    ValidatedLirTargetSelection::new(crate::LINUX_X86_64_GNU, crate::LLVM_22_1_LINUX_X86_64);
pub const LINUX_X86_64_MUSL_LLVM_22_1: ValidatedLirTargetSelection =
    ValidatedLirTargetSelection::new(crate::LINUX_X86_64_MUSL, crate::LLVM_22_1_LINUX_X86_64);

pub const fn selection(id: TargetProfileId) -> ValidatedLirTargetSelection {
    match id {
        TargetProfileId::DarwinAarch64 => DARWIN_AARCH64_LLVM_22_1,
        TargetProfileId::LinuxX86_64Gnu => LINUX_X86_64_GNU_LLVM_22_1,
        TargetProfileId::LinuxX86_64Musl => LINUX_X86_64_MUSL_LLVM_22_1,
    }
}
