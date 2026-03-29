// Scoop GC runtime (early stage).
//
// TODO T0904：mark-sweep GC 的数据结构骨架。
// TODO T0910：实现最小可用的单线程 mark-sweep（手动触发）。

#include "scoop_gc.h"

#include <pthread.h>
#include <stddef.h>
#include <stdlib.h>

// --- 线程注册 + stop-the-world（TODO T0911） ---
//
// 设计说明（early stage）：
// - 当前 GC v0 的根集来自 shadow stack（编译器插桩维护 `ScoopGcFrame` 链）。
// - 为了在多线程下正确扫描所有线程的 shadow stack，需要在 GC 期间暂停（stop-the-world）
//   所有“已注册线程”，并在暂停期间枚举每个线程的 `current_frame` 链。
// - 该实现采用“协作式 STW”：线程必须在 safepoint 调用 `scoop_gc_safepoint()` 才会被暂停。
//   后续编译器会在需要的位置插入 safepoint（例如分配/循环回边等）。
//
// 约束：
// - 该实现优先满足“可验证且不崩溃”的语义，不追求性能。
// - 线程必须显式调用 `scoop_thread_register/unregister`（由 runtime 侧提供）以参与 GC STW。

typedef struct ScoopGcThreadRecord {
  struct ScoopGcThreadRecord *next;
  pthread_t thread;
  // 指向该线程 TLS 内的 `current_frame` 槽位（由 runtime 注册时传入）。
  ScoopGcFrame **current_frame_slot;
  // 1 表示该线程已进入 STW park；用于避免重复计数。
  uint32_t parked;
} ScoopGcThreadRecord;

static pthread_mutex_t scoop_gc_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t scoop_gc_cond = PTHREAD_COND_INITIALIZER;

static ScoopGcThreadRecord *scoop_gc_threads = 0;
static uint32_t scoop_gc_thread_count = 0;

// STW 状态（由 `scoop_gc_collect` 驱动）。
static uint32_t scoop_gc_stw_requested = 0;
static pthread_t scoop_gc_stw_initiator;
static uint32_t scoop_gc_stw_parked_count = 0;

static ScoopGcThreadRecord *scoop_gc_find_thread_unlocked(pthread_t t) {
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    if (pthread_equal(it->thread, t)) {
      return it;
    }
  }
  return 0;
}

// runtime 侧在 `scoop_thread_register/unregister` 中调用这些函数，把线程纳入 GC 的 STW 范围。
void scoop_gc_thread_register(ScoopGcFrame **current_frame_slot) {
  if (current_frame_slot == 0) {
    return;
  }

  pthread_t self = pthread_self();

  (void)pthread_mutex_lock(&scoop_gc_lock);

  ScoopGcThreadRecord *existing = scoop_gc_find_thread_unlocked(self);
  if (existing != 0) {
    existing->current_frame_slot = current_frame_slot;
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return;
  }

  ScoopGcThreadRecord *rec = (ScoopGcThreadRecord *)malloc(sizeof(ScoopGcThreadRecord));
  if (rec == 0) {
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return;
  }

  rec->next = scoop_gc_threads;
  rec->thread = self;
  rec->current_frame_slot = current_frame_slot;
  rec->parked = 0;

  scoop_gc_threads = rec;
  scoop_gc_thread_count += 1;

  (void)pthread_mutex_unlock(&scoop_gc_lock);
}

void scoop_gc_thread_unregister(ScoopGcFrame **current_frame_slot) {
  (void)current_frame_slot;
  pthread_t self = pthread_self();

  (void)pthread_mutex_lock(&scoop_gc_lock);

  // 若当前有其它线程正在进行 STW，则等它结束后再注销，避免破坏 stop-the-world 计数。
  while (scoop_gc_stw_requested && !pthread_equal(self, scoop_gc_stw_initiator)) {
    (void)pthread_cond_wait(&scoop_gc_cond, &scoop_gc_lock);
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

  (void)pthread_mutex_unlock(&scoop_gc_lock);
}

// safepoint：若 GC 正在请求 STW，则当前线程在此处 park，直到 GC 结束。
void scoop_gc_safepoint(void) {
  pthread_t self = pthread_self();

  (void)pthread_mutex_lock(&scoop_gc_lock);

  // 协作式 STW：只有在该线程已注册且不是 initiator 时才会 park。
  while (scoop_gc_stw_requested && !pthread_equal(self, scoop_gc_stw_initiator)) {
    ScoopGcThreadRecord *rec = scoop_gc_find_thread_unlocked(self);
    if (rec == 0) {
      // 未注册：不参与 STW（early stage 语义约定）。
      break;
    }

    if (!rec->parked) {
      rec->parked = 1;
      scoop_gc_stw_parked_count += 1;
      // 唤醒 GC 线程：它可能正在等待 parked_count 达标。
      (void)pthread_cond_broadcast(&scoop_gc_cond);
    }

    (void)pthread_cond_wait(&scoop_gc_cond, &scoop_gc_lock);
  }

  (void)pthread_mutex_unlock(&scoop_gc_lock);
}

// scope helper：进入 stop-the-world（等待其它线程 park）。
static void scoop_gc_stop_the_world_begin_unlocked(pthread_t initiator) {
  scoop_gc_stw_requested = 1;
  scoop_gc_stw_initiator = initiator;
  scoop_gc_stw_parked_count = 0;

  // 重置 parked 标记，避免上一轮残留（健壮性）。
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    it->parked = 0;
  }

  // 需要 park 的线程数量：所有已注册线程 - initiator（若 initiator 已注册）。
  uint32_t need_to_park = scoop_gc_thread_count;
  if (scoop_gc_find_thread_unlocked(initiator) != 0 && need_to_park > 0) {
    need_to_park -= 1;
  }

  while (scoop_gc_stw_parked_count < need_to_park) {
    (void)pthread_cond_wait(&scoop_gc_cond, &scoop_gc_lock);
  }
}

