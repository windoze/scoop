// Scoop GC backend: immix (v0; single-thread, non-moving).
//
// 当前实现（TODO T1406a/T1406b/T1406c / PLAN §15.3）：
// - allocator：line/block + hole bump（优先复用 partial blocks，降低碎片化）；
// - mark-region：按对象 trace 标记其覆盖到的 lines；
// - region sweep：基于 line mark/alloc bitmap 回收 holes，并重建可复用 block 列表。
//
// 限制（v0）：
// - 不支持多线程 roots 枚举；若检测到多个线程参与注册，则 `scoop_gc_collect()` 退化为 no-op；
// - 不支持移动/压缩（moving/compaction），见 TODO T1407。

#include "scoop_gc_backend.h"

#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX

#include "scoop_gc.h"
#include "scoop_gc_immix_internal.h"

#include <pthread.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

// 进程全局 heap（对外 ABI：`scoop_gc_heap`）。
//
// 注意：
// - baseline/minimal backend 使用 `heap.free_list` 存放 free list（未来复用）；
// - Immix v0 在不改动 ABI 的前提下，把 `heap.free_list` “挪作内部 state 指针”。
ScoopGcHeap scoop_gc_heap;

static ScoopGcImmixState *scoop_gc_immix_state(void) {
  return scoop_gc_immix_state_from_heap(&scoop_gc_heap);
}

static void scoop_gc_immix_lock(ScoopGcImmixState *state) {
  if (state == 0 || !state->lock_inited) {
    return;
  }
  (void)pthread_mutex_lock(&state->lock);
}

static void scoop_gc_immix_unlock(ScoopGcImmixState *state) {
  if (state == 0 || !state->lock_inited) {
    return;
  }
  (void)pthread_mutex_unlock(&state->lock);
}

// --- Pinning（spec §15.10 / TODO T0912） ---
typedef struct ScoopGcPinnedRecord {
  struct ScoopGcPinnedRecord *next;
  ScoopGcObjectHeader *object;
  uint64_t pin_count;
} ScoopGcPinnedRecord;

static ScoopGcPinnedRecord *scoop_gc_pinned_objects = 0;

static uint32_t scoop_gc_heap_contains_object_unlocked(ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return 0;
  }

  for (ScoopGcObjectHeader *it = scoop_gc_heap.objects; it != 0; it = it->next) {
    if (it == obj) {
      return 1;
    }
  }
  return 0;
}

static ScoopGcPinnedRecord *scoop_gc_find_pinned_unlocked(ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return 0;
  }

  for (ScoopGcPinnedRecord *it = scoop_gc_pinned_objects; it != 0; it = it->next) {
    if (it->object == obj) {
      return it;
    }
  }
  return 0;
}

void scoop_gc_thread_register(ScoopGcFrame **current_frame_slot) {
  (void)current_frame_slot;

  pthread_t self = pthread_self();
  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return;
  }

  scoop_gc_immix_lock(state);

  if (!state->owner_thread_set) {
    state->owner_thread_set = 1;
    state->owner_thread = self;
  } else if (!pthread_equal(self, state->owner_thread)) {
    state->multi_thread_seen = 1;
  }

  scoop_gc_immix_unlock(state);
}

void scoop_gc_thread_unregister(ScoopGcFrame **current_frame_slot) {
  (void)current_frame_slot;
  // Immix v0（单线程）：不维护线程列表；保持幂等且不崩溃。
}

void scoop_gc_safepoint(void) {
  // Immix v0（单线程）：无 STW，safepoint 为 no-op。
}

uint32_t scoop_pin(void *raw_obj) {
  if (raw_obj == 0) {
    return 0;
  }

  // 说明：保持与 baseline/minimal backend 对齐：允许在未显式 init/register 的情况下被调用。
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)raw_obj;

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return 0;
  }
  scoop_gc_immix_lock(state);

  if (!scoop_gc_heap_contains_object_unlocked(obj)) {
    scoop_gc_immix_unlock(state);
    return 0;
  }

  ScoopGcPinnedRecord *rec = scoop_gc_find_pinned_unlocked(obj);
  if (rec != 0) {
    if (rec->pin_count == UINT64_MAX) {
      scoop_gc_immix_unlock(state);
      return 0;
    }
    rec->pin_count += 1;
    scoop_gc_immix_unlock(state);
    return 1;
  }

  rec = (ScoopGcPinnedRecord *)malloc(sizeof(ScoopGcPinnedRecord));
  if (rec == 0) {
    scoop_gc_immix_unlock(state);
    return 0;
  }

  rec->next = scoop_gc_pinned_objects;
  rec->object = obj;
  rec->pin_count = 1;
  scoop_gc_pinned_objects = rec;

  scoop_gc_immix_unlock(state);
  return 1;
}

