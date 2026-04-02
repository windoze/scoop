// Scoop GC backend: immix (v0; cooperative STW, moving/compacting).
//
// 当前实现（TODO T1406a/T1406b/T1406c/T1407 / PLAN §15.3）：
// - allocator：line/block + hole bump（优先复用 partial blocks，降低碎片化）；
// - mark-region：按对象 trace 标记其覆盖到的 lines；
// - region sweep：基于 line mark/alloc bitmap 回收 holes，并重建可复用 block 列表。
// - moving/compaction：基于 block evacuation 的搬迁与引用修复（forwarding pointer + roots update）。
//
// 限制（v0）：
// - stop-the-world 当前为协作式：线程必须进入 `scoop_gc_safepoint()` 才会被暂停；
// - roots 来源仅为 shadow stack（TODO T1506 会引入 stackmap roots）。

#include "scoop_gc_backend.h"

#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX

#include "scoop_gc.h"
#include "scoop_gc_immix_internal.h"
#include "scoop_tls_internal.h"

#include <pthread.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#include "scoop_gc_stw_internal.h"

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

// --- heap 链表（T1409a：并发 push） ---
//
// 说明：
// - Immix backend 的分配路径在 T1409a 引入 thread-local blocks 后，不再为每次分配持有全局 GC 锁；
// - 因此 heap.objects 的维护需要改为并发安全（lock-free push）；
// - stop-the-world 期间（所有线程 park 后）不会有并发分配，因此 GC 仍可在持锁状态下
//   以“单线程视角”重建/遍历该链表。
static inline ScoopGcObjectHeader *scoop_gc_heap_objects_load_acquire(void) {
  return __atomic_load_n(&scoop_gc_heap.objects, __ATOMIC_ACQUIRE);
}

static inline void scoop_gc_heap_bytes_allocated_add(uint64_t delta) {
  (void)__atomic_fetch_add(&scoop_gc_heap.bytes_allocated, delta, __ATOMIC_RELAXED);
}

static inline void scoop_gc_heap_push_object_atomic(ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return;
  }

  ScoopGcObjectHeader *head = 0;
  do {
    head = scoop_gc_heap_objects_load_acquire();
    obj->next = head;
  } while (!__atomic_compare_exchange_n(&scoop_gc_heap.objects,
                                        &head,
                                        obj,
                                        0,
                                        __ATOMIC_RELEASE,
                                        __ATOMIC_RELAXED));
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

  for (ScoopGcObjectHeader *it = scoop_gc_heap_objects_load_acquire(); it != 0; it = it->next) {
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

// --- 线程注册 + stop-the-world（TODO T1408a） ---
//
// 设计说明（Immix backend，early stage）：
// - roots 来源为 shadow stack（编译器插桩维护 `ScoopGcFrame` 链）；
// - 为在多线程下正确做 mark/compaction，需要在 GC 周期内暂停所有“已注册线程”，并在暂停期间
//   扫描/更新每个线程的 `current_frame` 链；
// - 当前实现为协作式 STW：线程只有在 safepoint 调用 `scoop_gc_safepoint()` 才会 park；
// - 目标优先级：正确性与可回归；性能优化（TLAB/并行标记）留给后续任务（T1409）。

// 线程表 + STW 状态由 Immix `state->lock` 保护（避免引入额外全局锁）。
static pthread_cond_t scoop_gc_stw_cond = PTHREAD_COND_INITIALIZER;
static ScoopGcThreadRecord *scoop_gc_threads = 0;
static uint32_t scoop_gc_thread_count = 0;

static ScoopGcStwState scoop_gc_stw = {0};

static ScoopGcThreadRecord *scoop_gc_find_thread_unlocked(pthread_t t) {
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    if (pthread_equal(it->thread, t)) {
      return it;
    }
  }
  return 0;
}