static void scoop_gc_stop_the_world_end_unlocked(void) {
  scoop_gc_stw_requested = 0;
  scoop_gc_stw_parked_count = 0;

  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    it->parked = 0;
  }

  (void)pthread_cond_broadcast(&scoop_gc_cond);
}

// 进程全局 heap（v0：单线程）。
//
// 说明：
// - 该符号不在头文件中导出；对外通过 `scoop_alloc`/`scoop_gc_collect` 等 API 访问；
// - 多线程 stop-the-world 与 per-thread allocator 将在后续任务（T0911+）补齐。
ScoopGcHeap scoop_gc_heap;

// --- Pinning（spec §15.10 / TODO T0912） ---
//
// 说明（early stage）：
// - 在移动/压缩 GC 中，pin 的核心语义是“对象地址稳定”；v0 非移动 GC 下对象不会移动，
//   但 pin 仍必须保证“对象在 pin 期间被保活（视为 root）”以及“pin/unpin 配对检查”。
// - 为了便于单独回归验证，这里采用“每对象 pin 计数”的实现：同一对象可被多次 pin，
//   需对应次数 unpin；当计数归零时从 pinned 集合移除。
// - v0 实现选择用链表保存 pinned 集合（对象数不大，且该 API 为 @Unsafe 低频路径）。
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

uint32_t scoop_pin(void *raw_obj) {
  if (raw_obj == 0) {
    return 0;
  }

  // 说明：保持与其它 runtime API 一致：允许在未显式 init/register 的情况下被调用。
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)raw_obj;

  (void)pthread_mutex_lock(&scoop_gc_lock);

  // 健壮性：只允许 pin 由 `scoop_alloc` 分配并登记到 heap 的对象，避免 GC 在后续扫描
  // pinned roots 时对非法指针解引用导致崩溃。
  if (!scoop_gc_heap_contains_object_unlocked(obj)) {
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return 0;
  }

  ScoopGcPinnedRecord *rec = scoop_gc_find_pinned_unlocked(obj);
  if (rec != 0) {
    if (rec->pin_count == UINT64_MAX) {
      // overflow：保守失败（避免 wrap 导致“错误解 pin”）。
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

  // 说明：与 `scoop_pin` 对齐：确保 runtime init + 当前线程参与 STW。
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

    // 找到了：递减计数；归零则移除节点。
    if (it->pin_count == 0) {
      // 理论上不会发生；保守失败（且不崩溃）。
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

  // 未找到：unpin 下溢（对未 pin 的对象 unpin，或重复 unpin）。
  (void)pthread_mutex_unlock(&scoop_gc_lock);
  return 0;
}

void scoop_gc_heap_register_object(ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return;
  }

  // 说明：heap 链表与统计字段是进程全局共享状态；在多线程下需加锁保护。
  (void)pthread_mutex_lock(&scoop_gc_lock);

  obj->next = scoop_gc_heap.objects;
  scoop_gc_heap.objects = obj;
  scoop_gc_heap.bytes_allocated += obj->size;

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
  // v0->v0+：协作式 stop-the-world，扫描所有已注册线程 roots。
  //
  // 说明：
  // - 该函数会阻塞直到其它注册线程在 safepoint 处 park（`scoop_gc_safepoint()`）。
  // - 若有线程注册但从不进入 safepoint，本函数可能无限等待（early stage 限制）。

  // 先确保 runtime 已 init 且当前线程已注册（便于被纳入 roots 枚举）。
  //
  // 注意：这些函数定义在 `scoop_runtime.c`，这里用本地声明以避免头文件耦合。
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  pthread_t self = pthread_self();

  (void)pthread_mutex_lock(&scoop_gc_lock);

  // 保证同一时刻只允许一个 GC 周期。
  while (scoop_gc_stw_requested) {
    (void)pthread_cond_wait(&scoop_gc_cond, &scoop_gc_lock);
  }

  scoop_gc_stop_the_world_begin_unlocked(self);

  ScoopGcHeap *heap = &scoop_gc_heap;
  uint32_t mark_value = scoop_gc_collect_next_mark_value(heap);

  ScoopGcMarkStack stack = {0};
  ScoopGcMarkCtx ctx = {heap, mark_value, &stack};

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

  // 1b) mark pinned roots（spec §15.10）：pinned 对象必须保活，即使没有 shadow stack 引用。
  for (ScoopGcPinnedRecord *it = scoop_gc_pinned_objects; it != 0; it = it->next) {
    if (it->object == 0) {
      continue;
    }
    if (it->pin_count == 0) {
      continue;
    }

    scoop_gc_mark_object_if_needed(&ctx, it->object);
  }

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

    // 若该类型提供 release 回调，则在释放对象内存前调用它。
    //
    // 注意：该回调运行在 GC 锁 + stop-the-world 的受限上下文中；应避免分配与 re-enter GC。
    if (obj->type_desc != 0 && obj->type_desc->release_fn != 0) {
      obj->type_desc->release_fn((void *)obj);
    }

    heap->bytes_freed += obj->size;
    free(obj);
  }

  scoop_gc_stop_the_world_end_unlocked();
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
