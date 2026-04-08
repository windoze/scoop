// 强制链接本 package 的 `scoop_runtime` crate，确保其 build.rs 输出的 native link args 生效。
use scoop_runtime as _;

use std::mem;
use std::sync::Mutex;

// 注意：同一测试二进制内的多个 `#[test]` 默认会并行执行；而 stackmap registry 是进程全局状态，
// 并行测试会互相干扰（即使各自调用了 reset 也无法保证时序）。
//
// 这里用一个进程内全局锁把相关测试串行化，避免 flaky。
static STACKMAP_REGISTRY_TEST_LOCK: Mutex<()> = Mutex::new(());

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
    fn scoop_runtime_init();

    fn scoop_stackmap_registry_reset();
    fn scoop_stackmap_registry_register_section(bytes: *const u8, len: usize) -> u32;
    fn scoop_stackmap_registry_record_count() -> u32;
    fn scoop_stackmap_registry_lookup(
        return_address: usize,
        out: *mut ScoopStackmapRecordRef,
    ) -> u32;
}

// --- 构造一个“最小可解析”的 stackmap section（用于解析器单测） ---

#[repr(C, packed)]
struct StackMapHeader {
    version: u8,
    reserved0: u8,
    reserved1: u16,
    num_functions: u32,
    num_constants: u32,
    num_records: u32,
}

#[repr(C, packed)]
struct StackSizeRecordU64 {
    function_address: u64,
    stack_size: u64,
    record_count: u64,
}

#[repr(C, packed)]
struct StackMapRecordMin {
    patchpoint_id: u64,
    instruction_offset: u32,
    reserved0: u16,
    num_locations: u16,
    num_live_outs: u16,
    reserved1: u16,
    padding_to_8: u32,
}

#[repr(C, packed)]
struct StackMapSectionMin {
    header: StackMapHeader,
    func: StackSizeRecordU64,
    record: StackMapRecordMin,
}

static MOCK_STACKMAP_SECTION: StackMapSectionMin = StackMapSectionMin {
    header: StackMapHeader {
        version: 3,
        reserved0: 0,
        reserved1: 0,
        num_functions: 1,
        num_constants: 0,
        num_records: 1,
    },
    func: StackSizeRecordU64 {
        function_address: 0x1000,
        stack_size: 0,
        record_count: 1,
    },
    record: StackMapRecordMin {
        patchpoint_id: 1,
        instruction_offset: 0x20,
        reserved0: 0,
        num_locations: 0,
        num_live_outs: 0,
        reserved1: 0,
        padding_to_8: 0,
    },
};

fn build_stackmap_section_bytes(function_address: u64, records: &[(u64, u32)]) -> Vec<u8> {
    let num_records_u32: u32 = records.len().try_into().unwrap();

    let mut bytes = Vec::new();

    // header (16 bytes)
    bytes.push(3); // version
    bytes.push(0); // reserved0
    bytes.extend_from_slice(&0u16.to_le_bytes()); // reserved1
    bytes.extend_from_slice(&1u32.to_le_bytes()); // num_functions
    bytes.extend_from_slice(&0u32.to_le_bytes()); // num_constants
    bytes.extend_from_slice(&num_records_u32.to_le_bytes()); // num_records

    // function record (24 bytes)
    bytes.extend_from_slice(&function_address.to_le_bytes());
    bytes.extend_from_slice(&0u64.to_le_bytes()); // stack_size (unused)
    bytes.extend_from_slice(&(num_records_u32 as u64).to_le_bytes()); // record_count

    // records (each 24 bytes with 0 locations + 0 live-outs)
    for (patchpoint_id, instruction_offset) in records {
        bytes.extend_from_slice(&patchpoint_id.to_le_bytes());
        bytes.extend_from_slice(&instruction_offset.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes()); // reserved0
        bytes.extend_from_slice(&0u16.to_le_bytes()); // num_locations
        bytes.extend_from_slice(&0u16.to_le_bytes()); // num_live_outs
        bytes.extend_from_slice(&0u16.to_le_bytes()); // reserved1
        bytes.extend_from_slice(&0u32.to_le_bytes()); // padding to 8
    }

    bytes
}

