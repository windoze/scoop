// Scoop GC runtime (early stage).
//
// TODO T0904：mark-sweep GC 的数据结构骨架。
// TODO T0910：实现最小可用的单线程 mark-sweep（手动触发）。

#include "scoop_gc.h"

#include <stddef.h>
#include <stdlib.h>

// 进程全局 heap（v0：单线程）。
//
// 说明：
// - 该符号不在头文件中导出；对外通过 `scoop_alloc`/`scoop_gc_collect` 等 API 访问；
// - 多线程 stop-the-world 与 per-thread allocator 将在后续任务（T0911+）补齐。
ScoopGcHeap scoop_gc_heap;

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

typedef struct ScoopGcMarkStack {
  ScoopGcObjectHeader **items;
  size_t len;
  size_t cap;
} ScoopGcMarkStack;

static uint32_t scoop_gc_collect_next_mark_value(ScoopGcHeap *heap) {
  // v0：用 `gc_cycles` 生成一个 u32 mark stamp，避免每次 sweep 都遍历 survivors 清零。
  // 只要 stamp 不回卷（wrap），`header.mark == stamp` 即表示“本轮已标记”。
  if (heap == 0) {
    return 1;
  }

  heap->gc_cycles += 1;
  uint32_t mark_value = (uint32_t)heap->gc_cycles;
  if (mark_value != 0) {
    return mark_value;
  }

  // 处理 u32 wrap：回到 0 时，先把所有对象 mark 清零，再重新开始计数。
  for (ScoopGcObjectHeader *it = heap->objects; it != 0; it = it->next) {
    it->mark = 0;
  }

  heap->gc_cycles += 1;
  mark_value = (uint32_t)heap->gc_cycles;
  if (mark_value == 0) {
    // 极端情况：u64->u32 连续两次为 0（理论上不可能）；保守回退为 1。
    mark_value = 1;
  }
  return mark_value;
}

static void scoop_gc_mark_stack_push(ScoopGcMarkStack *stack, ScoopGcObjectHeader *obj) {
  if (stack == 0 || obj == 0) {
    return;
  }

  if (stack->len == stack->cap) {
    size_t new_cap = (stack->cap == 0) ? 1024u : stack->cap * 2u;
    if (new_cap < stack->cap) {
      // overflow：放弃扩容（v0：宁可漏标也不崩溃；但实际不应发生）。
      return;
    }
    if (new_cap > (SIZE_MAX / sizeof(ScoopGcObjectHeader *))) {
      return;
    }

    void *p = realloc(stack->items, new_cap * sizeof(ScoopGcObjectHeader *));
    if (p == 0) {
      return;
    }
    stack->items = (ScoopGcObjectHeader **)p;
    stack->cap = new_cap;
  }

  stack->items[stack->len++] = obj;
}

static ScoopGcObjectHeader *scoop_gc_mark_stack_pop(ScoopGcMarkStack *stack) {
  if (stack == 0 || stack->len == 0) {
    return 0;
  }

  stack->len -= 1;
  return stack->items[stack->len];
}

typedef struct ScoopGcMarkCtx {
  ScoopGcHeap *heap;
  uint32_t mark_value;
  ScoopGcMarkStack *stack;
} ScoopGcMarkCtx;

static void scoop_gc_mark_object_if_needed(ScoopGcMarkCtx *ctx, ScoopGcObjectHeader *obj) {
  if (ctx == 0 || obj == 0) {
    return;
  }

  if (obj->mark == ctx->mark_value) {
    return;
  }

  obj->mark = ctx->mark_value;
  scoop_gc_mark_stack_push(ctx->stack, obj);
}

static void scoop_gc_mark_visitor(void **slot, void *raw_ctx) {
  if (slot == 0 || raw_ctx == 0) {
    return;
  }

  ScoopGcMarkCtx *ctx = (ScoopGcMarkCtx *)raw_ctx;
  void *raw = *slot;
  if (raw == 0) {
    return;
  }

  scoop_gc_mark_object_if_needed(ctx, (ScoopGcObjectHeader *)raw);
}

void scoop_gc_collect(void) {
  // v0：单线程，无 stop-the-world；只扫描当前线程 roots。
  ScoopGcHeap *heap = &scoop_gc_heap;
  uint32_t mark_value = scoop_gc_collect_next_mark_value(heap);

  ScoopGcMarkStack stack = {0};
  ScoopGcMarkCtx ctx = {heap, mark_value, &stack};

  // 1) mark roots
  (void)scoop_gc_shadow_stack_visit_roots_current_thread(scoop_gc_mark_visitor,
                                                        (void *)&ctx);

  // 2) mark transitive closure（若对象带 type descriptor）。
  while (stack.len > 0) {
    ScoopGcObjectHeader *obj = scoop_gc_mark_stack_pop(&stack);
    if (obj == 0) {
      continue;
    }

    if (obj->type_desc == 0) {
      continue;
    }

    (void)scoop_gc_type_descriptor_trace(obj->type_desc,
                                         (void *)obj,
                                         scoop_gc_mark_visitor,
                                         (void *)&ctx);
  }

  if (stack.items != 0) {
    free(stack.items);
  }

  // 3) sweep
  ScoopGcObjectHeader **link = &heap->objects;
  while (*link != 0) {
    ScoopGcObjectHeader *obj = *link;
    if (obj->mark == mark_value) {
      link = &obj->next;
      continue;
    }

    // unreachable：从链表摘除并释放
    *link = obj->next;
    heap->bytes_freed += obj->size;
    free(obj);
  }
}

uint64_t scoop_gc_debug_heap_object_count(void) {
  uint64_t count = 0;
  for (ScoopGcObjectHeader *it = scoop_gc_heap.objects; it != 0; it = it->next) {
    count++;
  }
  return count;
}

uint64_t scoop_gc_debug_heap_bytes_allocated(void) {
  return scoop_gc_heap.bytes_allocated;
}

uint64_t scoop_gc_debug_heap_bytes_freed(void) {
  return scoop_gc_heap.bytes_freed;
}

// `scoop_alloc` 由 `scoop_runtime.c` 实现；这里仅声明供 debug helper 调用。
void *scoop_alloc(uint64_t size);

void scoop_gc_debug_alloc_garbage(int64_t count) {
  if (count <= 0) {
    return;
  }

  uint64_t obj_size = (uint64_t)sizeof(ScoopGcObjectHeader);
  for (int64_t i = 0; i < count; i++) {
    void *p = scoop_alloc(obj_size);
    if (p == 0) {
      // OOM：提前停止分配，避免无意义的长循环。
      break;
    }
  }
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
