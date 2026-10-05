//! Stack geometry is independent of the object container and target libc.

use scoop_identity::TargetProfileId;

#[derive(Clone, Copy)]
pub(super) enum StackmapArchitecture {
    Aarch64,
    X86_64,
}

impl StackmapArchitecture {
    pub(super) fn for_target(target: TargetProfileId) -> Self {
        match target {
            TargetProfileId::DarwinAarch64 => Self::Aarch64,
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => Self::X86_64,
        }
    }

    pub(super) fn valid_stack_size(self, size: u64) -> bool {
        match self {
            Self::Aarch64 => size >= 16 && size % 16 == 0,
            Self::X86_64 => size >= 8 && size % 16 == 8 && size <= i64::MAX as u64,
        }
    }

    pub(super) fn registers(self) -> (u16, u16) {
        match self {
            Self::Aarch64 => (31, 29),
            Self::X86_64 => (7, 6),
        }
    }

    pub(super) fn valid_root(self, register: u16, offset: i64, size: u64) -> bool {
        let (sp, fp) = self.registers();
        let (frame_pointer, root_end) = match self {
            Self::Aarch64 => (i128::from(size) - 16, i128::from(size)),
            Self::X86_64 => (i128::from(size) - 8, i128::from(size) - 8),
        };
        let base = if register == sp {
            0
        } else if register == fp {
            frame_pointer
        } else {
            return false;
        };
        let address = base + i128::from(offset);
        address >= 0
            && address + 8 <= root_end
            && (!matches!(self, Self::X86_64) || address % 8 == 0)
    }
}
