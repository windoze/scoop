// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use std::ffi::c_void;

// 与 `runtime/c/scoop_stackmap.h` 中的 `ScoopStackmapRecordRef` 对齐。
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct ScoopStackmapRecordRef {
    return_address: usize,
    function_address: usize,
    instruction_offset: u32,
    patchpoint_id: u64,
    record_ptr: *const u8,
    record_size: u32,
}

unsafe extern "C" {
    fn scoop_stackmap_record_visit_root_slots(
        rec: *const ScoopStackmapRecordRef,
        frame_sp: usize,
        frame_fp: usize,
        visitor: extern "C" fn(*mut *mut c_void, *mut c_void),
        ctx: *mut c_void,
        out_error: *mut u32,
    ) -> u64;
}

// --- StackMap v3 location encoding（subset） ---
const LOC_REGISTER: u8 = 1;
const LOC_DIRECT: u8 = 2;
const LOC_INDIRECT: u8 = 3;

// 与 `runtime/c/scoop_stackmap.h` 的 `ScoopStackmapVisitError` 对齐（只引用本测试需要的子集）。
const VISIT_OK: u32 = 0;
const ERR_UNSUPPORTED_LOCATION: u32 = 4;
const ERR_UNSUPPORTED_DWARF_REG: u32 = 5;

#[repr(C)]
struct SlotCollector {
    slots: *mut usize,
    cap: usize,
    len: usize,
}

extern "C" fn collect_slot(slot: *mut *mut c_void, ctx: *mut c_void) {
    if ctx.is_null() {
        return;
    }
    unsafe {
        let collector = &mut *(ctx as *mut SlotCollector);
        if collector.len >= collector.cap {
            return;
        }
        *collector.slots.add(collector.len) = slot as usize;
        collector.len += 1;
    }
}

fn dwarf_reg_sp() -> u16 {
    if cfg!(target_arch = "x86_64") {
        7
    } else if cfg!(target_arch = "aarch64") {
        31
    } else {
        0
    }
}

#[derive(Debug, Clone, Copy)]
struct LocationSpec {
    loc_type: u8,
    dwarf_reg: u16,
    offset: i32,
}

fn build_record_locations(locations: &[LocationSpec]) -> Vec<u8> {
    let ptr_size = std::mem::size_of::<*mut c_void>() as u16;

    // record layout（v3）：
    //  u64 PatchPointID
    //  u32 InstructionOffset
    //  u16 Reserved
    //  u16 NumLocations
    //  Location[NumLocations]（12 bytes）
    //  padding to 8 bytes
    //  u16 NumLiveOuts
    //  u16 Reserved
    //  padding to 8 bytes
    let mut out = Vec::new();
    out.extend_from_slice(&1u64.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(locations.len() as u16).to_le_bytes());

    for loc in locations {
        // Location（12 bytes）:
        //  u8  Type
        //  u8  Reserved0
        //  u16 LocationSize
        //  u16 DwarfRegNum
        //  u16 Reserved1
        //  i32 Offset
        out.push(loc.loc_type);
        out.push(0u8);
        out.extend_from_slice(&ptr_size.to_le_bytes());
        out.extend_from_slice(&loc.dwarf_reg.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&loc.offset.to_le_bytes());
    }

    while out.len() % 8 != 0 {
        out.push(0u8);
    }
    out.extend_from_slice(&0u16.to_le_bytes()); // NumLiveOuts
    out.extend_from_slice(&0u16.to_le_bytes()); // Reserved
    while out.len() % 8 != 0 {
        out.push(0u8);
    }

    out
}

