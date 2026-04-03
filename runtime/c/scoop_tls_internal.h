// Scoop runtime TLS layout (internal).
//
// 说明：
// - 该文件是 runtime/c 的 **internal** 头文件：用于在多个 C 编译单元之间共享
//   “每线程 TLS 结构”的布局约定（例如：GC 线程记录需要从 `current_frame_slot`
//   反推出其它 TLS 槽位）。
// - 不属于对外 ABI：不会出现在 `runtime/c/scoop_runtime_api.h` allowlist 中。
// - 若你修改了该文件，请同步更新 `crates/scoop_runtime/build.rs` 的 `rerun-if-changed`。
//
// 当前用途（TODO T1409a）：
// - Immix thread-local allocator：在每线程 TLS 中保存“当前分配 block”指针；
// - GC 在 stop-the-world 后需要清空该槽位，避免 compaction/free block 后出现悬挂指针。

#ifndef SCOOP_TLS_INTERNAL_H
#define SCOOP_TLS_INTERNAL_H

#include <stddef.h>
#include <stdint.h>

typedef struct ScoopGcFrame ScoopGcFrame;

// 每线程 TLS 状态（early stage：占位 + 渐进扩展）。
//
// 注意：
// - 该结构体不是稳定 ABI；它是 runtime/c 的内部实现细节；
// - 但其字段偏移会被其它 C 编译单元通过 `offsetof` 使用，因此需要集中声明。
typedef struct ScoopThreadTls {
  // 1 表示已注册到 runtime；0 表示未注册。
  uint32_t registered;

  // 保留字段：未来用于版本/flags 等。
  uint32_t _reserved_u32;

  // GC：shadow stack 当前帧链头（TODO T0905）。
  ScoopGcFrame *gc_current_frame;

  // Immix：thread-local 当前分配 block（TODO T1409a）。
  // - 该字段只在 Immix backend 下使用；
  // - 用 `void*` 避免把 Immix 内部类型泄漏到 TLS 头文件（保持 include 依赖最小）。
  void *gc_immix_current_block;

  // Immix：thread-local block cache（TODO T1409b）。
  //
  // 目的：
  // - 在并发分配路径中，当 current block 放不下需要 refill 时，尽量避免每次都抢全局 GC 锁；
  // - 通过“批量从全局 block pool 取 blocks 并缓存在 TLS”降低锁进入频率。
  //
  // 约定：
  // - cache 以 `ScoopGcImmixBlock.next_free` 串成单链表；
  // - cache 中的 blocks 只属于当前线程使用（不会被其它线程并发写）；
  // - stop-the-world / GC 周期开始后必须清空 cache 槽位，避免 compaction/free block 后悬挂。
  void *gc_immix_block_cache;
  uint32_t gc_immix_block_cache_len;
  uint32_t _reserved_u32_1;

  // GC：native roots buffer（TODO T1505c）。
  //
  // 语义：
  // - 当线程进入 native/extern 过渡态时，`enter_native` 会在 TLS 中维护该 buffer；
  // - STW/GC 将把处于 InNative 的线程视为“已就绪”，roots 从该 buffer 枚举；
  // - v0：允许调用方直接传入 roots slots 指针数组（不强制复制）。
  //
  // 表示：
  // - `gc_native_roots` 指向一个“slots 指针数组”，数组元素的类型为 `void**`（可读写引用槽位）；
  // - 由于该头文件不引入 triple-pointer 类型，这里用 `void*` 表示，使用方需 cast 为 `void***`。
  void *gc_native_roots;
  uint32_t gc_native_roots_len;
  uint32_t _reserved_u32_2;

  // effect runtime（TODO T0906/...）：预留字段（未来用于 handler stack 等）。
  void *_reserved0;
  void *_reserved1;
  void *_reserved2;
} ScoopThreadTls;

// 从 `&tls.gc_current_frame` 反推 `tls` 基址。
static inline ScoopThreadTls *scoop_tls_from_gc_current_frame_slot(
    ScoopGcFrame **current_frame_slot) {
  if (current_frame_slot == 0) {
    return 0;
  }

  uintptr_t p = (uintptr_t)current_frame_slot;
  uintptr_t base = p - (uintptr_t)offsetof(ScoopThreadTls, gc_current_frame);
  return (ScoopThreadTls *)base;
}

static inline void **scoop_tls_gc_immix_current_block_slot_from_current_frame_slot(
    ScoopGcFrame **current_frame_slot) {
  ScoopThreadTls *tls = scoop_tls_from_gc_current_frame_slot(current_frame_slot);
  if (tls == 0) {
    return 0;
  }
  return &tls->gc_immix_current_block;
}

static inline void **scoop_tls_gc_immix_block_cache_slot_from_current_frame_slot(
    ScoopGcFrame **current_frame_slot) {
  ScoopThreadTls *tls = scoop_tls_from_gc_current_frame_slot(current_frame_slot);
  if (tls == 0) {
    return 0;
  }
  return &tls->gc_immix_block_cache;
}

static inline uint32_t *scoop_tls_gc_immix_block_cache_len_slot_from_current_frame_slot(
    ScoopGcFrame **current_frame_slot) {
  ScoopThreadTls *tls = scoop_tls_from_gc_current_frame_slot(current_frame_slot);
  if (tls == 0) {
    return 0;
  }
  return &tls->gc_immix_block_cache_len;
}

#endif // SCOOP_TLS_INTERNAL_H
