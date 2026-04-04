// Scoop GC backend: minimal (single-thread, non-STW).
//
// 目标（TODO T1405a）：
// - 提供一个“最小可用”的第二 GC backend，用于验证 backend 抽象/选择机制；
// - 复用现有对象头 + type descriptor + shadow stack roots 语义；
// - 不实现多线程 stop-the-world（STW）。
//
// 安全语义（early stage）：
// - 若检测到多个线程参与注册，则 `scoop_gc_collect()` 将退化为 no-op（宁可泄漏也不错误回收）。

#include "scoop_gc_backend.h"

#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_MINIMAL

#include "scoop_gc.h"

#include <pthread.h>
#include <stddef.h>
#include <stdlib.h>

static pthread_mutex_t scoop_gc_lock = PTHREAD_MUTEX_INITIALIZER;

// 进程全局 heap（v0：minimal backend 仍使用链表 + mark-sweep）。
ScoopGcHeap scoop_gc_heap;

// --- multi-thread detection（best-effort, leak-on-threads） ---
static uint32_t scoop_gc_owner_thread_set = 0;
static pthread_t scoop_gc_owner_thread;
static uint32_t scoop_gc_multi_thread_seen = 0;

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
  (void)pthread_mutex_lock(&scoop_gc_lock);

  if (!scoop_gc_owner_thread_set) {
    scoop_gc_owner_thread_set = 1;
    scoop_gc_owner_thread = self;
  } else if (!pthread_equal(self, scoop_gc_owner_thread)) {
    scoop_gc_multi_thread_seen = 1;
  }

  (void)pthread_mutex_unlock(&scoop_gc_lock);
}

void scoop_gc_thread_unregister(ScoopGcFrame **current_frame_slot) {
  (void)current_frame_slot;
  // minimal backend 不维护线程列表；保持幂等且不崩溃。
}

void scoop_gc_safepoint(void) {
  // minimal backend：无 STW，safepoint 为 no-op。
}

void scoop_gc_safepoint_poll(void) {
  // T1505a：minimal backend 无 STW；poll 与 safepoint 一致为 no-op。
  scoop_gc_safepoint();
}

// enter_native/leave_native（TODO T1505c）：
// - minimal backend 无 STW/线程状态机；这里提供最小的“可链接”实现（幂等且不崩溃）。
// - 语义：仅保证在未显式 init/register 的情况下调用也不会出错。
void scoop_enter_native(void ***root_slots, uint32_t root_slots_len) {
  (void)root_slots;
  (void)root_slots_len;

  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();
}

void scoop_leave_native(void) {
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();
}

// Test-only export（T1505b）：minimal backend 无 STW/park，因此该 smoke 统一返回 0。
intptr_t scoop_test_gc_stack_walking_ctx_smoke(void) { return 0; }

// Test-only export（T1411b）：minimal backend 无 STW/park，因此该 smoke 统一返回 0。
intptr_t scoop_test_gc_stack_walking_unwind_smoke(void) { return 0; }

// Test-only export（T1506b）：minimal backend 无 STW/park，因此该 smoke 统一返回 0。
intptr_t scoop_test_gc_stackmap_roots_enum_smoke(void) { return 0; }

// Test-only export（T1506c）：minimal backend 无 STW/park，因此该测试统一返回 0。
intptr_t scoop_test_gc_stackmap_multiframe_keepalive(void) { return 0; }

