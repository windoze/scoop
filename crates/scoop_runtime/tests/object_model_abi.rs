// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::mem;

type ScoopGcTraceVisitor = unsafe extern "C" fn(slot: *mut *mut c_void, ctx: *mut c_void);
type ScoopTypeTraceFn = Option<
    unsafe extern "C" fn(
        object: *mut c_void,
        visitor: ScoopGcTraceVisitor,
        ctx: *mut c_void,
    ) -> u64,
>;
type ScoopTypeReleaseFn = Option<unsafe extern "C" fn(object: *mut c_void)>;

// 对齐 `runtime/c/scoop_gc.h` 的 `ScoopTypeDescriptor`（T1501：固化对象模型 ABI）。
#[repr(C)]
struct ScoopTypeDescriptor {
    abi_version: u32,
    flags: u32,
    size_bytes: u64,
    align_bytes: u64,
    trace_start_offset_bytes: u64,
    trace_bitmap_u64_len: u32,
    _reserved_u32: u32,
    trace_bitmap: *const u64,
    trace_fn: ScoopTypeTraceFn,
    release_fn: ScoopTypeReleaseFn,
    type_id: u64,
    parent_type_desc: *const ScoopTypeDescriptor,
    itable: *const c_void,
    vtable: *const c_void,
}

// 对齐 `runtime/c/scoop_gc.h` 的 `ScoopGcObjectHeader`（T1501：固化对象头 ABI）。
#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const ScoopTypeDescriptor,
    size_bytes: u64,
    flags: u32,
    mark: u32,
}

fn align_up(value: usize, align: usize) -> usize {
    assert!(
        align.is_power_of_two(),
        "align must be power-of-two: {align}"
    );
    (value + (align - 1)) & !(align - 1)
}

#[test]
fn object_header_layout_matches_spec() {
    let ptr_size = mem::size_of::<*const c_void>();
    assert!(
        ptr_size == 4 || ptr_size == 8,
        "unexpected ptr size: {ptr_size}"
    );

    assert_eq!(mem::offset_of!(ScoopGcObjectHeader, next), 0);
    assert_eq!(mem::offset_of!(ScoopGcObjectHeader, type_desc), ptr_size);
    assert_eq!(
        mem::offset_of!(ScoopGcObjectHeader, size_bytes),
        2 * ptr_size
    );
    assert_eq!(
        mem::offset_of!(ScoopGcObjectHeader, flags),
        2 * ptr_size + 8
    );
    assert_eq!(
        mem::offset_of!(ScoopGcObjectHeader, mark),
        2 * ptr_size + 8 + 4
    );

    let header_size = mem::size_of::<ScoopGcObjectHeader>();
    assert_eq!(
        header_size % ptr_size,
        0,
        "object header size must be pointer-aligned"
    );
}

#[test]
fn type_descriptor_layout_matches_spec() {
    // 固定宽度字段的偏移（不依赖目标指针宽度）。
    assert_eq!(mem::offset_of!(ScoopTypeDescriptor, abi_version), 0);
    assert_eq!(mem::offset_of!(ScoopTypeDescriptor, flags), 4);
    assert_eq!(mem::offset_of!(ScoopTypeDescriptor, size_bytes), 8);
    assert_eq!(mem::offset_of!(ScoopTypeDescriptor, align_bytes), 16);
    assert_eq!(
        mem::offset_of!(ScoopTypeDescriptor, trace_start_offset_bytes),
        24
    );
    assert_eq!(
        mem::offset_of!(ScoopTypeDescriptor, trace_bitmap_u64_len),
        32
    );
    assert_eq!(mem::offset_of!(ScoopTypeDescriptor, trace_bitmap), 40);

    // 指针/函数指针字段的偏移：按当前目标的 ABI 计算。
    let ptr_align = mem::align_of::<*const c_void>();
    let ptr_size = mem::size_of::<*const c_void>();

    let mut offset = 40 + ptr_size; // trace_bitmap 结束后的位置
    offset = align_up(offset, mem::align_of::<ScoopTypeTraceFn>());
    assert_eq!(mem::offset_of!(ScoopTypeDescriptor, trace_fn), offset);
    offset += mem::size_of::<ScoopTypeTraceFn>();

    offset = align_up(offset, mem::align_of::<ScoopTypeReleaseFn>());
    assert_eq!(mem::offset_of!(ScoopTypeDescriptor, release_fn), offset);
    offset += mem::size_of::<ScoopTypeReleaseFn>();

    offset = align_up(offset, mem::align_of::<u64>());
    assert_eq!(mem::offset_of!(ScoopTypeDescriptor, type_id), offset);
    offset += mem::size_of::<u64>();

    offset = align_up(offset, ptr_align);
    assert_eq!(
        mem::offset_of!(ScoopTypeDescriptor, parent_type_desc),
        offset
    );
    offset += ptr_size;

    offset = align_up(offset, ptr_align);
    assert_eq!(mem::offset_of!(ScoopTypeDescriptor, itable), offset);
    offset += ptr_size;

    offset = align_up(offset, ptr_align);
    assert_eq!(mem::offset_of!(ScoopTypeDescriptor, vtable), offset);

    // 结构体整体必须保持指针对齐（便于把 descriptor 放进只保证 pointer-aligned 的只读段/常量池）。
    let desc_size = mem::size_of::<ScoopTypeDescriptor>();
    assert_eq!(
        desc_size % ptr_align,
        0,
        "type descriptor size must be pointer-aligned"
    );
}
