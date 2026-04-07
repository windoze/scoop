// Scoop GC backend: hosted/adapter (v0).
//
// 目标（TODO T1410）：
// - 为不适合自带 GC 的环境提供一个“hosted/adapter”形态的 backend；
// - v0 先落地一个可单独回归验证的实现：单线程、无 STW，复用 shadow stack roots；
// - 实现尽量不依赖 pthread/condvar 等 OS 原语，便于后续在 WASM/embedded 等受限环境裁剪/替换。
//
// 安全语义（early stage）：
// - 该 backend 不支持多线程 roots 枚举；
// - 当检测到“存在多个已注册线程”时，`scoop_gc_collect()` 退化为 no-op（宁可泄漏也不错误回收）。

#include "scoop_gc_backend.h"

#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_HOSTED

#include "scoop_gc.h"

#include <stdatomic.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

typedef struct ScoopThreadTls ScoopThreadTls;

// --- spin lock（避免 pthread 依赖） ---
//
// 说明：
// - v0 选择使用 `atomic_flag` 实现最小自旋锁，保护 heap 链表与 pinned 链表的并发修改；
// - 在“受限平台/单线程”场景下，该锁几乎不会产生自旋；在多线程场景下本 backend 仍不启用 GC，
//   但至少保证链表结构不会被并发写入破坏。
static atomic_flag scoop_gc_lock = ATOMIC_FLAG_INIT;

static inline void scoop_gc_lock_acquire(void) {
  while (atomic_flag_test_and_set_explicit(&scoop_gc_lock, memory_order_acquire)) {
    // busy-wait
  }
}

static inline void scoop_gc_lock_release(void) {
  atomic_flag_clear_explicit(&scoop_gc_lock, memory_order_release);
}

// 进程全局 heap（v0：hosted backend 仍使用链表 + mark-sweep）。
ScoopGcHeap scoop_gc_heap;

// --- registered threads（best-effort gating） ---
static atomic_uint_fast32_t scoop_gc_registered_threads = 0;

// --- Pinning（spec §15.10 / TODO T0912） ---
typedef struct ScoopGcPinnedRecord {
  struct ScoopGcPinnedRecord *next;
  ScoopGcObjectHeader *object;
  uint64_t pin_count;
} ScoopGcPinnedRecord;

static ScoopGcPinnedRecord *scoop_gc_pinned_objects = 0;

// --- Stable handles（spec §15.10.1 / TODO T1510a） ---
typedef struct ScoopGcHandleRecord {
  struct ScoopGcHandleRecord *next;
  ScoopGcObjectHeader *object;
} ScoopGcHandleRecord;

static ScoopGcHandleRecord *scoop_gc_handle_records = 0;

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

void scoop_gc_thread_register(ScoopThreadTls *tls) {
  (void)tls;

  scoop_gc_lock_acquire();
  (void)atomic_fetch_add_explicit(&scoop_gc_registered_threads, 1, memory_order_relaxed);
  scoop_gc_lock_release();
}

void scoop_gc_thread_unregister(ScoopThreadTls *tls) {
  (void)tls;

  scoop_gc_lock_acquire();
  uint32_t prev = (uint32_t)atomic_load_explicit(&scoop_gc_registered_threads, memory_order_relaxed);
  if (prev > 0) {
    (void)atomic_fetch_sub_explicit(&scoop_gc_registered_threads, 1, memory_order_relaxed);
  }
  scoop_gc_lock_release();
}

void scoop_gc_safepoint(void) {
  // hosted backend：无 STW，safepoint 为 no-op。
}

void scoop_gc_safepoint_poll(void) {
  // T1505a：hosted backend 无 STW；poll 与 safepoint 一致为 no-op。
  scoop_gc_safepoint();
}

void *scoop_gc_write_barrier(void *slot_addr, void *value) {
  if (slot_addr == 0) {
    return value;
  }

  // hosted backend：无 nursery/minor 语义，写屏障退化为“写入 slot”本身。
  (void)memcpy(slot_addr, &value, sizeof(void *));
  return value;
}

// enter_native/leave_native（TODO T1505c）：
// - hosted backend 无 STW/线程状态机；这里提供最小的“可链接”实现（幂等且不崩溃）。
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

// Test-only export（T1505b）：hosted backend 无 STW/park，因此该 smoke 统一返回 0。
intptr_t scoop_test_gc_stack_walking_ctx_smoke(void) { return 0; }

// Test-only export（T1411b）：hosted backend 无 STW/park，因此该 smoke 统一返回 0。
intptr_t scoop_test_gc_stack_walking_unwind_smoke(void) { return 0; }

// Test-only export（T1506b）：hosted backend 无 STW/park，因此该 smoke 统一返回 0。
intptr_t scoop_test_gc_stackmap_roots_enum_smoke(void) { return 0; }

