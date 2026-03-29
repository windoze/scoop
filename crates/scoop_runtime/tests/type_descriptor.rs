// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;
use core::mem;
use core::ptr;

type ScoopGcTraceVisitor = unsafe extern "C" fn(slot: *mut *mut c_void, ctx: *mut c_void);
type ScoopTypeTraceFn =
    Option<unsafe extern "C" fn(object: *mut c_void, visitor: ScoopGcTraceVisitor, ctx: *mut c_void) -> u64>;
type ScoopTypeReleaseFn = Option<unsafe extern "C" fn(object: *mut c_void)>;

// 对齐 `runtime/c/scoop_gc.h` 的 `ScoopTypeDescriptor`（TODO T0907）。
#[repr(C)]
struct ScoopTypeDescriptor {
    abi_version: u32,
    flags: u32,
    size_bytes: u64,
    trace_start_offset_bytes: u64,
    trace_bitmap_u64_len: u32,
    _reserved_u32: u32,
    trace_bitmap: *const u64,
    trace_fn: ScoopTypeTraceFn,
    release_fn: ScoopTypeReleaseFn,
}

#[repr(C)]
struct VisitCtx {
    values: [*mut c_void; 16],
    len: usize,
}

unsafe extern "C" fn collect_slot_value(slot: *mut *mut c_void, ctx: *mut c_void) {
    if slot.is_null() || ctx.is_null() {
        return;
    }

    let ctx = unsafe { &mut *(ctx as *mut VisitCtx) };
    if ctx.len >= ctx.values.len() {
        return;
    }

    // `slot` 指向对象内的某个指针槽位；这里记录“槽位里存放的指针值”，用于断言扫描结果。
    ctx.values[ctx.len] = unsafe { *slot };
    ctx.len += 1;
}

unsafe extern "C" {
    fn scoop_runtime_init();

    fn scoop_gc_type_descriptor_trace(
        type_desc: *const ScoopTypeDescriptor,
        object: *mut c_void,
        visitor: ScoopGcTraceVisitor,
        ctx: *mut c_void,
    ) -> u64;
}

#[test]
fn type_descriptor_trace_bitmap_only_visits_marked_slots() {
    unsafe {
        scoop_runtime_init();

        let mut a = 1u8;
        let mut b = 2u8;

        // 假设对象布局为 4 个 word（指针大小），其中 word0 与 word2 是引用字段。
        let mut object_words: [usize; 4] = [
            (&mut a as *mut u8) as usize,
            0xDEAD_BEEFu64 as usize,
            (&mut b as *mut u8) as usize,
            0,
        ];

        // 只标记 word0 与 word2（bit0 与 bit2）。
        let bitmap: [u64; 1] = [0b0101];

        let desc = ScoopTypeDescriptor {
            abi_version: 0,
            flags: 0,
            size_bytes: (object_words.len() * mem::size_of::<usize>()) as u64,
            trace_start_offset_bytes: 0,
            trace_bitmap_u64_len: bitmap.len() as u32,
            _reserved_u32: 0,
            trace_bitmap: bitmap.as_ptr(),
            trace_fn: None,
            release_fn: None,
        };

        let mut ctx = VisitCtx {
            values: [ptr::null_mut(); 16],
            len: 0,
        };

        let visited = scoop_gc_type_descriptor_trace(
            &desc,
            object_words.as_mut_ptr().cast::<c_void>(),
            collect_slot_value,
            (&mut ctx as *mut VisitCtx).cast::<c_void>(),
        );

        assert_eq!(visited, 2);
        assert_eq!(ctx.len, 2);
        assert_eq!(ctx.values[0], (&mut a as *mut u8).cast::<c_void>());
        assert_eq!(ctx.values[1], (&mut b as *mut u8).cast::<c_void>());
    }
}

#[cfg(unix)]
mod unix_guard_page {
    use super::*;

    #[cfg(any(target_os = "linux", target_os = "android"))]
    const MAP_ANON_FLAG: i32 = libc::MAP_ANONYMOUS;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const MAP_ANON_FLAG: i32 = libc::MAP_ANON;

    struct GuardedMmap {
        base: *mut u8,
        len: usize,
        page_size: usize,
    }

    impl GuardedMmap {
        unsafe fn new_two_pages_with_guard() -> Self {
            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
            assert!(page_size > 0, "sysconf(_SC_PAGESIZE) failed");
            let page_size = page_size as usize;
            let len = page_size * 2;

            let addr = unsafe { libc::mmap(
                ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | MAP_ANON_FLAG,
                -1,
                0,
            ) };
            assert_ne!(addr, libc::MAP_FAILED, "mmap failed");

            let base = addr as *mut u8;
            let guard_addr = unsafe { base.add(page_size) };
            let rc = unsafe { libc::mprotect(guard_addr as *mut c_void, page_size, libc::PROT_NONE) };
            assert_eq!(rc, 0, "mprotect(PROT_NONE) failed");

            Self {
                base,
                len,
                page_size,
            }
        }
    }

    impl Drop for GuardedMmap {
        fn drop(&mut self) {
            unsafe {
                let _ = libc::munmap(self.base as *mut c_void, self.len);
            }
        }
    }

    #[test]
    fn type_descriptor_trace_bitmap_does_not_read_past_object_size() {
        unsafe {
            scoop_runtime_init();

            let mmap = GuardedMmap::new_two_pages_with_guard();
            let word_size = mem::size_of::<usize>();
            let object_words = 3usize;
            let object_size = object_words * word_size;
            assert!(object_size < mmap.page_size);

            // 把对象放在第一个 page 的末尾；第二个 page 设为 PROT_NONE。
            // 若 trace 逻辑错误（按 bitmap 扫描超过 object_size），则会触碰 guard page 并崩溃。
            let object_base = mmap.base.add(mmap.page_size - object_size);

            let mut a = 1u8;
            let mut b = 2u8;
            ptr::write(object_base as *mut usize, (&mut a as *mut u8) as usize);
            ptr::write(object_base.add(word_size) as *mut usize, 0usize);
            ptr::write(object_base.add(2 * word_size) as *mut usize, (&mut b as *mut u8) as usize);

            // bitmap 故意远大于对象大小：若实现按 bitmap 位数遍历，而不是按 size_bytes 裁剪，
            // 将导致越界访问并触碰 guard page。
            let bitmap: [u64; 2] = [u64::MAX, u64::MAX];
            let desc = ScoopTypeDescriptor {
                abi_version: 0,
                flags: 0,
                size_bytes: object_size as u64,
                trace_start_offset_bytes: 0,
                trace_bitmap_u64_len: bitmap.len() as u32,
                _reserved_u32: 0,
                trace_bitmap: bitmap.as_ptr(),
                trace_fn: None,
                release_fn: None,
            };

            let mut ctx = VisitCtx {
                values: [ptr::null_mut(); 16],
                len: 0,
            };

            let visited = scoop_gc_type_descriptor_trace(
                &desc,
                object_base.cast::<c_void>(),
                collect_slot_value,
                (&mut ctx as *mut VisitCtx).cast::<c_void>(),
            );

            assert_eq!(visited, object_words as u64);
            assert_eq!(ctx.len, object_words);
            assert_eq!(ctx.values[0], (&mut a as *mut u8).cast::<c_void>());
            assert_eq!(ctx.values[1], ptr::null_mut());
            assert_eq!(ctx.values[2], (&mut b as *mut u8).cast::<c_void>());
        }
    }
}