static void scoop_gc_stop_the_world_begin_unlocked(pthread_t initiator) {
  scoop_gc_stw_requested_store(&scoop_gc_stw, 1);
  scoop_gc_stw.initiator = initiator;
  scoop_gc_stw.epoch += 1;
  scoop_gc_stw.parked_count = 0;

  // 重置线程状态，避免上一轮残留（健壮性；对齐未来 T1505 的状态机语义）。
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    it->state = SCOOP_GC_THREAD_RUNNING;
    it->parked_epoch = 0;
  }

  // 需要 park 的线程数量：所有已注册线程 - initiator（若 initiator 已注册）。
  uint32_t need_to_park = scoop_gc_thread_count;
  if (scoop_gc_find_thread_unlocked(initiator) != 0 && need_to_park > 0) {
    need_to_park -= 1;
  }

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0 || !state->lock_inited) {
    return;
  }

  while (scoop_gc_stw.parked_count < need_to_park) {
    struct timespec ts;
    scoop_gc_stw_timespec_after_ms((uint32_t)SCOOP_GC_STW_DIAG_INTERVAL_MS, &ts);

    int rc = pthread_cond_timedwait(&scoop_gc_stw_cond, &state->lock, &ts);
    if (rc == ETIMEDOUT) {
      scoop_gc_stw_diag_dump_threads_unlocked(&scoop_gc_stw, scoop_gc_threads, need_to_park);
    }
  }
}

static void scoop_gc_stop_the_world_end_unlocked(void) {
  scoop_gc_stw_requested_store(&scoop_gc_stw, 0);
  scoop_gc_stw.parked_count = 0;

  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    it->state = SCOOP_GC_THREAD_RUNNING;
    it->parked_epoch = 0;
  }

  (void)pthread_cond_broadcast(&scoop_gc_stw_cond);
}

void scoop_gc_thread_register(ScoopGcFrame **current_frame_slot) {
  if (current_frame_slot == 0) {
    return;
  }

  pthread_t self = pthread_self();
  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return;
  }

  scoop_gc_immix_lock(state);

  // 若当前有其它线程正在进行 stop-the-world，则等它结束后再注册，避免破坏 STW 计数。
  while (scoop_gc_stw_requested_load(&scoop_gc_stw) && !pthread_equal(self, scoop_gc_stw.initiator)) {
    (void)pthread_cond_wait(&scoop_gc_stw_cond, &state->lock);
  }

  ScoopGcThreadRecord *existing = scoop_gc_find_thread_unlocked(self);
  if (existing != 0) {
    existing->current_frame_slot = current_frame_slot;
    existing->gc_alloc_block_slot =
        scoop_tls_gc_immix_current_block_slot_from_current_frame_slot(current_frame_slot);
    existing->state = SCOOP_GC_THREAD_RUNNING;
    existing->last_safepoint_epoch = scoop_gc_stw.epoch;
    existing->parked_epoch = 0;
    scoop_gc_immix_unlock(state);
    return;
  }

  ScoopGcThreadRecord *rec = (ScoopGcThreadRecord *)malloc(sizeof(ScoopGcThreadRecord));
  if (rec == 0) {
    scoop_gc_immix_unlock(state);
    return;
  }

  rec->next = scoop_gc_threads;
  rec->thread = self;
  rec->current_frame_slot = current_frame_slot;
  rec->gc_alloc_block_slot =
      scoop_tls_gc_immix_current_block_slot_from_current_frame_slot(current_frame_slot);
  rec->state = SCOOP_GC_THREAD_RUNNING;
  rec->last_safepoint_epoch = scoop_gc_stw.epoch;
  rec->parked_epoch = 0;
  rec->stack_walking_ctx = 0;
  rec->native_roots = 0;
  rec->native_roots_len = 0;

  scoop_gc_threads = rec;
  scoop_gc_thread_count += 1;

  scoop_gc_immix_unlock(state);
}