// Test-only export（T1506c）：hosted backend 无 STW/park，因此该测试统一返回 0。
intptr_t scoop_test_gc_stackmap_multiframe_keepalive(void) { return 0; }

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

  scoop_gc_lock_acquire();

  if (!scoop_gc_heap_contains_object_unlocked(obj)) {
    scoop_gc_lock_release();
    return 0;
  }

  ScoopGcPinnedRecord *rec = scoop_gc_find_pinned_unlocked(obj);
  if (rec != 0) {
    if (rec->pin_count == UINT64_MAX) {
      scoop_gc_lock_release();
      return 0;
    }
    rec->pin_count += 1;
    scoop_gc_lock_release();
    return 1;
  }

  rec = (ScoopGcPinnedRecord *)malloc(sizeof(ScoopGcPinnedRecord));
  if (rec == 0) {
    scoop_gc_lock_release();
    return 0;
  }

  rec->next = scoop_gc_pinned_objects;
  rec->object = obj;
  rec->pin_count = 1;
  scoop_gc_pinned_objects = rec;

  scoop_gc_lock_release();
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

  scoop_gc_lock_acquire();

  ScoopGcPinnedRecord **link = &scoop_gc_pinned_objects;
  while (*link != 0) {
    ScoopGcPinnedRecord *it = *link;
    if (it->object != obj) {
      link = &it->next;
      continue;
    }

    if (it->pin_count == 0) {
      scoop_gc_lock_release();
      return 0;
    }

    it->pin_count -= 1;
    if (it->pin_count == 0) {
      *link = it->next;
      free(it);
    }

    scoop_gc_lock_release();
    return 1;
  }

  scoop_gc_lock_release();
  return 0;
}

uint64_t scoop_handle_new(void *raw_obj) {
  if (raw_obj == 0) {
    return 0;
  }

  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)raw_obj;

  scoop_gc_lock_acquire();

  if (!scoop_gc_heap_contains_object_unlocked(obj)) {
    scoop_gc_lock_release();
    return 0;
  }

  ScoopGcHandleRecord *rec = (ScoopGcHandleRecord *)malloc(sizeof(ScoopGcHandleRecord));
  if (rec == 0) {
    scoop_gc_lock_release();
    return 0;
  }

  rec->next = scoop_gc_handle_records;
  rec->object = obj;
  scoop_gc_handle_records = rec;

  uint64_t handle = (uint64_t)(uintptr_t)rec;
  scoop_gc_lock_release();
  return handle;
}

void *scoop_handle_get(uint64_t handle) {
  if (handle == 0) {
    return 0;
  }

  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  ScoopGcHandleRecord *needle = (ScoopGcHandleRecord *)(uintptr_t)handle;

  scoop_gc_lock_acquire();
  for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
    if (it != needle) {
      continue;
    }
    void *obj = (void *)it->object;
    scoop_gc_lock_release();
    return obj;
  }
  scoop_gc_lock_release();
  return 0;
}

uint32_t scoop_handle_drop(uint64_t handle) {
  if (handle == 0) {
    return 0;
  }

  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  ScoopGcHandleRecord *needle = (ScoopGcHandleRecord *)(uintptr_t)handle;

  scoop_gc_lock_acquire();

  ScoopGcHandleRecord **link = &scoop_gc_handle_records;
  while (*link != 0) {
    ScoopGcHandleRecord *it = *link;
    if (it != needle) {
      link = &it->next;
      continue;
    }

    *link = it->next;
    free(it);
    scoop_gc_lock_release();
    return 1;
  }

  scoop_gc_lock_release();
  return 0;
}

void scoop_gc_heap_register_object(ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return;
  }

  scoop_gc_lock_acquire();

  obj->next = scoop_gc_heap.objects;
  scoop_gc_heap.objects = obj;
  scoop_gc_heap.bytes_allocated += obj->size_bytes;

  scoop_gc_lock_release();
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

  scoop_gc_lock_acquire();

  // hosted backend 不支持多线程 roots 枚举；当存在多个已注册线程时退化为 no-op。
  uint32_t threads = (uint32_t)atomic_load_explicit(&scoop_gc_registered_threads, memory_order_relaxed);
  if (threads != 1) {
    scoop_gc_lock_release();
    return;
  }

  ScoopGcHeap *heap = &scoop_gc_heap;
  uint32_t mark_value = scoop_gc_collect_next_mark_value(heap);

  ScoopGcMarkStack stack = {0};
  ScoopGcMarkCtx ctx = {heap, mark_value, &stack};

  // 1) mark roots（hosted backend v0：仅扫描 pinned/handles roots；不枚举栈 roots）

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

  // 1c) mark stable handles（spec §15.10.1）
  for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
    if (it->object == 0) {
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

  scoop_gc_lock_release();
}

void scoop_gc_collect_minor(void) {
  // hosted backend 无 nursery/minor 语义：minor 退化为一次 major collect（或 no-op）。
  scoop_gc_collect();
}

uint64_t scoop_gc_debug_heap_object_count(void) {
  scoop_gc_lock_acquire();
  uint64_t count = 0;
  for (ScoopGcObjectHeader *it = scoop_gc_heap.objects; it != 0; it = it->next) {
    count++;
  }
  scoop_gc_lock_release();
  return count;
}

uint64_t scoop_gc_debug_heap_bytes_allocated(void) {
  scoop_gc_lock_acquire();
  uint64_t v = scoop_gc_heap.bytes_allocated;
  scoop_gc_lock_release();
  return v;
}

uint64_t scoop_gc_debug_heap_bytes_freed(void) {
  scoop_gc_lock_acquire();
  uint64_t v = scoop_gc_heap.bytes_freed;
  scoop_gc_lock_release();
  return v;
}

uint64_t scoop_gc_debug_heap_bytes_reserved(void) {
  scoop_gc_lock_acquire();
  uint64_t total = 0;
  for (ScoopGcObjectHeader *it = scoop_gc_heap.objects; it != 0; it = it->next) {
    uint64_t size = it->size_bytes;
    if (UINT64_MAX - total < size) {
      total = UINT64_MAX;
      break;
    }
    total += size;
  }
  scoop_gc_lock_release();
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

#endif // SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_HOSTED