#[test]
fn stackmap_record_direct_location_yields_expected_slot_address() {
    let sp_reg = dwarf_reg_sp();
    if sp_reg == 0 {
        return;
    }

    // roots 契约：roots locations 必须为偶数（statepoint base/derived 成对语义）。
    // 因此这里用两个 Direct slots 模拟一对 roots。
    let record = build_record_locations(&[
        LocationSpec {
            loc_type: LOC_DIRECT,
            dwarf_reg: sp_reg,
            offset: 16,
        },
        LocationSpec {
            loc_type: LOC_DIRECT,
            dwarf_reg: sp_reg,
            offset: 24,
        },
    ]);
    let rec = ScoopStackmapRecordRef {
        record_ptr: record.as_ptr(),
        record_size: record.len() as u32,
        ..Default::default()
    };

    let mut stack = vec![0usize; 8];
    stack[2] = 0x1234; // non-null，使 visitor 一定被调用。
    stack[3] = 0; // 第二个 roots slot 设为 null，确保只收集一个 slot。
    let sp = stack.as_mut_ptr() as usize;

    let mut slots = [0usize; 4];
    let mut collector = SlotCollector {
        slots: slots.as_mut_ptr(),
        cap: slots.len(),
        len: 0,
    };

    let mut err: u32 = 999;
    let visited = unsafe {
        scoop_stackmap_record_visit_root_slots(
            &rec,
            sp,
            0,
            collect_slot,
            (&mut collector as *mut SlotCollector).cast::<c_void>(),
            &mut err,
        )
    };

    assert_eq!(err, VISIT_OK);
    assert_eq!(visited, 1);
    assert_eq!(collector.len, 1);
    assert_eq!(slots[0], sp + 16);
}

#[test]
fn stackmap_record_indirect_location_yields_expected_slot_address() {
    let sp_reg = dwarf_reg_sp();
    if sp_reg == 0 {
        return;
    }

    // roots 契约：roots locations 必须为偶数。
    let record = build_record_locations(&[
        LocationSpec {
            loc_type: LOC_INDIRECT,
            dwarf_reg: sp_reg,
            offset: 16,
        },
        LocationSpec {
            loc_type: LOC_INDIRECT,
            dwarf_reg: sp_reg,
            offset: 32,
        },
    ]);
    let rec = ScoopStackmapRecordRef {
        record_ptr: record.as_ptr(),
        record_size: record.len() as u32,
        ..Default::default()
    };

    let mut stack = vec![0usize; 8];
    let inner_slot_addr = (&mut stack[3] as *mut usize) as usize;
    stack[2] = inner_slot_addr; // *(sp+16) == &stack[3]
    stack[3] = 0x5678; // non-null，使 visitor 一定被调用。
    stack[4] = 0; // *(sp+32) == null，跳过第二个 indirect roots slot。
    let sp = stack.as_mut_ptr() as usize;

    let mut slots = [0usize; 4];
    let mut collector = SlotCollector {
        slots: slots.as_mut_ptr(),
        cap: slots.len(),
        len: 0,
    };

    let mut err: u32 = 999;
    let visited = unsafe {
        scoop_stackmap_record_visit_root_slots(
            &rec,
            sp,
            0,
            collect_slot,
            (&mut collector as *mut SlotCollector).cast::<c_void>(),
            &mut err,
        )
    };

    assert_eq!(err, VISIT_OK);
    assert_eq!(visited, 1);
    assert_eq!(collector.len, 1);
    assert_eq!(slots[0], sp + (3 * std::mem::size_of::<usize>()));
}

#[test]
fn stackmap_record_odd_roots_slots_count_is_error() {
    let sp_reg = dwarf_reg_sp();
    if sp_reg == 0 {
        return;
    }

    // roots slots 为奇数：违反 statepoint base/derived 成对语义，应返回稳定错误码。
    let record = build_record_locations(&[LocationSpec {
        loc_type: LOC_DIRECT,
        dwarf_reg: sp_reg,
        offset: 16,
    }]);
    let rec = ScoopStackmapRecordRef {
        record_ptr: record.as_ptr(),
        record_size: record.len() as u32,
        ..Default::default()
    };

    let mut stack = vec![0usize; 8];
    stack[2] = 0x1234;
    let sp = stack.as_mut_ptr() as usize;

    let mut slots = [0usize; 4];
    let mut collector = SlotCollector {
        slots: slots.as_mut_ptr(),
        cap: slots.len(),
        len: 0,
    };

    let mut err: u32 = 999;
    let visited = unsafe {
        scoop_stackmap_record_visit_root_slots(
            &rec,
            sp,
            0,
            collect_slot,
            (&mut collector as *mut SlotCollector).cast::<c_void>(),
            &mut err,
        )
    };

    assert_eq!(visited, 0);
    assert_eq!(collector.len, 0);
    assert_eq!(err, ERR_UNSUPPORTED_LOCATION);
}