void scoop_gc_thread_unregister(ScoopGcFrame **current_frame_slot) {
  (void)current_frame_slot;

  pthread_t self = pthread_self();
  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return;
  }

  scoop_gc_immix_lock(state);

  // 若当前有其它线程正在进行 stop-the-world，则等它结束后再注销，避免破坏 STW 计数。
  while (scoop_gc_stw_requested_load(&scoop_gc_stw) && !pthread_equal(self, scoop_gc_stw.initiator)) {
    (void)pthread_cond_wait(&scoop_gc_stw_cond, &state->lock);
  }

  ScoopGcThreadRecord **link = &scoop_gc_threads;
  while (*link != 0) {
    ScoopGcThreadRecord *it = *link;
    if (!pthread_equal(it->thread, self)) {
      link = &it->next;
      continue;
    }

    *link = it->next;
    if (scoop_gc_thread_count > 0) {
      scoop_gc_thread_count -= 1;
    }
    free(it);
    break;
  }

  scoop_gc_immix_unlock(state);
}

void scoop_gc_safepoint(void) {
  // T1409a：fast path（无 STW 时不抢全局锁）。
  if (!scoop_gc_stw_requested_load(&scoop_gc_stw)) {
    return;
  }

  pthread_t self = pthread_self();
  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return;
  }

  scoop_gc_immix_lock(state);

  // 协作式 STW：只有在该线程已注册且不是 initiator 时才会 park。
  ScoopGcThreadRecord *self_rec = scoop_gc_find_thread_unlocked(self);
  if (self_rec != 0) {
    self_rec->last_safepoint_epoch = scoop_gc_stw.epoch;
  }

  while (scoop_gc_stw_requested_load(&scoop_gc_stw) && !pthread_equal(self, scoop_gc_stw.initiator)) {
    ScoopGcThreadRecord *rec = scoop_gc_find_thread_unlocked(self);
    if (rec == 0) {
      // 未注册：不参与 STW（early stage 语义约定）。
      break;
    }

    rec->last_safepoint_epoch = scoop_gc_stw.epoch;

    if (rec->parked_epoch != scoop_gc_stw.epoch) {
      rec->state = SCOOP_GC_THREAD_PARKED;
      rec->parked_epoch = scoop_gc_stw.epoch;
      scoop_gc_stw.parked_count += 1;
      // 唤醒 GC 线程：它可能正在等待 parked_count 达标。
      (void)pthread_cond_broadcast(&scoop_gc_stw_cond);
    }

    (void)pthread_cond_wait(&scoop_gc_stw_cond, &state->lock);
  }

  scoop_gc_immix_unlock(state);
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

  // T1409a：并发 push（分配路径不持锁）。
  scoop_gc_heap_push_object_atomic(obj);
  scoop_gc_heap_bytes_allocated_add(obj->size);
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

// --- Moving / compaction（TODO T1407） ---
//
// 设计要点：
// - forwarding pointer 不占用 `flags/mark`：避免与上层/测试对对象头字段的写入发生冲突；
// - 复用对象头的 `next` 字段存放 forwarding pointer，并用低位 tag 区分“链表 next”与“转发指针”；
// - 只做 block evacuation：整块搬迁其内所有 live 对象；否则在 line-granularity bitmap 上无法安全
//   清空“已搬迁对象”占用的 line（多个对象可共享同一 line）。

#define SCOOP_GC_IMMIX_FORWARDING_TAG ((uintptr_t)1u)

static inline uint32_t scoop_gc_immix_object_is_forwarded(const ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return 0;
  }
  return (((uintptr_t)obj->next) & SCOOP_GC_IMMIX_FORWARDING_TAG) != 0;
}

static inline ScoopGcObjectHeader *scoop_gc_immix_object_forwarding_ptr(
    const ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return 0;
  }
  uintptr_t raw = (uintptr_t)obj->next;
  raw &= ~SCOOP_GC_IMMIX_FORWARDING_TAG;
  return (ScoopGcObjectHeader *)raw;
}

static inline void scoop_gc_immix_object_set_forwarding_ptr(ScoopGcObjectHeader *obj,
                                                            ScoopGcObjectHeader *to) {
  if (obj == 0) {
    return;
  }
  obj->next = (ScoopGcObjectHeader *)(((uintptr_t)to) | SCOOP_GC_IMMIX_FORWARDING_TAG);
}