uint32_t scoop_unpin(void *raw_obj) {
  if (raw_obj == 0) {
    return 0;
  }

  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)raw_obj;

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return 0;
  }
  scoop_gc_immix_lock(state);

  ScoopGcPinnedRecord **link = &scoop_gc_pinned_objects;
  while (*link != 0) {
    ScoopGcPinnedRecord *it = *link;
    if (it->object != obj) {
      link = &it->next;
      continue;
    }

    if (it->pin_count == 0) {
      scoop_gc_immix_unlock(state);
      return 0;
    }

    it->pin_count -= 1;
    if (it->pin_count == 0) {
      *link = it->next;
      free(it);
    }

    scoop_gc_immix_unlock(state);
    return 1;
  }

  scoop_gc_immix_unlock(state);
  return 0;
}

void scoop_gc_heap_register_object(ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return;
  }

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return;
  }
  scoop_gc_immix_lock(state);

  obj->next = scoop_gc_heap.objects;
  scoop_gc_heap.objects = obj;
  scoop_gc_heap.bytes_allocated += obj->size;

  scoop_gc_immix_unlock(state);
}

void scoop_gc_heap_init(ScoopGcHeap *heap) {
  if (heap == 0) {
    return;
  }

  ScoopGcImmixState *state = scoop_gc_immix_state_from_heap(heap);
  if (state == 0) {
    state = (ScoopGcImmixState *)malloc(sizeof(ScoopGcImmixState));
    if (state != 0) {
      (void)memset(state, 0, sizeof(*state));
      if (pthread_mutex_init(&state->lock, 0) == 0) {
        state->lock_inited = 1;
      }
    }
    scoop_gc_immix_heap_set_state(heap, state);
  }

  if (state != 0 && state->lock_inited) {
    scoop_gc_immix_lock(state);
    state->multi_thread_seen = 0;
    state->owner_thread_set = 0;

    // 把已分配的 blocks 复位并串到 free list，供分配路径复用。
    state->reusable_blocks = 0;
    state->free_blocks = 0;
    state->current_block = 0;
    for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
      scoop_gc_immix_block_reset(it);
      it->next_free = state->free_blocks;
      state->free_blocks = it;
    }

    scoop_gc_immix_unlock(state);
  }

  heap->objects = 0;
  heap->bytes_allocated = 0;
  heap->bytes_freed = 0;
  heap->gc_cycles = 0;
}

typedef struct ScoopGcMarkStack {
  ScoopGcObjectHeader **items;
  size_t len;
  size_t cap;
} ScoopGcMarkStack;

