//! Stack geometry is independent of the object container and target libc.

use scoop_identity::TargetProfileId;

use scoop_lir::MachineFrameRecoveryV1 as StackmapFrame;

pub(super) fn for_target(target: TargetProfileId) -> Option<StackmapFrame> {
    scoop_target_data::selection(target)
        .map(|selection| selection.backend().contract().machine_frame_recovery)
}

pub(super) fn valid_stack_size(frame: StackmapFrame, size: u64) -> bool {
    match frame {
        StackmapFrame::DarwinAarch64Fixed => size >= 16 && size.is_multiple_of(16),
        StackmapFrame::SysvAmd64Fixed | StackmapFrame::WindowsAmd64VirtualUnwind => {
            size >= 8 && size % 16 == 8 && size <= i64::MAX as u64
        }
    }
}

pub(super) fn valid_root(frame: StackmapFrame, register: u16, offset: i64, size: u64) -> bool {
    let (sp, fp) = frame.registers();
    if matches!(frame, StackmapFrame::WindowsAmd64VirtualUnwind) {
        // SEH reconstructs the actual RSP/RBP. Runtime checks the resolved
        // address against the thread's stack; size does not locate RBP.
        return (register == sp || register == fp) && offset % 8 == 0;
    }
    let (frame_pointer, root_end) = match frame {
        StackmapFrame::DarwinAarch64Fixed => (i128::from(size) - 16, i128::from(size)),
        StackmapFrame::SysvAmd64Fixed => (i128::from(size) - 8, i128::from(size) - 8),
        StackmapFrame::WindowsAmd64VirtualUnwind => {
            unreachable!("virtual-unwind locations were handled above")
        }
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
        && (!matches!(frame, StackmapFrame::SysvAmd64Fixed) || address % 8 == 0)
}
