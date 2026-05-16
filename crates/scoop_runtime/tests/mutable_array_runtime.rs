// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::mem;
use core::ptr;

type ScoopGcTraceVisitor = unsafe extern "C" fn(slot: *mut *mut c_void, ctx: *mut c_void);
type ScoopCompositeTraceFn = Option<
    unsafe extern "C" fn(
        descriptor: *const ScoopCompositeTransportDescriptor,
        value: *mut c_void,
        visitor: ScoopGcTraceVisitor,
        ctx: *mut c_void,
    ) -> u64,
>;
type ScoopCompositeCopyFn = Option<
    unsafe extern "C" fn(
        descriptor: *const ScoopCompositeTransportDescriptor,
        dst: *mut c_void,
        src: *const c_void,
    ),
>;
type ScoopCompositeDropFn = Option<
    unsafe extern "C" fn(descriptor: *const ScoopCompositeTransportDescriptor, value: *mut c_void),
>;

const ARRAY_ELEM_KIND_WORD: u32 = 1;
const ARRAY_ELEM_KIND_REF: u32 = 2;
const ARRAY_ELEM_KIND_COMPOSITE: u32 = 3;

#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const c_void,
    size_bytes: u64,
    flags: u32,
    mark: u32,
}

#[repr(C)]
struct ScoopCompositeTransportDescriptor {
    abi_version: u32,
    storage_kind: u32,
    size_bytes: u64,
    align_bytes: u64,
    gc_slot_offsets: *const u64,
    gc_slot_count: u32,
    _reserved_u32: u32,
    trace_fn: ScoopCompositeTraceFn,
    copy_fn: ScoopCompositeCopyFn,
    drop_fn: ScoopCompositeDropFn,
    type_desc: *const c_void,
}

#[repr(C)]
struct ScoopMutableArray {
    header: ScoopGcObjectHeader,
    len: u64,
    cap: u64,
    elem_size_bytes: u64,
    elem_align_bytes: u64,
    elem_desc: *const ScoopCompositeTransportDescriptor,
    data: *mut u8,
    elem_kind: u32,
    _reserved_u32: u32,
}

#[repr(C)]
struct ScoopArray {
    header: ScoopGcObjectHeader,
    len: u64,
    elem_size_bytes: u64,
    data_offset_bytes: u64,
    elem_desc: *const ScoopCompositeTransportDescriptor,
    elem_kind: u32,
    _reserved_u32: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Pair {
    x: u64,
    y: u64,
}

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_thread_register();
    fn scoop_thread_unregister();

    fn scoop_mutable_array_new(
        elem_kind: u32,
        elem_size: u64,
        elem_align: u64,
        elem_desc: *const c_void,
        capacity: u64,
    ) -> *mut c_void;
    fn scoop_mutable_array_push_word(arr: *mut c_void, value: u64);
    fn scoop_mutable_array_push_ref(arr: *mut c_void, value: *mut c_void);
    fn scoop_mutable_array_push_composite(
        arr: *mut c_void,
        slot_ptr: *const c_void,
        elem_size: u64,
    );
    fn scoop_mutable_array_freeze(arr: *mut c_void) -> *mut c_void;
    fn scoop_mutable_array_to_array_data(arr: *mut c_void) -> *const c_void;
}

struct RuntimeThread;

impl RuntimeThread {
    fn enter() -> Self {
        unsafe {
            scoop_runtime_init();
            scoop_thread_register();
        }
        Self
    }
}

impl Drop for RuntimeThread {
    fn drop(&mut self) {
        unsafe {
            scoop_thread_unregister();
        }
    }
}

#[test]
fn mutable_array_new_creates_with_capacity() {
    let _thread = RuntimeThread::enter();

    unsafe {
        let arr = scoop_mutable_array_new(ARRAY_ELEM_KIND_WORD, 8, 8, ptr::null(), 0)
            as *mut ScoopMutableArray;
        assert!(!arr.is_null());
        assert_eq!((*arr).len, 0);
        assert_eq!((*arr).cap, 4);
        assert_eq!((*arr).elem_size_bytes, mem::size_of::<usize>() as u64);
        assert_eq!((*arr).elem_align_bytes, mem::align_of::<usize>() as u64);
        assert!(!(*arr).data.is_null());
        assert_eq!((*arr).elem_kind, ARRAY_ELEM_KIND_WORD);
    }
}