#[test]
fn stackmap_registry_can_register_and_lookup_mock_section() {
    let _lock = STACKMAP_REGISTRY_TEST_LOCK.lock().unwrap();
    unsafe {
        scoop_stackmap_registry_reset();

        let bytes = (&MOCK_STACKMAP_SECTION as *const StackMapSectionMin).cast::<u8>();
        let added =
            scoop_stackmap_registry_register_section(bytes, mem::size_of::<StackMapSectionMin>());
        assert!(added > 0, "期望能注册至少 1 条 record，实际为 {added}");
        assert_eq!(scoop_stackmap_registry_record_count(), 1);

        let mut out = ScoopStackmapRecordRef::default();
        let ok = scoop_stackmap_registry_lookup(0x1000 + 0x20, &mut out);
        assert_eq!(ok, 1, "lookup 失败：未找到 return_address 对应 record");
        assert_eq!(out.patchpoint_id, 1);
        assert_eq!(out.instruction_offset, 0x20);
        assert_eq!(out.return_address, 0x1020);
    }
}

#[test]
fn stackmap_registry_lookup_does_not_use_nearest_window_match() {
    let _lock = STACKMAP_REGISTRY_TEST_LOCK.lock().unwrap();
    unsafe {
        scoop_stackmap_registry_reset();

        let base: u64 = 0x1000;
        let section = build_stackmap_section_bytes(base, &[(1, 0x20), (2, 0x30)]);
        let added = scoop_stackmap_registry_register_section(section.as_ptr(), section.len());
        assert_eq!(added, 2);

        // 0x1028 与两条 record 都相距 8 bytes：
        // - 旧实现（A2 前）会在 <=16 bytes 窗口内“挑一个最近的”，存在误配风险；
        // - 新约定：只能命中“确定性归一化集合”中的 key，否则返回未找到。
        let mut out = ScoopStackmapRecordRef::default();
        let ok = scoop_stackmap_registry_lookup(0x1028, &mut out);
        assert_eq!(ok, 0, "不应对非候选地址做 best-effort nearest-match");
    }
}

#[test]
fn stackmap_registry_lookup_supports_plus_minus_one_normalization() {
    let _lock = STACKMAP_REGISTRY_TEST_LOCK.lock().unwrap();
    unsafe {
        scoop_stackmap_registry_reset();

        let base: u64 = 0x1000;
        let section = build_stackmap_section_bytes(base, &[(7, 0x20)]);
        let added = scoop_stackmap_registry_register_section(section.as_ptr(), section.len());
        assert_eq!(added, 1);

        // 约定：允许把 `ra - 1` 作为候选（部分 unwind 实现返回 call 指令内部地址）。
        let ra = (base as usize) + 0x21;
        let mut out = ScoopStackmapRecordRef::default();
        let ok = scoop_stackmap_registry_lookup(ra, &mut out);
        assert_eq!(ok, 1, "期望 lookup 能处理 -1/+1 归一化");
        assert_eq!(out.patchpoint_id, 7);
        assert_eq!(out.instruction_offset, 0x20);
        assert_eq!(
            out.return_address, ra,
            "输出 record 应回填为真实 RA（lookup 输入）"
        );
    }
}

#[test]
fn stackmap_registry_lookup_detects_ambiguity_between_ra_and_ra_minus_one() {
    let _lock = STACKMAP_REGISTRY_TEST_LOCK.lock().unwrap();
    unsafe {
        scoop_stackmap_registry_reset();

        // 设计：对同一个 lookup 输入 ra，`ra` 与 `ra-1` 分别命中不同 records。
        // 该场景下不允许“挑一个最近的”，必须拒绝（无歧义命中）。
        let base: u64 = 0x1000;
        let section = build_stackmap_section_bytes(base, &[(1, 0x20), (2, 0x21)]);
        let added = scoop_stackmap_registry_register_section(section.as_ptr(), section.len());
        assert_eq!(added, 2);

        let ra = (base as usize) + 0x21;
        let mut out = ScoopStackmapRecordRef::default();
        let ok = scoop_stackmap_registry_lookup(ra, &mut out);
        assert_eq!(ok, 0, "存在多个候选 record 时应视为歧义并拒绝命中");
    }
}

