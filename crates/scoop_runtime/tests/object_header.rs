// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use core::ffi::c_void;

#[repr(C)]
struct ScoopGcObjectHeader {
    next: *mut ScoopGcObjectHeader,
    type_desc: *const c_void,
    size_bytes: u64,
    flags: u32,
    mark: u32,
}

unsafe extern "C" {
    fn scoop_runtime_init();
    fn scoop_alloc(size: u64) -> *mut c_void;
    fn scoop_gc_collect();
}

#[test]
fn scoop_alloc_initializes_object_header_and_alignment_is_sane() {
    unsafe {
        scoop_runtime_init();

        let header_size = core::mem::size_of::<ScoopGcObjectHeader>();
        let header_align = core::mem::align_of::<ScoopGcObjectHeader>();
        assert!(header_size > 0, "object header size must be non-zero");
        assert!(header_align > 0, "object header align must be non-zero");

        let total_size = (header_size + 16) as u64;
        let p = scoop_alloc(total_size);
        assert!(!p.is_null(), "scoop_alloc must return non-null");
        assert_eq!(
            (p as usize) % header_align,
            0,
            "allocated object must satisfy header alignment"
        );

        let hdr = &mut *(p as *mut ScoopGcObjectHeader);

        // T0908：runtime 应初始化对象头字段，确保它们可被稳定观测/调试。
        assert!(hdr.next.is_null(), "header.next should be NULL");
        assert!(
            hdr.type_desc.is_null(),
            "header.type_desc should be NULL (v0)"
        );
        assert_eq!(
            hdr.size_bytes, total_size,
            "header.size_bytes should equal alloc size"
        );
        assert_eq!(hdr.flags, 0, "header.flags should default to 0");
        assert_eq!(hdr.mark, 0, "header.mark should default to 0");

        // header 字段可写回。
        hdr.flags = 0xA5A5_1234;
        hdr.mark = 0xDEAD_BEEF;
        assert_eq!(hdr.flags, 0xA5A5_1234);
        assert_eq!(hdr.mark, 0xDEAD_BEEF);

        // payload 紧随 header 之后；这里做一次最小写入以验证“不会写到 header 内”。
        let payload = (p as *mut u8).add(header_size);
        *payload = 0xEE;
        *payload.add(15) = 0xFF;

        // T0910：alloc 的对象由 GC 管理；本测试不写入 shadow stack roots，因此 collect 会回收该对象。
        scoop_gc_collect();
    }
}