static inline ScoopGcObjectHeader *scoop_gc_immix_follow_forwarding(ScoopGcObjectHeader *obj) {
  // 防御：限制 forwarding chain 长度，避免错误写入导致死循环。
  for (uint32_t hops = 0; hops < 8; hops++) {
    if (obj == 0) {
      return 0;
    }
    if (!scoop_gc_immix_object_is_forwarded(obj)) {
      return obj;
    }
    obj = scoop_gc_immix_object_forwarding_ptr(obj);
  }
  return obj;
}

static void scoop_gc_immix_update_slot_visitor(void **slot, void *raw_ctx) {
  (void)raw_ctx;
  if (slot == 0) {
    return;
  }
  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)(*slot);
  if (obj == 0) {
    return;
  }
  ScoopGcObjectHeader *updated = scoop_gc_immix_follow_forwarding(obj);
  if (updated != 0 && updated != obj) {
    *slot = (void *)updated;
  }
}

static uint32_t scoop_gc_immix_block_contains_pinned_unlocked(ScoopGcImmixBlock *block) {
  if (block == 0) {
    return 0;
  }

  for (ScoopGcPinnedRecord *it = scoop_gc_pinned_objects; it != 0; it = it->next) {
    if (it->object == 0) {
      continue;
    }
    if (it->pin_count == 0) {
      continue;
    }
    ScoopGcImmixBlock *pinned_block = scoop_gc_immix_block_from_object((void *)it->object);
    if (pinned_block == block) {
      return 1;
    }
  }

  return 0;
}

static uint32_t scoop_gc_immix_block_is_in_list(ScoopGcImmixBlock *head, ScoopGcImmixBlock *needle) {
  for (ScoopGcImmixBlock *it = head; it != 0; it = it->next_free) {
    if (it == needle) {
      return 1;
    }
  }
  return 0;
}

typedef struct ScoopGcImmixMoveRecord {
  ScoopGcObjectHeader *from;
  ScoopGcObjectHeader *to;
  ScoopGcImmixBlock *from_block;
  uint64_t size;
} ScoopGcImmixMoveRecord;

typedef struct ScoopGcImmixToSpace {
  ScoopGcImmixBlock *current;
  // 从 `state->free_blocks` 里借用的空 block（abort 时 reset；commit 时保留并进入 reusable list）。
  ScoopGcImmixBlock *reused_blocks;
  // 新分配但尚未挂到 `state->all_blocks` 的 block（abort 时 free；commit 时挂入 all_blocks）。
  ScoopGcImmixBlock *new_blocks;
} ScoopGcImmixToSpace;

static ScoopGcImmixBlock *scoop_gc_immix_tospace_take_block(ScoopGcImmixToSpace *tospace,
                                                            ScoopGcImmixState *state) {
  if (tospace == 0 || state == 0) {
    return 0;
  }

  ScoopGcImmixBlock *block = 0;
  if (state->free_blocks != 0) {
    block = state->free_blocks;
    state->free_blocks = block->next_free;
    block->next_free = 0;

    // 记录“借用过的 free block”，以便 abort 时 reset（不依赖 free_list 还原顺序）。
    block->next_free = tospace->reused_blocks;
    tospace->reused_blocks = block;
  } else {
    block = scoop_gc_immix_block_alloc_new();
    if (block == 0) {
      return 0;
    }

    block->next_all = tospace->new_blocks;
    tospace->new_blocks = block;
  }

  tospace->current = block;
  return block;
}

