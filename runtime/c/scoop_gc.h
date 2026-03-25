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

// 前置声明：type descriptor 在 TODO T0907 引入；此处只占位。
typedef struct ScoopTypeDescriptor ScoopTypeDescriptor;

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
// - 该结构体只用于“规划对象布局”和“连接 heap 列表”；并不代表最终 ABI。
// - TODO T0908 会把对象头与 `scoop_alloc` 的返回指针语义对齐（header vs payload）。
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

// 返回当前线程的 shadow stack 链头。
ScoopGcFrame *scoop_gc_current_frame(void);

// 将 frame push 到当前线程的 shadow stack 链头。
void scoop_gc_frame_push(ScoopGcFrame *frame);

// 将 frame 从 shadow stack 链头 pop（要求 top == frame）。
void scoop_gc_frame_pop(ScoopGcFrame *frame);

// Debug helper：遍历当前线程的 shadow stack，并统计非空 roots slot 的个数。
//
// 说明：
// - 该 API 主要用于 compiler/codegen 的插桩回归（TODO T0816）；
// - 当前阶段不执行真正的 mark/sweep，只做“可遍历且不崩溃”的扫描。
uint64_t scoop_gc_debug_count_roots_current_thread(void);

#endif // SCOOP_GC_H