#[test]
fn mutable_array_push_word_grows_amortized() {
    let _thread = RuntimeThread::enter();

    unsafe {
        let arr = scoop_mutable_array_new(ARRAY_ELEM_KIND_WORD, 8, 8, ptr::null(), 0)
            as *mut ScoopMutableArray;
        assert!(!arr.is_null());

        for i in 0..1024u64 {
            scoop_mutable_array_push_word(arr.cast::<c_void>(), i);
        }

        assert_eq!((*arr).len, 1024);
        assert_eq!((*arr).cap, 1024);
        let data = (*arr).data.cast::<u64>();
        assert_eq!(*data.add(0), 0);
        assert_eq!(*data.add(17), 17);
        assert_eq!(*data.add(1023), 1023);
    }
}

#[test]
fn mutable_array_freeze_yields_correct_inline_array() {
    let _thread = RuntimeThread::enter();

    unsafe {
        let mutable = scoop_mutable_array_new(ARRAY_ELEM_KIND_WORD, 8, 8, ptr::null(), 4)
            as *mut ScoopMutableArray;
        assert!(!mutable.is_null());
        for value in [10u64, 20, 30] {
            scoop_mutable_array_push_word(mutable.cast::<c_void>(), value);
        }

        let frozen = scoop_mutable_array_freeze(mutable.cast::<c_void>()) as *mut ScoopArray;
        assert!(!frozen.is_null());
        assert_eq!((*frozen).len, 3);
        assert_eq!((*frozen).elem_kind, ARRAY_ELEM_KIND_WORD);

        let frozen_data = (frozen.cast::<u8>())
            .add((*frozen).data_offset_bytes as usize)
            .cast::<u64>();
        assert_eq!(*frozen_data.add(0), 10);
        assert_eq!(*frozen_data.add(1), 20);
        assert_eq!(*frozen_data.add(2), 30);

        scoop_mutable_array_push_word(mutable.cast::<c_void>(), 40);
        assert_eq!((*mutable).len, 4);
        assert_eq!((*frozen).len, 3);
        assert_eq!(*frozen_data.add(2), 30);
    }
}

#[test]
fn mutable_array_push_ref_and_composite_store_out_of_line_data() {
    let _thread = RuntimeThread::enter();

    unsafe {
        let refs = scoop_mutable_array_new(
            ARRAY_ELEM_KIND_REF,
            mem::size_of::<usize>() as u64,
            mem::align_of::<usize>() as u64,
            ptr::null(),
            1,
        ) as *mut ScoopMutableArray;
        assert!(!refs.is_null());
        let sentinel = ptr::null_mut();
        scoop_mutable_array_push_ref(refs.cast::<c_void>(), sentinel);
        assert_eq!((*refs).len, 1);
        assert_eq!(*(*refs).data.cast::<*mut c_void>(), sentinel);

        let descriptor = ScoopCompositeTransportDescriptor {
            abi_version: 0,
            storage_kind: 0,
            size_bytes: mem::size_of::<Pair>() as u64,
            align_bytes: mem::align_of::<Pair>() as u64,
            gc_slot_offsets: ptr::null(),
            gc_slot_count: 0,
            _reserved_u32: 0,
            trace_fn: None,
            copy_fn: None,
            drop_fn: None,
            type_desc: ptr::null(),
        };
        let composites = scoop_mutable_array_new(
            ARRAY_ELEM_KIND_COMPOSITE,
            mem::size_of::<Pair>() as u64,
            mem::align_of::<Pair>() as u64,
            (&descriptor as *const ScoopCompositeTransportDescriptor).cast::<c_void>(),
            1,
        ) as *mut ScoopMutableArray;
        assert!(!composites.is_null());
        let value = Pair { x: 7, y: 9 };
        scoop_mutable_array_push_composite(
            composites.cast::<c_void>(),
            (&value as *const Pair).cast::<c_void>(),
            mem::size_of::<Pair>() as u64,
        );
        assert_eq!((*composites).len, 1);
        let data = scoop_mutable_array_to_array_data(composites.cast::<c_void>()).cast::<Pair>();
        assert_eq!(*data, value);
    }
}
