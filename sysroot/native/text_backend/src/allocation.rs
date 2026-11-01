//! The no_std text backend borrows the target's system heap, without a Rust unwinder.

use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;
#[cfg(not(test))]
use core::panic::PanicInfo;
#[cfg(unix)]
use core::ptr;

struct SystemAllocator;

unsafe extern "C" {
    #[cfg(unix)]
    fn posix_memalign(pointer: *mut *mut c_void, alignment: usize, size: usize) -> i32;
    #[cfg(unix)]
    fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void;
    #[cfg(windows)]
    fn _aligned_malloc(size: usize, alignment: usize) -> *mut c_void;
    #[cfg(windows)]
    fn _aligned_realloc(pointer: *mut c_void, size: usize, alignment: usize) -> *mut c_void;
    #[cfg_attr(windows, link_name = "_aligned_free")]
    fn free(pointer: *mut c_void);
    #[cfg(not(test))]
    fn abort() -> !;
}

// SAFETY: Each operation uses the same system heap and preserves the requested
// Layout alignment. A null result leaves an existing allocation untouched.
unsafe impl GlobalAlloc for SystemAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        #[cfg(unix)]
        {
            let mut result = ptr::null_mut();
            // SAFETY: The output pointer is writable. POSIX additionally requires
            // alignment to be a multiple of pointer size; Layout is a power of two.
            let status = unsafe {
                posix_memalign(
                    &mut result,
                    layout.align().max(size_of::<usize>()),
                    layout.size(),
                )
            };
            if status == 0 {
                result.cast()
            } else {
                ptr::null_mut()
            }
        }
        #[cfg(windows)]
        // SAFETY: Layout supplies a nonzero size and a power-of-two alignment.
        unsafe {
            _aligned_malloc(layout.size(), layout.align()).cast()
        }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, _: Layout) {
        // SAFETY: The caller returns a live allocation from this same allocator.
        unsafe { free(pointer.cast()) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        #[cfg(unix)]
        {
            if layout.align() <= size_of::<usize>() {
                // SAFETY: POSIX allocations can be passed to realloc, whose
                // ordinary alignment suffices for these layouts.
                return unsafe { realloc(pointer.cast(), size).cast() };
            }
            // SAFETY: GlobalAlloc's caller guarantees a valid new size with the
            // old alignment. Overaligned allocations retain their alignment by
            // allocating before copying; failed allocation keeps the old block.
            unsafe {
                let result = self.alloc(Layout::from_size_align_unchecked(size, layout.align()));
                if !result.is_null() {
                    ptr::copy_nonoverlapping(pointer, result, layout.size().min(size));
                    self.dealloc(pointer, layout);
                }
                result
            }
        }
        #[cfg(windows)]
        // SAFETY: This pointer came from aligned_malloc/realloc with this alignment.
        unsafe {
            _aligned_realloc(pointer.cast(), size, layout.align()).cast()
        }
    }
}

#[cfg(not(test))]
#[global_allocator]
static ALLOCATOR: SystemAllocator = SystemAllocator;

#[cfg(not(test))]
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    // SAFETY: abort never returns or unwinds through the C ABI.
    unsafe { abort() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_heap_preserves_alignment_zeroing_and_reallocated_contents() {
        for alignment in [1, 2, 8, 16, 64, 4096] {
            let layout = Layout::from_size_align(37, alignment).unwrap();
            // SAFETY: Each successful allocation is accessed within its size,
            // then released exactly once with the current layout.
            unsafe {
                let pointer = SystemAllocator.alloc_zeroed(layout);
                assert!(!pointer.is_null());
                assert_eq!(pointer as usize % alignment, 0);
                for index in 0..layout.size() {
                    assert_eq!(*pointer.add(index), 0);
                    *pointer.add(index) = index as u8;
                }
                let grown = SystemAllocator.realloc(pointer, layout, 123);
                assert!(!grown.is_null());
                assert_eq!(grown as usize % alignment, 0);
                for index in 0..layout.size() {
                    assert_eq!(*grown.add(index), index as u8);
                }
                let grown_layout = Layout::from_size_align(123, alignment).unwrap();
                let shrunk = SystemAllocator.realloc(grown, grown_layout, 11);
                assert!(!shrunk.is_null());
                assert_eq!(shrunk as usize % alignment, 0);
                for index in 0..11 {
                    assert_eq!(*shrunk.add(index), index as u8);
                }
                SystemAllocator.dealloc(shrunk, Layout::from_size_align(11, alignment).unwrap());
            }
        }
    }
}
