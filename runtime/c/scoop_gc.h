// Scoop GC runtime (early stage).
//
// 说明：
// - 该文件提供 mark-sweep GC（v0）的“数据结构骨架”，用于后续逐步接入：
//   - type descriptor（对象内引用字段扫描）
//   - stop-the-world 与多线程支持
// - 在完成 TODO T0910 后，该文件也提供最小可用的单线程 mark-sweep（手动触发）。

#ifndef SCOOP_GC_H
#define SCOOP_GC_H

#include <stdint.h>
#include <stddef.h>

// --- Type descriptor / Object Model ABI（T0907/T0920/T1501） ---
//
// 说明：
// - `ScoopTypeDescriptor` 描述一个 heap 对象的布局信息与运行期元数据：
//   - 对象大小/对齐（分配与健壮性裁剪）
//   - 引用字段（GC-managed pointers）的扫描规则（trace bitmap 或自定义回调）
//   - 可选 release 回调（用于 FFI-managed 资源释放）
//   - RTTI/type id（为 `is/as/as?` 与动态分发做准备）
//   - vtable/itable 指针（TODO T15：class/interface 动态分发）
// - 该结构体的字段顺序与关键偏移会被 Rust/LLVM codegen 与集成测试依赖，因此需要在
//   演进时保持可审计（见 `SCOOP_RUNTIME.md`）。

// 扫描回调：`slot` 指向对象内部某个“指针槽位”（可读写）。
typedef void (*ScoopGcTraceVisitor)(void **slot, void *ctx);

// 自定义 trace 函数：当 bitmap 无法表达（例如复杂容器/变长布局）时使用。
// 返回值为调用 visitor 的次数。
typedef uint64_t (*ScoopTypeTraceFn)(void *object, ScoopGcTraceVisitor visitor, void *ctx);

// release 回调：当对象在 sweep 阶段被回收（free）前调用。
//
// 说明（early stage 语义约定）：
// - 这不是通用 finalizer：不保证顺序，不允许对象复活，不应依赖其它对象仍存活；
// - callback 运行在 GC 的受限上下文中（实现上通常持有 GC 锁 + stop-the-world）：
//   - 应避免分配；
//   - 应避免调用可能触发 GC 或持有同一把锁的 runtime API；
//   - 不应对对象内存本身做 free（对象内存由 GC 负责释放）。
// - `object` 指向 heap 对象的起始地址（即 `ScoopGcObjectHeader*`）。
typedef void (*ScoopTypeReleaseFn)(void *object);

typedef struct ScoopTypeDescriptor {
  // ABI 版本（预留）：便于后续演进时做兼容分支；v0 固定为 0。
  uint32_t abi_version;

  // 预留 flags：例如“是否有尾随变长 payload”等（v0 未定义语义）。
  uint32_t flags;

  // 对象在内存中的总大小（字节）。
  uint64_t size_bytes;

  // 对象的对齐（字节；必须为 2 的幂，且 >= sizeof(void*)）。
  uint64_t align_bytes;

  // 从 `object` 起始地址偏移多少字节开始扫描引用字段。
  // 说明：该偏移必须是指针对齐（`sizeof(void*)` 的倍数）；否则 v0 直接跳过扫描。
  uint64_t trace_start_offset_bytes;

  // `trace_bitmap` 的长度（单位：u64 word）。
  // 每个 bit 表示一个指针 word 是否为“引用槽位”：
  // - bit 0 表示 `trace_start_offset_bytes + 0*sizeof(void*)`
  // - bit 1 表示 `trace_start_offset_bytes + 1*sizeof(void*)`
  // - ...
  uint32_t trace_bitmap_u64_len;

  // 预留字段：用于对齐/未来扩展。
  uint32_t _reserved_u32;

  // trace bitmap：可为 NULL（表示无引用字段，或使用 trace_fn）。
  const uint64_t *trace_bitmap;

  // 自定义 trace 回调：可为 NULL（表示使用 trace_bitmap）。
  ScoopTypeTraceFn trace_fn;

  // 可选 release 回调：对象被 sweep/free 前调用（用于 FFI-managed 资源释放）。
  // 可为 NULL（表示无需释放）。
  ScoopTypeReleaseFn release_fn;

  // 运行期类型 ID（与编译器 `TypeId` 对齐；通常为稳定 hash64）。
  // v0：runtime 仅存储该字段，不强制解释其语义；`is/as/as?` 与动态分发留给 TODO T15。
  uint64_t type_id;

  // 父类型（class 继承链）的 type descriptor；无父类型则为 NULL。
  const struct ScoopTypeDescriptor *parent_type_desc;

  // interface dispatch table（itable）：布局由后续任务固化；无则为 NULL。
  const void *itable;

  // class virtual dispatch table（vtable）：布局由后续任务固化；无则为 NULL。
  const void *vtable;
} ScoopTypeDescriptor;

// --- Composite value transport descriptor（CG-T04a） ---
//
// 说明：
// - 该 descriptor 描述“不是单个机器字”的值跨 runtime/codegen 边界时需要的物理布局；
// - 后续 boxed value、enum payload、array element、closure env 与跨线程 payload 共用该表面；
// - copy/drop hooks 可为 NULL，表示使用 runtime 的默认 memcpy / no-op drop；traceable value 必须
//   通过 gc_slot_offsets、type_desc 或 trace_fn 提供真实 trace surface，不能用假 no-op trace 混过。
typedef struct ScoopCompositeTransportDescriptor ScoopCompositeTransportDescriptor;

typedef uint64_t (*ScoopCompositeTraceFn)(
    const ScoopCompositeTransportDescriptor *descriptor,
    void *value,
    ScoopGcTraceVisitor visitor,
    void *ctx);
typedef void (*ScoopCompositeCopyFn)(
    const ScoopCompositeTransportDescriptor *descriptor,
    void *dst,
    const void *src);
typedef void (*ScoopCompositeDropFn)(
    const ScoopCompositeTransportDescriptor *descriptor,
    void *value);

struct ScoopCompositeTransportDescriptor {
  uint32_t abi_version;
  uint32_t storage_kind;
  uint64_t size_bytes;
  uint64_t align_bytes;
  const uint64_t *gc_slot_offsets;
  uint32_t gc_slot_count;
  uint32_t _reserved_u32;
  ScoopCompositeTraceFn trace_fn;
  ScoopCompositeCopyFn copy_fn;
  ScoopCompositeDropFn drop_fn;
  const ScoopTypeDescriptor *type_desc;
};

uint64_t scoop_composite_trace(
    const ScoopCompositeTransportDescriptor *descriptor,
    void *value,
    ScoopGcTraceVisitor visitor,
    void *ctx);
void scoop_composite_copy(
    const ScoopCompositeTransportDescriptor *descriptor,
    void *dst,
    const void *src);
void scoop_composite_drop(
    const ScoopCompositeTransportDescriptor *descriptor,
    void *value);

// ABI 断言：固化 type descriptor 的关键字段偏移，避免在演进中“悄悄漂移”。
//
// 说明：
// - 这里只断言一组最关键的字段（GC 扫描/分配依赖），其余字段会在 `crates/scoop_runtime`
//   的集成测试中覆盖。
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(offsetof(ScoopTypeDescriptor, abi_version) == 0,
               "ScoopTypeDescriptor.abi_version offset must be 0");
_Static_assert(offsetof(ScoopTypeDescriptor, flags) == 4,
               "ScoopTypeDescriptor.flags offset must be 4");
_Static_assert(offsetof(ScoopTypeDescriptor, size_bytes) == 8,
               "ScoopTypeDescriptor.size_bytes offset must be 8");
_Static_assert(offsetof(ScoopTypeDescriptor, align_bytes) == 16,
               "ScoopTypeDescriptor.align_bytes offset must be 16");
_Static_assert(offsetof(ScoopTypeDescriptor, trace_start_offset_bytes) == 24,
               "ScoopTypeDescriptor.trace_start_offset_bytes offset must be 24");
_Static_assert(offsetof(ScoopTypeDescriptor, trace_bitmap_u64_len) == 32,
               "ScoopTypeDescriptor.trace_bitmap_u64_len offset must be 32");
_Static_assert(offsetof(ScoopTypeDescriptor, trace_bitmap) == 40,
               "ScoopTypeDescriptor.trace_bitmap offset must be 40");
#endif

// GC 对象头（v0：骨架）。
//
// 说明：
// - 该结构体用于定义“heap 对象的内存前缀（header）”：
//   - `scoop_alloc` 返回的指针指向 header 起始地址；
//   - 对象的 payload（用户字段/box payload 等）紧随 header 之后；
//   - type descriptor 的 `trace_start_offset_bytes` 可设置为 `sizeof(ScoopGcObjectHeader)`
//     以跳过 header，从 payload 的引用字段开始扫描（见 TODO T0907）。
// - 该布局仍处于早期阶段，但我们会用 `_Static_assert` 固定关键字段偏移，便于
//   Rust/LLVM codegen 在同一仓库内做一致性假设（TODO T0908）。
typedef struct ScoopGcObjectHeader {
  // heap 链表：用于 sweep 阶段遍历所有已分配对象。
  struct ScoopGcObjectHeader *next;

  // 指向对象的类型描述；用于扫描对象内部引用字段（TODO T0907）。
  const ScoopTypeDescriptor *type_desc;

  // 对象总大小（可包含 header + payload；具体约定由后续任务固定）。
  uint64_t size_bytes;

  // flags/mark bits（占位；后续可拆分为更紧凑的 bitfield）。
  uint32_t flags;
  uint32_t mark;
} ScoopGcObjectHeader;

// ABI 断言：固定对象头关键字段的相对偏移，避免在后续演进中“悄悄漂移”。
//
// 说明：
// - 这些断言旨在覆盖仓库当前支持的主流平台（64-bit/32-bit）；若未来需要支持更
//   特殊的 ABI，可在引入 target-aware layout（TODO T0803）时再做细化。
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(offsetof(ScoopGcObjectHeader, next) == 0,
               "ScoopGcObjectHeader.next offset must be 0");
_Static_assert(offsetof(ScoopGcObjectHeader, type_desc) == sizeof(void *),
               "ScoopGcObjectHeader.type_desc offset must be sizeof(void*)");
_Static_assert(offsetof(ScoopGcObjectHeader, size_bytes) == (2u * sizeof(void *)),
               "ScoopGcObjectHeader.size_bytes offset must be 2*sizeof(void*)");
_Static_assert(
    offsetof(ScoopGcObjectHeader, flags) ==
        (2u * sizeof(void *) + sizeof(uint64_t)),
    "ScoopGcObjectHeader.flags offset must be 2*sizeof(void*) + sizeof(u64)");
_Static_assert(
    offsetof(ScoopGcObjectHeader, mark) ==
        (2u * sizeof(void *) + sizeof(uint64_t) + sizeof(uint32_t)),
    "ScoopGcObjectHeader.mark offset must be flags+sizeof(u32)");
_Static_assert((sizeof(ScoopGcObjectHeader) % sizeof(void *)) == 0,
               "ScoopGcObjectHeader size must be pointer-aligned");
#endif

// 计算对象 payload 的起始指针（紧随 header 之后）。
//
// 注意：这是一个便捷宏；对象的实际字段布局由编译器按类型决定。
#define SCOOP_GC_OBJECT_PAYLOAD_PTR(object) \
  ((void *)((uint8_t *)(object) + sizeof(ScoopGcObjectHeader)))

// Free list 节点（v0：骨架）。
//
// 说明：
// - mark-sweep 通常会把被回收的块串成 free list 以复用；
// - 早期实现可先完全依赖 libc malloc/free，但仍提前定义结构，便于后续替换。
typedef struct ScoopGcFreeBlock {
  struct ScoopGcFreeBlock *next;
  uint64_t size;
} ScoopGcFreeBlock;

// GC heap（v0：骨架）。
//
// 说明：
// - 早期阶段把 heap 视为进程全局（单例）；多线程 stop-the-world 后续补齐（TODO T0911）。
// - 统计字段用于测试/观测（例如 verify sweep 是否回收）。
typedef struct ScoopGcHeap {
  ScoopGcObjectHeader *objects;
  ScoopGcFreeBlock *free_list;

  uint64_t bytes_allocated;
  uint64_t bytes_freed;
  uint64_t gc_cycles;
} ScoopGcHeap;

// 初始化 heap 结构（不分配任何内存）。
void scoop_gc_heap_init(ScoopGcHeap *heap);

// 手动触发一次 mark-sweep GC（v0：单线程）。
//
// 说明：
// - 该 API 当前用于 fixtures/集成测试回归（TODO T0910），不实现自动触发策略；
// - roots 枚举语义由编译期选择的 GC backend 决定（见 `crates/scoop_runtime/src/gc_backend.rs`）；
// - GC-FIX Phase B2（stackmap-only）路线下，roots 应来自 stackmap/native_roots/handles/pin；
// - 对象内部引用字段的扫描依赖 `ScoopTypeDescriptor`（若 `type_desc` 为 NULL 则视为无引用字段）。
void scoop_gc_collect(void);

// 手动触发一次 minor GC（young generation / nursery evacuation）。
//
// 说明（TODO T1412c）：
// - 该 API 当前仅在 Immix backend 下具备完整语义：stop-the-world + nursery evacuation；
// - baseline/minimal/hosted backend 下该 API 退化为 `scoop_gc_collect()`（或 no-op），用于保持链接稳定；
// - v0 目标：暂停时间与 nursery 大小近似线性；old→nursery 入口依赖写屏障（T1412d）维持为空。
void scoop_gc_collect_minor(void);

// 尝试触发一次 minor GC（young generation / nursery evacuation）。
//
// 语义（TODO T1412e，try-minor / deadline）：
// - 若当前 backend 支持协作式 stop-the-world：
//   - 只有在 `deadline_ms` 截止时间内达成 STW（所有线程 park 就绪）才会进入 tracing/evacuation；
//   - 若未能在 deadline 内达成，则撤销本轮 STW 请求并唤醒已 park 线程，然后立刻返回；
// - baseline/minimal/hosted backend 下该 API 退化为 `scoop_gc_collect_minor()`（忽略 deadline）
//   以保持链接稳定，并返回 1；
// - Immix backend 下若 nursery 未启用，则返回 0（no-op）。
//
// 返回值：
// - 1：本轮 minor 已执行（可能为 no-op，但已进入 STW 并完成一轮 commit/reset）；
// - 0：本轮 minor 放弃（例如 STW 超时、nursery 未启用、或内部资源不足导致回滚）。
uint32_t scoop_gc_try_collect_minor(uint32_t deadline_ms);

// --- Write barrier（TODO T1412d） ---
//
// 为编译器生成的“引用写入（store）”提供统一写屏障 hook。
//
// 约定（v0 promote-on-store）：
// - `slot_addr` 指向“要写入的引用槽位”的内存地址（slot 本身的地址，而非 `void**` 的二级指针）；
// - `value` 为要写入的引用值（可为 NULL）；
// - 返回值：最终写入的值（v0 等于入参 `value`；保留未来升级为“搬迁/重定向后返回新地址”的扩展点）。
//
// 注意：
// - v0 选择用 `memcpy` 写入 slot，避免在某些承载类型为 `uintptr_t` 的场景（例如 Array word slots）
//   触发严格别名规则的 UB；因此 `slot_addr` 采用 “void* 地址” 而非 “void** 指针”。
void *scoop_gc_write_barrier(void *slot_addr, void *value);

// --- Pinning（spec §15.10） ---
//
// 说明：
// - `pin/unpin` 用于把某个 heap 对象标记为“不可移动且必须保活”，供 FFI/异步 I/O
//   等场景在把指针交给外部系统时使用。
// - v0（非移动 mark-sweep）阶段，对象本身不会移动；pin 的主要效果是把对象加入
//   “额外 roots”，避免在没有其它 roots（stackmap/native_roots/handle 等）引用时被 sweep。
// - 返回值：1 表示成功；0 表示失败（例如 obj==NULL、对象不在 heap 中、或 unpin 下溢）。
uint32_t scoop_pin(void *obj);
uint32_t scoop_unpin(void *obj);

// --- Stable handles（spec §15.10.1） ---
//
// 说明：
// - stable handle 用于把 heap 对象引用以“整数 token”形式交给 native/外部系统长期持有；
// - 与 pin 不同：handle 不保证对象地址不变（moving GC 下对象可能被搬迁）；
// - 复制 `uint64_t handle` / `GcHandle.raw` 只会复制 token 位模式，不会克隆底层 handle record；
//   每次成功 `scoop_handle_new` 仍只允许被 `scoop_handle_drop` 消费一次；
// - runtime 必须把 handle 表视为 roots，并在 moving/compaction 时更新 handle->obj 槽位。
//
// API 约定（v0）：
// - handle 值 0 表示失败/空 handle；
// - get/drop 对非法/陈旧 handle 返回 NULL/0（不崩溃）；语言级 surface 可据此选择 trap。
uint64_t scoop_handle_new(void *obj);
void *scoop_handle_get(uint64_t handle);
uint32_t scoop_handle_drop(uint64_t handle);

// 仅供 GC release_fn 使用：要求调用方已处于 GC 持锁上下文，不会再次尝试注册线程/加锁。
// 返回语义与 `scoop_handle_drop` 相同。
uint32_t scoop_handle_drop_in_release(uint64_t handle);

// --- Module-global roots（T4016b4a0） ---
//
// 说明：
// - 编译器会把 object property globals、object singleton globals、top-level immutable backing
//   globals 中“实际承载 GC 引用”的槽位注册到 runtime；
// - GC 必须把这些模块级全局槽视为永久 roots，并在 moving/compaction 后更新其内部引用；
// - `base` 指向该全局槽的起始地址，`type_desc` 描述其内存布局中的 GC pointer words。
//
// API 约定（v0）：
// - 重复注册同一 `base` 是幂等的：runtime 会复用现有记录并更新其 `type_desc`；
// - `base == NULL` 或 `type_desc == NULL` 时退化为 no-op；
// - 当前不提供 unregister：这些记录与编译单元同生命周期，进程退出时统一释放。
void scoop_gc_register_global_root(void *base, const ScoopTypeDescriptor *type_desc);

// 最小自检：用于 smoke test，确保结构体布局/基本假设可用。
// 返回 1 表示通过，0 表示失败。
uint32_t scoop_gc_self_check(void);

// 使用 type descriptor 扫描对象内部引用字段。
//
// 返回值：visitor 被调用的次数（即扫描到的引用槽位数量）。
uint64_t scoop_gc_type_descriptor_trace(const ScoopTypeDescriptor *type_desc,
                                       void *object,
                                       ScoopGcTraceVisitor visitor,
                                       void *ctx);

// --- Debug helpers（用于测试/fixtures；不承诺稳定 ABI） ---

// 返回当前 heap 链表中的对象个数（用于回归 sweep 是否回收）。
uint64_t scoop_gc_debug_heap_object_count(void);

// 返回 heap 统计字段（累计值）。
uint64_t scoop_gc_debug_heap_bytes_allocated(void);
uint64_t scoop_gc_debug_heap_bytes_freed(void);

// 返回“当前 heap 保留的内存”估算值（字节）。
//
// 说明：
// - 该值用于 microbench/观测碎片化趋势，不承诺与 OS 层 RSS/VM 完全一致；
// - 不同 GC backend 的语义不同，但应满足：`reserved_bytes >= live_bytes`；
// - baseline/minimal：按对象逐个 `malloc`，该值近似等于 live bytes（对象 size 之和）；
// - Immix：返回 `block_count * block_size + large_object_live_bytes`，因此可反映
//   “稀疏存活对象导致 block 无法回收”的碎片化开销。
uint64_t scoop_gc_debug_heap_bytes_reserved(void);

// Debug helper：分配 `count` 个“垃圾对象”（不写入 roots），用于 GC 压测与回归。
//
// 说明：当 `count <= 0` 时不分配；当 OOM 时会提前停止分配。
void scoop_gc_debug_alloc_garbage(int64_t count);

#endif // SCOOP_GC_H
