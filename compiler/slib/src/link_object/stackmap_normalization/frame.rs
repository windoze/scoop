//! Stack geometry is independent of the object container and target libc.

use scoop_identity::TargetProfileId;

use scoop_lir::BackendMachineFrameV1 as StackmapFrame;

pub(super) fn for_target(target: TargetProfileId) -> Option<StackmapFrame> {
    scoop_target_data::selection(target)
        .map(|selection| selection.backend().contract().machine_frame)
}

pub(super) fn valid_stack_size(frame: StackmapFrame, size: u64) -> bool {
    match frame {
        StackmapFrame::DarwinAarch64 => size >= 16 && size % 16 == 0,
        StackmapFrame::SysvAmd64 | StackmapFrame::WindowsSeh => {
            size >= 8 && size % 16 == 8 && size <= i64::MAX as u64
        }
    }
}

pub(super) fn valid_root(frame: StackmapFrame, register: u16, offset: i64, size: u64) -> bool {
    let (sp, fp) = frame.registers();
    if matches!(frame, StackmapFrame::WindowsSeh) {
        // SEH reconstructs the actual RSP/RBP. Runtime checks the resolved
        // address against the thread's stack; size does not locate RBP.
        return (register == sp || register == fp) && offset % 8 == 0;
    }
    let (frame_pointer, root_end) = match frame {
        StackmapFrame::DarwinAarch64 => (i128::from(size) - 16, i128::from(size)),
        StackmapFrame::SysvAmd64 => (i128::from(size) - 8, i128::from(size) - 8),
        StackmapFrame::WindowsSeh => unreachable!("SEH locations were handled above"),
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
        && (!matches!(frame, StackmapFrame::SysvAmd64) || address % 8 == 0)
}