uint32_t scoop_pin(void *raw_obj) {
  if (raw_obj == 0) {
    return 0;
  }

  // 说明：保持与 baseline backend 对齐：允许在未显式 init/register 的情况下被调用。
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)raw_obj;

  (void)pthread_mutex_lock(&scoop_gc_lock);

  if (!scoop_gc_heap_contains_object_unlocked(obj)) {
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return 0;
  }

  ScoopGcPinnedRecord *rec = scoop_gc_find_pinned_unlocked(obj);
  if (rec != 0) {
    if (rec->pin_count == UINT64_MAX) {
      (void)pthread_mutex_unlock(&scoop_gc_lock);
      return 0;
    }
    rec->pin_count += 1;
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return 1;
  }

  rec = (ScoopGcPinnedRecord *)malloc(sizeof(ScoopGcPinnedRecord));
  if (rec == 0) {
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return 0;
  }

  rec->next = scoop_gc_pinned_objects;
  rec->object = obj;
  rec->pin_count = 1;
  scoop_gc_pinned_objects = rec;

  (void)pthread_mutex_unlock(&scoop_gc_lock);
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

  (void)pthread_mutex_lock(&scoop_gc_lock);

  ScoopGcPinnedRecord **link = &scoop_gc_pinned_objects;
  while (*link != 0) {
    ScoopGcPinnedRecord *it = *link;
    if (it->object != obj) {
      link = &it->next;
      continue;
    }

    if (it->pin_count == 0) {
      (void)pthread_mutex_unlock(&scoop_gc_lock);
      return 0;
    }

    it->pin_count -= 1;
    if (it->pin_count == 0) {
      *link = it->next;
      free(it);
    }

    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return 1;
  }

  (void)pthread_mutex_unlock(&scoop_gc_lock);
  return 0;
}

void scoop_gc_heap_register_object(ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return;
  }

  (void)pthread_mutex_lock(&scoop_gc_lock);

  obj->next = scoop_gc_heap.objects;
  scoop_gc_heap.objects = obj;
  scoop_gc_heap.bytes_allocated += obj->size_bytes;

  (void)pthread_mutex_unlock(&scoop_gc_lock);
}

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

  (void)pthread_mutex_lock(&scoop_gc_lock);

  // minimal backend 不支持多线程 roots 枚举；检测到多线程时退化为 no-op。
  if (scoop_gc_multi_thread_seen) {
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return;
  }

  ScoopGcHeap *heap = &scoop_gc_heap;
  uint32_t mark_value = scoop_gc_collect_next_mark_value(heap);

  ScoopGcMarkStack stack = {0};
  ScoopGcMarkCtx ctx = {heap, mark_value, &stack};

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

  // 3) sweep
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

    heap->bytes_freed += obj->size_bytes;
    free(obj);
  }

  (void)pthread_mutex_unlock(&scoop_gc_lock);
}

uint64_t scoop_gc_debug_heap_object_count(void) {
  (void)pthread_mutex_lock(&scoop_gc_lock);
  uint64_t count = 0;
  for (ScoopGcObjectHeader *it = scoop_gc_heap.objects; it != 0; it = it->next) {
    count++;
  }
  (void)pthread_mutex_unlock(&scoop_gc_lock);
  return count;
}

uint64_t scoop_gc_debug_heap_bytes_allocated(void) {
  (void)pthread_mutex_lock(&scoop_gc_lock);
  uint64_t v = scoop_gc_heap.bytes_allocated;
  (void)pthread_mutex_unlock(&scoop_gc_lock);
  return v;
}

uint64_t scoop_gc_debug_heap_bytes_freed(void) {
  (void)pthread_mutex_lock(&scoop_gc_lock);
  uint64_t v = scoop_gc_heap.bytes_freed;
  (void)pthread_mutex_unlock(&scoop_gc_lock);
  return v;
}

uint64_t scoop_gc_debug_heap_bytes_reserved(void) {
  (void)pthread_mutex_lock(&scoop_gc_lock);
  uint64_t total = 0;
  for (ScoopGcObjectHeader *it = scoop_gc_heap.objects; it != 0; it = it->next) {
    uint64_t size = it->size_bytes;
    if (UINT64_MAX - total < size) {
      total = UINT64_MAX;
      break;
    }
    total += size;
  }
  (void)pthread_mutex_unlock(&scoop_gc_lock);
  return total;
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

#endif // SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_MINIMAL
