// Scoop GC runtime (early stage).
//
// TODO T0904：mark-sweep GC 的数据结构骨架（不要求可用）。

#include "scoop_gc.h"

void scoop_gc_heap_init(ScoopGcHeap *heap) {
  if (heap == 0) {
    return;
  }

  heap->objects = 0;
  heap->free_list = 0;
  heap->bytes_allocated = 0;
  heap->bytes_freed = 0;
  heap->gc_cycles = 0;
}

uint32_t scoop_gc_self_check(void) {
  // 说明：
  // - 这里的自检尽量保持“永远不会崩溃”：失败时返回 0；
  // - 更严格的布局/对齐断言会在 TODO T0908/T0907 的单测中补齐。
  //
  // 当前阶段只验证“基础假设”：
  // - uint64_t 为 8 字节
  // - 结构体 size 不为 0
  if (sizeof(uint64_t) != 8) {
    return 0;
  }
  if (sizeof(ScoopGcHeap) == 0) {
    return 0;
  }
  if (sizeof(ScoopGcObjectHeader) == 0) {
    return 0;
  }
  if (sizeof(ScoopGcFreeBlock) == 0) {
    return 0;
  }

  return 1;
}