#[test]
fn stackmap_record_register_location_is_ignored() {
    let sp_reg = dwarf_reg_sp();
    if sp_reg == 0 {
        return;
    }

    let record = build_record_locations(&[LocationSpec {
        loc_type: LOC_REGISTER,
        dwarf_reg: sp_reg,
        offset: 0,
    }]);
    let rec = ScoopStackmapRecordRef {
        record_ptr: record.as_ptr(),
        record_size: record.len() as u32,
        ..Default::default()
    };

    let mut stack = vec![0usize; 8];
    stack[0] = 0x1;
    let sp = stack.as_mut_ptr() as usize;

    let mut slots = [0usize; 4];
    let mut collector = SlotCollector {
        slots: slots.as_mut_ptr(),
        cap: slots.len(),
        len: 0,
    };

    let mut err: u32 = 0;
    let visited = unsafe {
        scoop_stackmap_record_visit_root_slots(
            &rec,
            sp,
            0,
            collect_slot,
            (&mut collector as *mut SlotCollector).cast::<c_void>(),
            &mut err,
        )
    };

    assert_eq!(visited, 0);
    assert_eq!(collector.len, 0);
    assert_eq!(
        err, VISIT_OK,
        "register locations are ignored (they are not addressable `void**` slots)"
    );
}

#[test]
fn stackmap_record_root_slot_must_be_suffix_or_error() {
    let sp_reg = dwarf_reg_sp();
    if sp_reg == 0 {
        return;
    }

    // roots 契约：roots locations 必须是连续后缀。
    //
    // 这里故意构造一个违反契约的 record：
    // - 第一个 location 是“像 roots slot”的 Direct；
    // - 但最后一个 location 不是 roots slot（Register）。
    let record = build_record_locations(&[
        LocationSpec {
            loc_type: LOC_DIRECT,
            dwarf_reg: sp_reg,
            offset: 16,
        },
        LocationSpec {
            loc_type: LOC_REGISTER,
            dwarf_reg: sp_reg,
            offset: 0,
        },
    ]);
    let rec = ScoopStackmapRecordRef {
        record_ptr: record.as_ptr(),
        record_size: record.len() as u32,
        ..Default::default()
    };

    let mut stack = vec![0usize; 8];
    stack[2] = 0x1234;
    let sp = stack.as_mut_ptr() as usize;

    let mut slots = [0usize; 4];
    let mut collector = SlotCollector {
        slots: slots.as_mut_ptr(),
        cap: slots.len(),
        len: 0,
    };

    let mut err: u32 = 999;
    let visited = unsafe {
        scoop_stackmap_record_visit_root_slots(
            &rec,
            sp,
            0,
            collect_slot,
            (&mut collector as *mut SlotCollector).cast::<c_void>(),
            &mut err,
        )
    };

    assert_eq!(visited, 0);
    assert_eq!(collector.len, 0);
    assert_eq!(err, ERR_UNSUPPORTED_LOCATION);
}

#[test]
fn stackmap_record_unsupported_dwarf_reg_returns_stable_error_code() {
    let sp_reg = dwarf_reg_sp();
    if sp_reg == 0 {
        return;
    }

    let record = build_record_locations(&[LocationSpec {
        loc_type: LOC_DIRECT,
        dwarf_reg: 0,
        offset: 0,
    }]);
    let rec = ScoopStackmapRecordRef {
        record_ptr: record.as_ptr(),
        record_size: record.len() as u32,
        ..Default::default()
    };

    let mut stack = vec![0usize; 8];
    stack[0] = 0x1;
    let sp = stack.as_mut_ptr() as usize;

    let mut slots = [0usize; 4];
    let mut collector = SlotCollector {
        slots: slots.as_mut_ptr(),
        cap: slots.len(),
        len: 0,
    };

    let mut err: u32 = 0;
    let visited = unsafe {
        scoop_stackmap_record_visit_root_slots(
            &rec,
            sp,
            0,
            collect_slot,
            (&mut collector as *mut SlotCollector).cast::<c_void>(),
            &mut err,
        )
    };

    assert_eq!(visited, 0);
    assert_eq!(collector.len, 0);
    assert_eq!(err, ERR_UNSUPPORTED_DWARF_REG);
}