static void *scoop_gc_immix_tospace_alloc(ScoopGcImmixToSpace *tospace,
                                         ScoopGcImmixState *state,
                                         uint64_t raw_size) {
  if (tospace == 0 || state == 0 || raw_size == 0) {
    return 0;
  }

  if (raw_size > (uint64_t)SIZE_MAX) {
    return 0;
  }
  size_t size = (size_t)raw_size;

  ScoopGcImmixBlock *block = tospace->current;
  if (block == 0) {
    block = scoop_gc_immix_tospace_take_block(tospace, state);
  }

  for (uint32_t tries = 0; tries < 128; tries++) {
    if (block == 0) {
      return 0;
    }
    void *p = scoop_gc_immix_block_alloc(block, size, (size_t)sizeof(void *));
    if (p != 0) {
      return p;
    }
    block = scoop_gc_immix_tospace_take_block(tospace, state);
  }

  return 0;
}

static void scoop_gc_immix_tospace_abort(ScoopGcImmixToSpace *tospace, ScoopGcImmixState *state) {
  if (tospace == 0 || state == 0) {
    return;
  }

  // 1) reset 复用过的 free blocks（它们已在 all_blocks 中，无需额外释放）
  ScoopGcImmixBlock *rb = tospace->reused_blocks;
  while (rb != 0) {
    ScoopGcImmixBlock *next = rb->next_free;
    scoop_gc_immix_block_reset(rb);
    rb = next;
  }

  // 2) 释放新分配的 blocks（它们尚未挂入 all_blocks）
  ScoopGcImmixBlock *b = tospace->new_blocks;
  while (b != 0) {
    ScoopGcImmixBlock *next = b->next_all;
    free(b);
    b = next;
  }

  tospace->current = 0;
  tospace->reused_blocks = 0;
  tospace->new_blocks = 0;
}

static void scoop_gc_immix_state_rebuild_block_lists(ScoopGcImmixState *state) {
  if (state == 0) {
    return;
  }

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

    scoop_gc_immix_block_setup_first_hole(it);
    if (it->cursor < it->limit) {
      it->next_free = state->reusable_blocks;
      state->reusable_blocks = it;
    }
  }
}

static void scoop_gc_immix_state_remove_and_free_block(ScoopGcImmixState *state,
                                                       ScoopGcImmixBlock *block) {
  if (state == 0 || block == 0) {
    return;
  }

  ScoopGcImmixBlock **link = &state->all_blocks;
  while (*link != 0) {
    ScoopGcImmixBlock *it = *link;
    if (it != block) {
      link = &it->next_all;
      continue;
    }

    *link = it->next_all;
    free(it);
    return;
  }
}