static uint32_t scoop_gc_collect_next_mark_value(ScoopGcHeap *heap) {
  if (heap == 0) {
    return 1;
  }

  heap->gc_cycles += 1;
  uint32_t mark_value = (uint32_t)heap->gc_cycles;
  if (mark_value != 0) {
    return mark_value;
  }

  for (ScoopGcObjectHeader *it = heap->objects; it != 0; it = it->next) {
    it->mark = 0;
  }

  heap->gc_cycles += 1;
  mark_value = (uint32_t)heap->gc_cycles;
  if (mark_value == 0) {
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
  // mark-region：额外把对象覆盖到的 lines 记录到 block 的 mark bitmap（用于 region sweep 回收 holes）。
  ScoopGcImmixBlock *block = scoop_gc_immix_block_from_object((void *)obj);
  if (block != 0) {
    uint64_t raw_size = obj->size;
    size_t size = (raw_size > (uint64_t)SIZE_MAX) ? (size_t)SIZE_MAX : (size_t)raw_size;
    scoop_gc_immix_block_mark_marked_range(block, (const uint8_t *)obj, size);
  }
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
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return;
  }
  scoop_gc_immix_lock(state);

  // Immix v0（单线程）不支持多线程 roots 枚举；检测到多线程时退化为 no-op。
  if (state->multi_thread_seen) {
    scoop_gc_immix_unlock(state);
    return;
  }

  ScoopGcHeap *heap = &scoop_gc_heap;
  uint32_t mark_value = scoop_gc_collect_next_mark_value(heap);

  ScoopGcMarkStack stack = {0};
  ScoopGcMarkCtx ctx = {heap, mark_value, &stack};

  // 0) clear per-block mark bitmap（避免上一轮残留影响 region sweep）
  for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
    scoop_gc_immix_block_clear_mark_bits(it);
  }

  // 1) mark roots（只扫描当前线程的 shadow stack）
  (void)scoop_gc_shadow_stack_visit_roots_current_thread(scoop_gc_mark_visitor, (void *)&ctx);

  // 1b) mark pinned roots（spec §15.10）
  for (ScoopGcPinnedRecord *it = scoop_gc_pinned_objects; it != 0; it = it->next) {
    if (it->object == 0) {
      continue;
    }
    if (it->pin_count == 0) {
      continue;
    }
    scoop_gc_mark_object_if_needed(&ctx, it->object);
  }

  // 2) mark transitive closure
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

  // 3) sweep：释放 unreachable 对象；Immix block 内对象不逐个 free，而是留给 region sweep 复用 holes。
  ScoopGcObjectHeader **link = &heap->objects;
  while (*link != 0) {
    ScoopGcObjectHeader *obj = *link;
    if (obj->mark == mark_value) {
      link = &obj->next;
      continue;
    }

    *link = obj->next;

    if (obj->type_desc != 0 && obj->type_desc->release_fn != 0) {
      obj->type_desc->release_fn((void *)obj);
    }

    heap->bytes_freed += obj->size;

    ScoopGcImmixBlock *block = scoop_gc_immix_block_from_object((void *)obj);
    if (block == 0) {
      // large object / fallback malloc：可以直接 free。
      free(obj);
      continue;
    }

    if (block->live_objects > 0) {
      block->live_objects -= 1;
    }
  }

  // 4) region sweep：把 mark bitmap（live lines）融合回 alloc bitmap，并重建可复用 block 列表。
  //
  // 策略（v0）：优先复用 partial blocks（减少碎片化），其次复用整块空闲 blocks。
  state->reusable_blocks = 0;
  state->free_blocks = 0;
  state->current_block = 0;

  for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
    it->next_free = 0;

    if (it->live_objects == 0) {
      scoop_gc_immix_block_reset(it);
      it->next_free = state->free_blocks;
      state->free_blocks = it;
      continue;
    }

    // 把 live lines 保留为 alloc bits；dead lines 清零为 hole；并清空 mark bits。
    size_t reserved = scoop_gc_immix_block_reserved_lines(it);
    for (size_t line = reserved; line < (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK; line++) {
      uint32_t live = scoop_gc_immix_bitmap_test_bit(it->line_mark_bits,
                                                     SCOOP_GC_IMMIX_BITMAP_WORDS,
                                                     line);
      if (live) {
        scoop_gc_immix_bitmap_set_bit(it->line_alloc_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
      } else {
        scoop_gc_immix_bitmap_clear_bit(it->line_alloc_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
      }
      scoop_gc_immix_bitmap_clear_bit(it->line_mark_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
    }

    // 准备第一个 hole：之后分配可以在 hole 内 bump。
    scoop_gc_immix_block_setup_first_hole(it);
    if (it->cursor < it->limit) {
      it->next_free = state->reusable_blocks;
      state->reusable_blocks = it;
    }
  }

  scoop_gc_immix_unlock(state);
}

uint64_t scoop_gc_debug_heap_object_count(void) {
  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return 0;
  }
  scoop_gc_immix_lock(state);
  uint64_t count = 0;
  for (ScoopGcObjectHeader *it = scoop_gc_heap.objects; it != 0; it = it->next) {
    count++;
  }
  scoop_gc_immix_unlock(state);
  return count;
}

uint64_t scoop_gc_debug_heap_bytes_allocated(void) {
  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return 0;
  }
  scoop_gc_immix_lock(state);
  uint64_t v = scoop_gc_heap.bytes_allocated;
  scoop_gc_immix_unlock(state);
  return v;
}

uint64_t scoop_gc_debug_heap_bytes_freed(void) {
  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return 0;
  }
  scoop_gc_immix_lock(state);
  uint64_t v = scoop_gc_heap.bytes_freed;
  scoop_gc_immix_unlock(state);
  return v;
}

void *scoop_alloc(uint64_t size);

void scoop_gc_debug_alloc_garbage(int64_t count) {
  if (count <= 0) {
    return;
  }

  uint64_t obj_size = (uint64_t)sizeof(ScoopGcObjectHeader);
  for (int64_t i = 0; i < count; i++) {
    void *p = scoop_alloc(obj_size);
    if (p == 0) {
      break;
    }
  }
}

#endif // SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX
