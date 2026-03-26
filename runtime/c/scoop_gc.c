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

uint64_t scoop_gc_type_descriptor_trace(const ScoopTypeDescriptor *type_desc,
                                       void *object,
                                       ScoopGcTraceVisitor visitor,
                                       void *ctx) {
  if (type_desc == 0 || object == 0 || visitor == 0) {
    return 0;
  }

  // 若提供了自定义 trace 回调，则优先使用它（用于复杂/变长布局）。
  if (type_desc->trace_fn != 0) {
    return type_desc->trace_fn(object, visitor, ctx);
  }

  // bitmap 为 NULL 则表示“无引用字段”。
  if (type_desc->trace_bitmap == 0 || type_desc->trace_bitmap_u64_len == 0) {
    return 0;
  }

  // 健壮性：避免 size_t 溢出；真实对象不可能超过 address space。
  if (type_desc->size_bytes > (uint64_t)SIZE_MAX) {
    return 0;
  }
  if (type_desc->trace_start_offset_bytes > (uint64_t)SIZE_MAX) {
    return 0;
  }

  size_t size_bytes = (size_t)type_desc->size_bytes;
  size_t trace_start = (size_t)type_desc->trace_start_offset_bytes;
  size_t word_size = sizeof(void *);

  if (word_size == 0) {
    return 0;
  }
  if (trace_start >= size_bytes) {
    return 0;
  }
  // v0 仅支持 word 对齐扫描，避免在部分平台上出现未对齐指针访问问题。
  if ((trace_start % word_size) != 0) {
    return 0;
  }

  uint8_t *base = (uint8_t *)object + trace_start;
  size_t scan_bytes = size_bytes - trace_start;
  size_t scan_words = scan_bytes / word_size;

  uint64_t visited = 0;
  size_t bitmap_words = (size_t)type_desc->trace_bitmap_u64_len;

  for (size_t i = 0; i < scan_words; i++) {
    size_t word_index = i;
    size_t bitmap_word_index = word_index / 64u;
    if (bitmap_word_index >= bitmap_words) {
      // bitmap 未覆盖的部分视为“无引用字段”（更安全；避免 OOB）。
      continue;
    }

    uint64_t bits = type_desc->trace_bitmap[bitmap_word_index];
    uint64_t mask = (uint64_t)1u << (uint64_t)(word_index % 64u);
    if ((bits & mask) == 0) {
      continue;
    }

    void **slot = (void **)(base + (i * word_size));
    visitor(slot, ctx);
    visited++;
  }

  return visited;
}