static void scoop_gc_immix_compact(ScoopGcImmixState *state,
                                   ScoopGcHeap *heap,
                                   ScoopGcImmixBlock *evac_blocks) {
  if (state == 0 || heap == 0 || evac_blocks == 0) {
    return;
  }

  // 0) snapshot：把当前 heap.objects（已完成 sweep 的 live 集合）拍成数组，
  //    避免后续写入 forwarding pointer 破坏链表遍历。
  size_t live_len = 0;
  for (ScoopGcObjectHeader *it = heap->objects; it != 0; it = it->next) {
    live_len += 1;
  }
  if (live_len == 0) {
    scoop_gc_immix_state_rebuild_block_lists(state);
    return;
  }

  if (live_len > (SIZE_MAX / sizeof(ScoopGcObjectHeader *))) {
    scoop_gc_immix_state_rebuild_block_lists(state);
    return;
  }
  ScoopGcObjectHeader **live =
      (ScoopGcObjectHeader **)malloc(live_len * sizeof(ScoopGcObjectHeader *));
  if (live == 0) {
    scoop_gc_immix_state_rebuild_block_lists(state);
    return;
  }

  size_t idx = 0;
  for (ScoopGcObjectHeader *it = heap->objects; it != 0 && idx < live_len; it = it->next) {
    live[idx++] = it;
  }
  live_len = idx;

  // 1) 统计需要搬迁的对象（仅限：位于待 evacuation blocks 内的 small objects）
  size_t move_len = 0;
  for (size_t i = 0; i < live_len; i++) {
    ScoopGcObjectHeader *obj = live[i];
    ScoopGcImmixBlock *block = scoop_gc_immix_block_from_object((void *)obj);
    if (block == 0) {
      continue;
    }
    if (!scoop_gc_immix_block_is_in_list(evac_blocks, block)) {
      continue;
    }
    move_len += 1;
  }

  if (move_len == 0) {
    scoop_gc_immix_state_rebuild_block_lists(state);
    free(live);
    return;
  }

  if (move_len > (SIZE_MAX / sizeof(ScoopGcImmixMoveRecord))) {
    scoop_gc_immix_state_rebuild_block_lists(state);
    free(live);
    return;
  }
  ScoopGcImmixMoveRecord *moves =
      (ScoopGcImmixMoveRecord *)malloc(move_len * sizeof(ScoopGcImmixMoveRecord));
  if (moves == 0) {
    scoop_gc_immix_state_rebuild_block_lists(state);
    free(live);
    return;
  }

  // 2) to-space 分配与拷贝（可回滚）：若任一步失败，则 reset/free to-space 并放弃本轮 compaction。
  ScoopGcImmixToSpace tospace = {0};
  size_t written = 0;

  for (size_t i = 0; i < live_len; i++) {
    ScoopGcObjectHeader *from = live[i];
    ScoopGcImmixBlock *from_block = scoop_gc_immix_block_from_object((void *)from);
    if (from_block == 0) {
      continue;
    }
    if (!scoop_gc_immix_block_is_in_list(evac_blocks, from_block)) {
      continue;
    }

    uint64_t raw_size = from->size;
    void *p = scoop_gc_immix_tospace_alloc(&tospace, state, raw_size);
    if (p == 0) {
      scoop_gc_immix_tospace_abort(&tospace, state);
      scoop_gc_immix_state_rebuild_block_lists(state);
      free(moves);
      free(live);
      return;
    }

    // to-space 里的对象是“真实 heap 对象”：拷贝 header+payload，保持 type_desc/mark 等一致。
    size_t size = (raw_size > (uint64_t)SIZE_MAX) ? (size_t)SIZE_MAX : (size_t)raw_size;
    (void)memcpy(p, (const void *)from, size);

    ScoopGcObjectHeader *to = (ScoopGcObjectHeader *)p;
    // to 对象将由我们重建 heap 链表，因此清空 next，避免携带旧链表指针。
    to->next = 0;

    moves[written].from = from;
    moves[written].to = to;
    moves[written].from_block = from_block;
    moves[written].size = raw_size;
    written += 1;
  }

  move_len = written;
  if (move_len == 0) {
    scoop_gc_immix_tospace_abort(&tospace, state);
    scoop_gc_immix_state_rebuild_block_lists(state);
    free(moves);
    free(live);
    return;
  }

  // 3) 提交：写入 forwarding pointer + 更新 roots + 修复对象内部引用槽位。
  for (size_t i = 0; i < move_len; i++) {
    ScoopGcObjectHeader *from = moves[i].from;
    ScoopGcObjectHeader *to = moves[i].to;
    scoop_gc_immix_object_set_forwarding_ptr(from, to);

    ScoopGcImmixBlock *from_block = moves[i].from_block;
    if (from_block != 0 && from_block->live_objects > 0) {
      from_block->live_objects -= 1;
    }
  }

  // 3a) roots update：shadow stack slots 原地改写为新地址（moving GC 的关键语义）。
  //
  // 注意：必须更新“所有已注册线程”的 roots；否则在多线程 + moving/compaction 下会产生悬挂指针。
  uint64_t scoop_gc_shadow_stack_visit_roots_from_frame(ScoopGcFrame *frame,
                                                        ScoopGcTraceVisitor visitor,
                                                        void *ctx);
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    if (it->current_frame_slot == 0) {
      continue;
    }
    ScoopGcFrame *frame = *(it->current_frame_slot);
    (void)scoop_gc_shadow_stack_visit_roots_from_frame(frame, scoop_gc_immix_update_slot_visitor, 0);
  }

  // 3b) heap object fields update：扫描所有 live 对象（对已搬迁对象改为扫描其 to-space 副本）。
  for (size_t i = 0; i < live_len; i++) {
    ScoopGcObjectHeader *obj = live[i];
    if (obj == 0) {
      continue;
    }

    ScoopGcObjectHeader *current = obj;
    if (scoop_gc_immix_object_is_forwarded(obj)) {
      current = scoop_gc_immix_object_forwarding_ptr(obj);
    }
    if (current == 0) {
      continue;
    }
    if (current->type_desc == 0) {
      continue;
    }

    (void)scoop_gc_type_descriptor_trace(current->type_desc,
                                         (void *)current,
                                         scoop_gc_immix_update_slot_visitor,
                                         0);
  }

  // 4) 重建 heap.objects：保留未搬迁对象 + 追加 to-space 副本；from-space 旧对象从 heap 链表中移除。
  ScoopGcObjectHeader *new_list = 0;
  for (size_t i = 0; i < live_len; i++) {
    ScoopGcObjectHeader *obj = live[i];
    if (obj == 0) {
      continue;
    }
    if (scoop_gc_immix_object_is_forwarded(obj)) {
      continue;
    }
    obj->next = new_list;
    new_list = obj;
  }
  for (size_t i = 0; i < move_len; i++) {
    ScoopGcObjectHeader *obj = moves[i].to;
    if (obj == 0) {
      continue;
    }
    obj->next = new_list;
    new_list = obj;
  }
  heap->objects = new_list;

  // 5) 将 to-space 新 block 挂入 all_blocks；随后可统一 rebuild free/reusable list。
  ScoopGcImmixBlock *nb = tospace.new_blocks;
  while (nb != 0) {
    ScoopGcImmixBlock *next = nb->next_all;
    nb->next_all = state->all_blocks;
    state->all_blocks = nb;
    nb = next;
  }
  tospace.new_blocks = 0;

  // 6) 释放已 evacuation 的 blocks：必须是“整块搬空”（live_objects==0），否则无法安全回收 bitmap。
  ScoopGcImmixBlock *eb = evac_blocks;
  while (eb != 0) {
    ScoopGcImmixBlock *next = eb->next_free;
    if (eb->live_objects == 0) {
      scoop_gc_immix_state_remove_and_free_block(state, eb);
    }
    eb = next;
  }

  // 7) 重新构建 free/reusable block lists，确保 allocator 能继续工作且不包含悬挂指针。
  scoop_gc_immix_state_rebuild_block_lists(state);

  free(moves);
  free(live);
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
  pthread_t self = pthread_self();

  scoop_gc_immix_lock(state);

  // 保证同一时刻只允许一个 GC 周期。
  while (scoop_gc_stw_requested_load(&scoop_gc_stw)) {
    (void)pthread_cond_wait(&scoop_gc_stw_cond, &state->lock);
  }

  scoop_gc_stop_the_world_begin_unlocked(self);

  // T1409a：在 stop-the-world 达成后，清空所有线程的 thread-local current block 指针，
  // 避免 moving/compaction/free block 后出现悬挂指针（use-after-free）。
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    if (it->gc_alloc_block_slot == 0) {
      continue;
    }
    *(it->gc_alloc_block_slot) = 0;
  }

  ScoopGcHeap *heap = &scoop_gc_heap;
  uint32_t mark_value = scoop_gc_collect_next_mark_value(heap);

  ScoopGcMarkStack stack = {0};
  ScoopGcMarkCtx ctx = {heap, mark_value, &stack};

  // 0) clear per-block mark bitmap（避免上一轮残留影响 region sweep）
  for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
    scoop_gc_immix_block_clear_mark_bits(it);
  }

  // 1) mark roots（扫描所有已注册线程的 shadow stack）
  uint64_t scoop_gc_shadow_stack_visit_roots_from_frame(ScoopGcFrame *frame,
                                                        ScoopGcTraceVisitor visitor,
                                                        void *ctx);
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    if (it->current_frame_slot == 0) {
      continue;
    }

    ScoopGcFrame *frame = *(it->current_frame_slot);
    (void)scoop_gc_shadow_stack_visit_roots_from_frame(frame, scoop_gc_mark_visitor, (void *)&ctx);
  }

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
  ScoopGcImmixBlock *evac_blocks = 0;

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
    size_t live_lines = 0;
    for (size_t line = reserved; line < (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK; line++) {
      uint32_t live = scoop_gc_immix_bitmap_test_bit(it->line_mark_bits,
                                                     SCOOP_GC_IMMIX_BITMAP_WORDS,
                                                     line);
      if (live) {
        live_lines += 1;
        scoop_gc_immix_bitmap_set_bit(it->line_alloc_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
      } else {
        scoop_gc_immix_bitmap_clear_bit(it->line_alloc_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
      }
      scoop_gc_immix_bitmap_clear_bit(it->line_mark_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
    }

    // 准备第一个 hole：之后分配可以在 hole 内 bump。
    scoop_gc_immix_block_setup_first_hole(it);

    // moving/compaction（T1407）：选择性 block evacuation。
    //
    // v0 策略（保守但可回归）：
    // - 仅对“足够稀疏”的 blocks 做 evacuation（live lines <= 25%）；
    // - 若 block 内存在 pinned 对象，则跳过（pin 语义要求地址稳定）。
    //
    // 注：该策略不追求最优，只保证“可触发移动 + 引用修复”与基本碎片化控制。
    size_t total_lines = (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK - reserved;
    uint32_t is_sparse = 0;
    if (total_lines > 0 && live_lines > 0) {
      is_sparse = (live_lines * 4u) <= total_lines;
    }
    uint32_t has_pinned = scoop_gc_immix_block_contains_pinned_unlocked(it);

    if (it->cursor < it->limit) {
      if (is_sparse && !has_pinned) {
        it->next_free = evac_blocks;
        evac_blocks = it;
      } else {
        it->next_free = state->reusable_blocks;
        state->reusable_blocks = it;
      }
    }
  }

  // 5) moving/compaction：对候选 blocks 做 evacuation，并更新 roots 与 heap 引用槽位。
  if (evac_blocks != 0) {
    scoop_gc_immix_compact(state, heap, evac_blocks);
  }

  scoop_gc_stop_the_world_end_unlocked();
  scoop_gc_immix_unlock(state);
}

uint64_t scoop_gc_debug_heap_object_count(void) {
  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return 0;
  }
  scoop_gc_immix_lock(state);
  uint64_t count = 0;
  for (ScoopGcObjectHeader *it = scoop_gc_heap_objects_load_acquire(); it != 0; it = it->next) {
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
  uint64_t v = __atomic_load_n(&scoop_gc_heap.bytes_allocated, __ATOMIC_RELAXED);
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

uint64_t scoop_gc_debug_heap_bytes_reserved(void) {
  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return 0;
  }
  scoop_gc_immix_lock(state);

  uint64_t total = 0;

  // 1) Immix blocks：以固定 block size 计入“已保留的 heap 空间”。
  for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
    uint64_t block_bytes = (uint64_t)SCOOP_GC_IMMIX_BLOCK_SIZE;
    if (UINT64_MAX - total < block_bytes) {
      total = UINT64_MAX;
      break;
    }
    total += block_bytes;
  }

  // 2) large objects / fallback malloc：它们不在任何 block 内，需要单独计入。
  if (total != UINT64_MAX) {
    for (ScoopGcObjectHeader *obj = scoop_gc_heap_objects_load_acquire(); obj != 0; obj = obj->next) {
      ScoopGcImmixBlock *block = scoop_gc_immix_block_from_object((void *)obj);
      if (block != 0) {
        continue;
      }

      uint64_t size = obj->size;
      if (UINT64_MAX - total < size) {
        total = UINT64_MAX;
        break;
      }
      total += size;
    }
  }

  scoop_gc_immix_unlock(state);
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

#endif // SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX
