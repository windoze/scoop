// Scoop GC runtime (early stage).
//
// 说明：
// - 该文件提供 mark-sweep GC（v0）的“数据结构骨架”，用于后续逐步接入：
//   - type descriptor（对象内引用字段扫描）
//   - shadow stack root 扫描
//   - stop-the-world 与多线程支持
// - 当前阶段（TODO T0904）不要求可用的 GC 算法实现；先把结构与接口稳定下来。

#ifndef SCOOP_GC_H
#define SCOOP_GC_H

#include <stdint.h>
#include <stddef.h>

// --- Type descriptor（TODO T0907） ---
//
// 说明：
// - type descriptor 用于描述一个 heap 对象（或 box 的 payload）的布局信息：
//   - 对象大小（用于边界裁剪/健壮性）
//   - 引用字段（GC-managed pointers）的扫描规则（trace bitmap 或自定义回调）
// - 早期阶段只要求“可安全扫描”与“ABI 可被 Rust/C 测试构造并调用”；并不承诺最终 ABI。
// - TODO T0908 会把该 descriptor 与对象头/heap 布局对齐；届时 `trace_start_offset_bytes`
//   可用于跳过 header。

// 扫描回调：`slot` 指向对象内部某个“指针槽位”（可读写）。
typedef void (*ScoopGcTraceVisitor)(void **slot, void *ctx);

// 自定义 trace 函数：当 bitmap 无法表达（例如复杂容器/变长布局）时使用。
// 返回值为调用 visitor 的次数。
typedef uint64_t (*ScoopTypeTraceFn)(void *object, ScoopGcTraceVisitor visitor, void *ctx);

typedef struct ScoopTypeDescriptor {
  // ABI 版本（预留）：便于后续演进时做兼容分支；v0 固定为 0。
  uint32_t abi_version;

  // 预留 flags：例如“是否有尾随变长 payload”等（v0 未定义语义）。
  uint32_t flags;

  // 对象在内存中的总大小（字节）。
  uint64_t size_bytes;

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
} ScoopTypeDescriptor;

// Shadow stack（精确根集）帧。
//
// TODO T0905：
// - 该结构体将由编译器在函数 prologue/epilogue 插桩 push/pop（PLAN §8.3）。
// - 当前阶段仅要求能维护链表（prev 指针），不要求实现 root 扫描。
typedef struct ScoopGcFrame {
  // 上一个 frame（按调用栈嵌套形成链表）。
  struct ScoopGcFrame *prev;

  // `roots[]` 的元素个数。early stage：push/pop 不依赖该字段，但为后续扫描预留。
  uint32_t root_count;

  // 保留字段：用于对齐/版本/flags 等。
  uint32_t _reserved_u32;

  // roots slots：每个 slot 存放一个 GC-managed 指针（或 NULL）。
  void *roots[];
} ScoopGcFrame;

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
  uint64_t size;

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
_Static_assert(offsetof(ScoopGcObjectHeader, size) == (2u * sizeof(void *)),
               "ScoopGcObjectHeader.size offset must be 2*sizeof(void*)");
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

// 返回当前线程的 shadow stack 链头。
ScoopGcFrame *scoop_gc_current_frame(void);

// 将 frame push 到当前线程的 shadow stack 链头。
void scoop_gc_frame_push(ScoopGcFrame *frame);

// 将 frame 从 shadow stack 链头 pop（要求 top == frame）。
void scoop_gc_frame_pop(ScoopGcFrame *frame);

// 遍历当前线程的 shadow stack，并对每个非空 root slot 调用 visitor。
//
// 返回值：visitor 被调用的次数（即扫描到的 roots 数量）。
//
// 说明：
// - 该 API 为后续 mark 阶段提供“根集枚举”能力（TODO T0909）；
// - v0 只支持单线程：仅扫描当前线程的 frame 链；
// - visitor 会收到 `void** slot`（可读写），以便未来移动 GC 可原地更新引用。
uint64_t scoop_gc_shadow_stack_visit_roots_current_thread(ScoopGcTraceVisitor visitor,
                                                         void *ctx);

// Debug helper：遍历当前线程的 shadow stack，并统计非空 roots slot 的个数。
//
// 说明：
// - 该 API 主要用于 compiler/codegen 的插桩回归（TODO T0816）；
// - 当前阶段不执行真正的 mark/sweep，只做“可遍历且不崩溃”的扫描。
uint64_t scoop_gc_debug_count_roots_current_thread(void);

#endif // SCOOP_GC_H