// --- “真实链接产物 smoke”：在测试二进制里内嵌一个 `.llvm_stackmaps` section ---
//
// 注意：
// - 在部分 Apple 平台（arm64e）上，函数指针可能带有 pointer authentication（PAC）签名位，
//   “把函数地址当作纯整数” 的做法会变得不稳定。
// - 为了让 smoke test 更稳定，这里内嵌 section 使用“合成的绝对地址常量”，
//   只验证：runtime 能发现 section、能解析 records、能按 return address 查到 record。

const EMBEDDED_SYNTHETIC_RA: usize = 0x2000;

#[repr(C, packed)]
struct StackMapSectionEmbedded {
    header: StackMapHeader,
    func: StackSizeRecordU64,
    record: StackMapRecordMin,
}

#[cfg(target_vendor = "apple")]
#[used]
#[unsafe(link_section = "__LLVM_STACKMAPS,__llvm_stackmaps")]
static EMBEDDED_STACKMAP_SECTION: StackMapSectionEmbedded = StackMapSectionEmbedded {
    header: StackMapHeader {
        version: 3,
        reserved0: 0,
        reserved1: 0,
        num_functions: 1,
        num_constants: 0,
        num_records: 1,
    },
    func: StackSizeRecordU64 {
        function_address: EMBEDDED_SYNTHETIC_RA as u64,
        stack_size: 0,
        record_count: 1,
    },
    record: StackMapRecordMin {
        patchpoint_id: 7,
        instruction_offset: 0,
        reserved0: 0,
        num_locations: 0,
        num_live_outs: 0,
        reserved1: 0,
        padding_to_8: 0,
    },
};

// 注意：使用 `llvm_stackmaps`（无前导点）而非 `.llvm_stackmaps`（LLVM 实际生成的名称）。
// 原因：GNU ld 只为名称是合法 C 标识符的 section 生成 `__start_`/`__stop_` 边界符号；
// `.llvm_stackmaps` 以点开头，不是合法 C 标识符，因此 GNU ld 不会生成边界符号。
// LLD 会自动剥除前导点（`.llvm_stackmaps` → `__start_llvm_stackmaps`），但
// cargo/rustc 在大多数 Linux 发行版上默认使用 GNU ld。
// 此处使用无点名称保证两种链接器都能正确生成边界符号，使 smoke test 稳定通过。
#[cfg(not(target_vendor = "apple"))]
#[used]
#[unsafe(link_section = "llvm_stackmaps")]
static EMBEDDED_STACKMAP_SECTION: StackMapSectionEmbedded = StackMapSectionEmbedded {
    header: StackMapHeader {
        version: 3,
        reserved0: 0,
        reserved1: 0,
        num_functions: 1,
        num_constants: 0,
        num_records: 1,
    },
    func: StackSizeRecordU64 {
        function_address: EMBEDDED_SYNTHETIC_RA as u64,
        stack_size: 0,
        record_count: 1,
    },
    record: StackMapRecordMin {
        patchpoint_id: 7,
        instruction_offset: 0,
        reserved0: 0,
        num_locations: 0,
        num_live_outs: 0,
        reserved1: 0,
        padding_to_8: 0,
    },
};

#[test]
fn runtime_init_registers_stackmaps_from_current_process() {
    let _lock = STACKMAP_REGISTRY_TEST_LOCK.lock().unwrap();
    // 当前实现：
    // - macOS：通过 dyld + getsectiondata* 扫描已加载 images
    // - ELF：尝试 `__start_llvm_stackmaps`/`__stop_llvm_stackmaps`（weak）
    // - Windows：暂未实现自动发现
    //
    // 因此本 smoke test 仅在“已实现自动发现”的平台上强制断言。
    if !(cfg!(target_vendor = "apple") || cfg!(target_os = "linux")) {
        return;
    }

    unsafe {
        scoop_stackmap_registry_reset();
        scoop_runtime_init();

        let n = scoop_stackmap_registry_record_count();
        assert!(
            n > 0,
            "runtime_init 后 stackmap registry 仍为空（records={n}）"
        );

        let ra = EMBEDDED_SYNTHETIC_RA;
        let mut out = ScoopStackmapRecordRef::default();
        let ok = scoop_stackmap_registry_lookup(ra, &mut out);
        assert_eq!(ok, 1, "lookup 失败：未找到内嵌 stackmap 的目标函数 record");
        assert_eq!(out.function_address, ra);
        assert_eq!(out.return_address, ra);
        assert_eq!(out.patchpoint_id, 7);
    }
}
