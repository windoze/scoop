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
// - GC roots 枚举不再扫描 shadow stack：
//   - InNative 线程：roots 来自 `native_roots` slots（enter_native 注册）；
//   - 其余线程：roots 来自 stack walking ctx + stackmap records（Parked/initiator）；
//   - pinned objects 与 stable handles 作为全局 roots 单独扫描。

#include "scoop_gc_backend.h"

#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_IMMIX

#include "scoop_gc.h"
#include "scoop_gc_immix_internal.h"
#include "scoop_stackmap.h"
#include "scoop_tls_internal.h"

#include <errno.h>
#include <limits.h>
#include <pthread.h>
#include <sched.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#include "scoop_gc_root_map_internal.h"
#include "platform/unwind.h"
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

// `scoop_gc_safepoint_poll` 定义在本 backend 内；这里前置声明以避免 C99 的隐式声明错误。
void scoop_gc_safepoint_poll(void);

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

// --- Stable handles（spec §15.10.1 / TODO T1510a） ---
//
// 说明：
// - handle 表必须被 GC 当作 roots（否则对象可能在没有任何 roots 引用时被回收）；
// - Immix backend 支持 moving/compaction，因此在 evacuation 后必须更新 handle->obj 槽位，
//   避免 native 通过 handle_get() 读到悬挂指针。
typedef struct ScoopGcHandleRecord {
  struct ScoopGcHandleRecord *next;
  ScoopGcObjectHeader *object;
} ScoopGcHandleRecord;

static ScoopGcHandleRecord *scoop_gc_handle_records = 0;

typedef struct ScoopGcGlobalRootRecord {
  struct ScoopGcGlobalRootRecord *next;
  void *base;
  const ScoopTypeDescriptor *type_desc;
} ScoopGcGlobalRootRecord;

static ScoopGcGlobalRootRecord *scoop_gc_global_roots = 0;

static uint64_t scoop_gc_global_roots_visit_unlocked(ScoopGcTraceVisitor visitor, void *ctx) {
  if (visitor == 0) {
    return 0;
  }

  uint64_t visited = 0;
  for (ScoopGcGlobalRootRecord *it = scoop_gc_global_roots; it != 0; it = it->next) {
    if (it->base == 0 || it->type_desc == 0) {
      continue;
    }
    visited += scoop_gc_type_descriptor_trace(it->type_desc, it->base, visitor, ctx);
  }
  return visited;
}

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
// - roots 来源为 stackmap（Parked/initiator）+ native_roots（InNative）+ pinned/handles；
// - 为在多线程下正确做 mark/compaction，需要在 GC 周期内暂停所有“已注册线程”，并在暂停期间
//   扫描/更新每个线程的 roots slots（stackmap spill slots / native_roots slots / handle 表槽位）；
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

static uint64_t scoop_gc_native_roots_visit_slots(void *native_roots,
                                                  uint32_t native_roots_len,
                                                  ScoopGcTraceVisitor visitor,
                                                  void *ctx) {
  if (native_roots == 0 || native_roots_len == 0 || visitor == 0) {
    return 0;
  }

  // `native_roots` 表示一个 “void** slots” 的指针数组（即 `void***`）。
  void ***slots = (void ***)native_roots;

  uint64_t visited = 0;
  for (uint32_t i = 0; i < native_roots_len; i++) {
    void **slot = slots[i];
    if (slot == 0) {
      continue;
    }
    visitor(slot, ctx);
    visited += 1;
  }

  return visited;
}

static void scoop_gc_stop_the_world_end_unlocked(void);

static int scoop_gc_timespec_cmp(const struct timespec *a, const struct timespec *b) {
  if (a == 0 || b == 0) {
    return 0;
  }
  if (a->tv_sec < b->tv_sec) {
    return -1;
  }
  if (a->tv_sec > b->tv_sec) {
    return 1;
  }
  if (a->tv_nsec < b->tv_nsec) {
    return -1;
  }
  if (a->tv_nsec > b->tv_nsec) {
    return 1;
  }
  return 0;
}

static uint32_t scoop_gc_stop_the_world_begin_prepare_unlocked(pthread_t initiator) {
  scoop_gc_stw_requested_store(&scoop_gc_stw, 1);
  scoop_gc_stw.initiator = initiator;
  scoop_gc_stw.epoch += 1;
  scoop_gc_stw.parked_count = 0;

  // 重置线程状态，避免上一轮残留（健壮性；对齐未来 T1505 的状态机语义）。
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    // T1505c：保留 InNative 线程状态；否则 GC 会错误等待其 park，导致死锁。
    if (it->state != SCOOP_GC_THREAD_IN_NATIVE) {
      it->state = SCOOP_GC_THREAD_RUNNING;
      // 非 InNative 线程的 ctx 只在本轮 STW 内有效；新一轮开始前必须清空。
      scoop_platform_unwind_ctx_destroy(it->stack_walking_ctx);
      it->stack_walking_ctx = 0;
    }
    it->parked_epoch = 0;
  }

  // 需要 park 的线程数量：
  // - initiator 不需要 park；
  // - InNative 线程视为“已就绪”（roots 来自 native_roots），因此也不需要 park。
  uint32_t need_to_park = 0;
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    if (pthread_equal(it->thread, initiator)) {
      continue;
    }
    if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
      continue;
    }
    need_to_park += 1;
  }

  return need_to_park;
}

static void scoop_gc_stop_the_world_begin_unlocked(pthread_t initiator) {
  const uint32_t need_to_park = scoop_gc_stop_the_world_begin_prepare_unlocked(initiator);

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

static uint32_t scoop_gc_stop_the_world_try_begin_unlocked(pthread_t initiator, uint32_t deadline_ms) {
  const uint32_t need_to_park = scoop_gc_stop_the_world_begin_prepare_unlocked(initiator);

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0 || !state->lock_inited) {
    scoop_gc_stop_the_world_end_unlocked();
    return 0;
  }

  if (need_to_park == 0) {
    return 1;
  }

  // deadline=0：表示“不要等待其它线程进入 safepoint”，直接放弃本轮 STW/minor。
  if (deadline_ms == 0) {
    scoop_gc_stop_the_world_end_unlocked();
    return 0;
  }

  struct timespec deadline_ts;
  scoop_gc_stw_timespec_after_ms(deadline_ms, &deadline_ts);

  while (scoop_gc_stw.parked_count < need_to_park) {
    struct timespec diag_ts;
    scoop_gc_stw_timespec_after_ms((uint32_t)SCOOP_GC_STW_DIAG_INTERVAL_MS, &diag_ts);

    struct timespec ts = diag_ts;
    uint32_t is_deadline_wait = 0;
    if (scoop_gc_timespec_cmp(&deadline_ts, &diag_ts) <= 0) {
      ts = deadline_ts;
      is_deadline_wait = 1;
    }

    int rc = pthread_cond_timedwait(&scoop_gc_stw_cond, &state->lock, &ts);
    if (rc == ETIMEDOUT) {
      if (is_deadline_wait) {
        // 未能在 deadline 内达成 STW：撤销请求并唤醒已 park 线程（避免卡住渲染帧）。
        scoop_gc_stop_the_world_end_unlocked();
        return 0;
      }
      scoop_gc_stw_diag_dump_threads_unlocked(&scoop_gc_stw, scoop_gc_threads, need_to_park);
    }
  }

  return 1;
}

static void scoop_gc_stop_the_world_end_unlocked(void) {
  scoop_gc_stw_requested_store(&scoop_gc_stw, 0);
  scoop_gc_stw.parked_count = 0;

  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    // T1505c：InNative 状态由 `leave_native()` 显式恢复，不在 STW end 中强制切回 Running。
    if (it->state == SCOOP_GC_THREAD_PARKED) {
      it->state = SCOOP_GC_THREAD_RUNNING;
    }
    it->parked_epoch = 0;
    // T1512c：InNative 线程需要保留 enter_native 时捕获的 ctx，用于 native 期间枚举更高层
    // managed caller frames；其余线程的 ctx 只在当前 STW 内有效，结束后必须清空。
    if (it->state != SCOOP_GC_THREAD_IN_NATIVE) {
      scoop_platform_unwind_ctx_destroy(it->stack_walking_ctx);
      it->stack_walking_ctx = 0;
    }
  }

  (void)pthread_cond_broadcast(&scoop_gc_stw_cond);
}

typedef struct ScoopTestGcCtxWorkerArgs {
  uint32_t *stop;
  uint64_t *poll_count;
} ScoopTestGcCtxWorkerArgs;

static void *scoop_test_gc_ctx_worker_entry(void *raw_args) {
  void scoop_thread_register(void);
  void scoop_thread_unregister(void);

  ScoopTestGcCtxWorkerArgs *args = (ScoopTestGcCtxWorkerArgs *)raw_args;
  if (args == 0 || args->stop == 0 || args->poll_count == 0) {
    return 0;
  }

  scoop_thread_register();

  while (!__atomic_load_n(args->stop, __ATOMIC_SEQ_CST)) {
    scoop_gc_safepoint_poll();
    (void)__atomic_fetch_add(args->poll_count, 1, __ATOMIC_SEQ_CST);
    sched_yield();
  }

  scoop_thread_unregister();
  return 0;
}

// Test-only export（T1505b）：触发一次 stop-the-world，并验证 Parked 线程在 park 期间
// `stack_walking_ctx` 非空、STW 结束后 ctx 被清空。
//
// 返回：
// - 1：通过
// - <0：失败（用于测试诊断）
intptr_t scoop_test_gc_stack_walking_ctx_smoke(void) {
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  void scoop_thread_unregister(void);

  scoop_runtime_init();
  scoop_thread_register();

  uint32_t stop = 0;
  uint64_t poll_count = 0;
  ScoopTestGcCtxWorkerArgs args = {
      .stop = &stop,
      .poll_count = &poll_count,
  };

  pthread_t worker = 0;
  if (pthread_create(&worker, 0, scoop_test_gc_ctx_worker_entry, (void *)&args) != 0) {
    scoop_thread_unregister();
    return -10;
  }

  // 等待 worker 进入 poll 循环，避免在其尚未注册/调度前触发 STW 导致挂死或结果不稳定。
  struct timespec start;
#if defined(CLOCK_MONOTONIC)
  (void)clock_gettime(CLOCK_MONOTONIC, &start);
#else
  (void)timespec_get(&start, TIME_UTC);
#endif

  while (__atomic_load_n(&poll_count, __ATOMIC_SEQ_CST) < 128) {
    struct timespec now;
#if defined(CLOCK_MONOTONIC)
    (void)clock_gettime(CLOCK_MONOTONIC, &now);
#else
    (void)timespec_get(&now, TIME_UTC);
#endif
    int64_t elapsed_ns = ((int64_t)(now.tv_sec - start.tv_sec) * 1000000000ll) +
                         ((int64_t)now.tv_nsec - (int64_t)start.tv_nsec);
    if (elapsed_ns < 0) {
      elapsed_ns = 0;
    }
    uint64_t elapsed_ms = (uint64_t)(elapsed_ns / 1000000ll);
    if (elapsed_ms > 2000) {
      __atomic_store_n(&stop, 1, __ATOMIC_SEQ_CST);
      (void)pthread_join(worker, 0);
      scoop_thread_unregister();
      return -11;
    }
    sched_yield();
  }

  intptr_t rc = 1;

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    rc = -12;
    goto done_unlock;
  }

  pthread_t self = pthread_self();
  scoop_gc_immix_lock(state);
  scoop_gc_stop_the_world_begin_unlocked(self);

  ScoopGcThreadRecord *worker_rec = scoop_gc_find_thread_unlocked(worker);
  if (worker_rec == 0) {
    rc = -20;
    goto done;
  }
  if (worker_rec->state != SCOOP_GC_THREAD_PARKED) {
    rc = -21;
    goto done;
  }
  if (worker_rec->stack_walking_ctx == 0) {
    rc = -22;
    goto done;
  }

  scoop_gc_stop_the_world_end_unlocked();

  // STW end 会在持锁状态下清空所有线程的 ctx；这里回归验证该行为对 worker 生效。
  if (worker_rec->stack_walking_ctx != 0) {
    rc = -23;
    goto done;
  }

done:
  // 若出现早退，确保 STW 不会悬挂。
  if (scoop_gc_stw_requested_load(&scoop_gc_stw)) {
    scoop_gc_stop_the_world_end_unlocked();
  }
  scoop_gc_immix_unlock(state);

done_unlock:
  __atomic_store_n(&stop, 1, __ATOMIC_SEQ_CST);
  (void)pthread_join(worker, 0);
  scoop_thread_unregister();
  return rc;
}

typedef struct ScoopTestGcUnwindFramesState {
  uint32_t frame_count;
  uint32_t query_count;
  uint32_t sp_non_decreasing;
  uintptr_t last_sp;
} ScoopTestGcUnwindFramesState;

static uint32_t scoop_test_gc_unwind_frame_visitor(uintptr_t sp,
                                                   uintptr_t ra,
                                                   uintptr_t fp,
                                                   void *user_data) {
  (void)ra;
  (void)fp;
  if (user_data == 0) {
    return 0;
  }

  ScoopTestGcUnwindFramesState *state = (ScoopTestGcUnwindFramesState *)user_data;
  // mock stackmap query：只记录被查询的次数与顺序约束（用于回归 walk 的调用行为）。
  state->query_count += 1;

  if (state->frame_count > 0 && sp < state->last_sp) {
    state->sp_non_decreasing = 0;
  }

  state->last_sp = sp;
  state->frame_count += 1;
  return 1;
}

// Test-only export（T1411b）：触发一次 stop-the-world，并验证 Parked 线程的 stack walking
// 能从捕获的 ctx 中枚举至少 3 帧，并把每帧 `(sp, ra)` 输入到 mock stackmap 查询。
//
// 返回：
// - 1：通过
// - <0：失败（用于测试诊断）
intptr_t scoop_test_gc_stack_walking_unwind_smoke(void) {
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  void scoop_thread_unregister(void);

  scoop_runtime_init();
  scoop_thread_register();

  uint32_t stop = 0;
  uint64_t poll_count = 0;
  ScoopTestGcCtxWorkerArgs args = {
      .stop = &stop,
      .poll_count = &poll_count,
  };

  pthread_t worker = 0;
  if (pthread_create(&worker, 0, scoop_test_gc_ctx_worker_entry, (void *)&args) != 0) {
    scoop_thread_unregister();
    return -10;
  }

  // 等待 worker 进入 poll 循环，避免在其尚未注册/调度前触发 STW 导致挂死或结果不稳定。
  struct timespec start;
#if defined(CLOCK_MONOTONIC)
  (void)clock_gettime(CLOCK_MONOTONIC, &start);
#else
  (void)timespec_get(&start, TIME_UTC);
#endif

  while (__atomic_load_n(&poll_count, __ATOMIC_SEQ_CST) < 128) {
    struct timespec now;
#if defined(CLOCK_MONOTONIC)
    (void)clock_gettime(CLOCK_MONOTONIC, &now);
#else
    (void)timespec_get(&now, TIME_UTC);
#endif
    int64_t elapsed_ns = ((int64_t)(now.tv_sec - start.tv_sec) * 1000000000ll) +
                         ((int64_t)now.tv_nsec - (int64_t)start.tv_nsec);
    if (elapsed_ns < 0) {
      elapsed_ns = 0;
    }
    uint64_t elapsed_ms = (uint64_t)(elapsed_ns / 1000000ll);
    if (elapsed_ms > 2000) {
      __atomic_store_n(&stop, 1, __ATOMIC_SEQ_CST);
      (void)pthread_join(worker, 0);
      scoop_thread_unregister();
      return -11;
    }
    sched_yield();
  }

  intptr_t rc = 1;

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    rc = -12;
    goto done_unlock;
  }

  pthread_t self = pthread_self();
  scoop_gc_immix_lock(state);
  scoop_gc_stop_the_world_begin_unlocked(self);

  ScoopGcThreadRecord *worker_rec = scoop_gc_find_thread_unlocked(worker);
  if (worker_rec == 0) {
    rc = -20;
    goto done;
  }
  if (worker_rec->state != SCOOP_GC_THREAD_PARKED) {
    rc = -21;
    goto done;
  }
  if (worker_rec->stack_walking_ctx == 0) {
    rc = -22;
    goto done;
  }

  ScoopTestGcUnwindFramesState unwind = {
      .frame_count = 0,
      .query_count = 0,
      .sp_non_decreasing = 1,
      .last_sp = 0,
  };
  const uint32_t skip_frames = 0;
  const uint32_t visited = scoop_platform_unwind_ctx_walk_frames(
      worker_rec->stack_walking_ctx, skip_frames, scoop_test_gc_unwind_frame_visitor, (void *)&unwind);

  if (visited < 3 || unwind.frame_count < 3 || unwind.query_count < 3) {
    rc = -30;
    goto done;
  }
  if (!unwind.sp_non_decreasing) {
    rc = -31;
    goto done;
  }
  if (visited != unwind.frame_count || visited != unwind.query_count) {
    rc = -32;
    goto done;
  }

  scoop_gc_stop_the_world_end_unlocked();

  // STW end 会在持锁状态下清空所有线程的 ctx；这里防御式回归一下。
  if (worker_rec->stack_walking_ctx != 0) {
    rc = -33;
    goto done;
  }

done:
  // 若出现早退，确保 STW 不会悬挂。
  if (scoop_gc_stw_requested_load(&scoop_gc_stw)) {
    scoop_gc_stop_the_world_end_unlocked();
  }
  scoop_gc_immix_unlock(state);

done_unlock:
  __atomic_store_n(&stop, 1, __ATOMIC_SEQ_CST);
  (void)pthread_join(worker, 0);
  scoop_thread_unregister();
  return rc;
}

typedef struct ScoopTestGcStackmapRootsShared {
  uint32_t stop;
  uint64_t poll_count;

  // worker 函数内某个 stack slot（`void* root`）的地址：用于构造 stackmap location offset。
  void **root_slot;

  // worker 函数内 `scoop_gc_safepoint_poll()` 调用点的“返回地址近似”（label addr）。
  uintptr_t poll_return_address;
} ScoopTestGcStackmapRootsShared;

static void scoop_test_gc_stackmap_roots_count_visitor(void **slot, void *ctx) {
  if (slot == 0 || ctx == 0) {
    return;
  }

  uint64_t *out = (uint64_t *)ctx;
  (void)slot;
  *out += 1;
}

static uint16_t scoop_test_gc_stackmap_dwarf_reg_sp(void) {
#if defined(__x86_64__)
  return 7; // DWARF reg for RSP
#elif defined(__aarch64__)
  return 31; // DWARF reg for SP
#else
  return 0;
#endif
}

static void scoop_test_gc_stackmap_write_u16_le(uint8_t *out, size_t off, uint16_t v) {
  out[off + 0] = (uint8_t)(v & 0xffu);
  out[off + 1] = (uint8_t)((v >> 8) & 0xffu);
}

static void scoop_test_gc_stackmap_write_u32_le(uint8_t *out, size_t off, uint32_t v) {
  out[off + 0] = (uint8_t)(v & 0xffu);
  out[off + 1] = (uint8_t)((v >> 8) & 0xffu);
  out[off + 2] = (uint8_t)((v >> 16) & 0xffu);
  out[off + 3] = (uint8_t)((v >> 24) & 0xffu);
}

static void scoop_test_gc_stackmap_write_u64_le(uint8_t *out, size_t off, uint64_t v) {
  out[off + 0] = (uint8_t)(v & 0xffu);
  out[off + 1] = (uint8_t)((v >> 8) & 0xffu);
  out[off + 2] = (uint8_t)((v >> 16) & 0xffu);
  out[off + 3] = (uint8_t)((v >> 24) & 0xffu);
  out[off + 4] = (uint8_t)((v >> 32) & 0xffu);
  out[off + 5] = (uint8_t)((v >> 40) & 0xffu);
  out[off + 6] = (uint8_t)((v >> 48) & 0xffu);
  out[off + 7] = (uint8_t)((v >> 56) & 0xffu);
}

static void scoop_test_gc_stackmap_write_i32_le(uint8_t *out, size_t off, int32_t v) {
  scoop_test_gc_stackmap_write_u32_le(out, off, (uint32_t)v);
}

typedef struct ScoopTestGcStackmapFindFrame {
  uintptr_t want_ra;
  uintptr_t slot_addr;

  uint32_t found;
  uintptr_t found_sp;
  uintptr_t found_ra;
} ScoopTestGcStackmapFindFrame;

static uint32_t scoop_test_gc_stackmap_find_frame_visitor(uintptr_t sp,
                                                          uintptr_t ra,
                                                          uintptr_t fp,
                                                          void *ctx) {
  (void)fp;
  if (ctx == 0) {
    return 0;
  }

  ScoopTestGcStackmapFindFrame *s = (ScoopTestGcStackmapFindFrame *)ctx;
  if (s->found) {
    return 0;
  }

  // 既用“返回地址近似”做过滤，也用 slot offset 的合理区间做二次约束，避免误匹配。
  intptr_t ra_delta = (intptr_t)ra - (intptr_t)s->want_ra;
  if (ra_delta < 0) {
    ra_delta = -ra_delta;
  }
  if ((uintptr_t)ra_delta > 256u) {
    return 1;
  }

  intptr_t off = (intptr_t)s->slot_addr - (intptr_t)sp;
  if (off < (intptr_t)INT32_MIN || off > (intptr_t)INT32_MAX) {
    return 1;
  }
  if (off > 65536 || off < -65536) {
    return 1;
  }

  s->found = 1;
  s->found_sp = sp;
  s->found_ra = ra;
  return 0;
}

static void *scoop_test_gc_stackmap_roots_worker_entry(void *raw) {
  void scoop_thread_register(void);
  void scoop_thread_unregister(void);

  ScoopTestGcStackmapRootsShared *shared = (ScoopTestGcStackmapRootsShared *)raw;
  if (shared == 0) {
    return 0;
  }

  scoop_thread_register();

  static int dummy = 0;
  // 强制把 root 放在 stack 上（取地址并发布给 main 线程）。
  void *root = (void *)&dummy;
  shared->root_slot = (void **)&root;

  // 用 label address 近似“call safepoint_poll 的 return address”，供 main 线程定位对应帧。
  // NOTE：这依赖 GNU C 扩展；非 clang/gcc 平台下该 smoke 将返回 0（见主函数 gating）。
  shared->poll_return_address = (uintptr_t)&&after_poll;

  while (!__atomic_load_n(&shared->stop, __ATOMIC_SEQ_CST)) {
    scoop_gc_safepoint_poll();
  after_poll:
    (void)__atomic_fetch_add(&shared->poll_count, 1, __ATOMIC_SEQ_CST);
    sched_yield();
  }

  // 防御：确保 root 未被编译器优化掉。
  if (root == 0) {
    (void)fprintf(stderr, "[scooprt][test] unexpected null root\n");
  }

  scoop_thread_unregister();
  return 0;
}

// Test-only export（T1506b）：触发一次 stop-the-world，并验证：
// - Parked 线程 ctx 可用于逐帧 unwind；
// - stackmap lookup 至少命中 1 条 record；
// - locations→slot 解析能枚举到至少 1 个 non-null roots slot。
//
// 返回：
// - 1：通过
// - 0：当前平台/编译器不支持（例如非 clang/gcc）
// - <0：失败（用于测试诊断）
intptr_t scoop_test_gc_stackmap_roots_enum_smoke(void) {
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  void scoop_thread_unregister(void);

#if !defined(__clang__) && !defined(__GNUC__)
  return 0;
#endif

  scoop_runtime_init();
  scoop_thread_register();

  ScoopTestGcStackmapRootsShared shared = {
      .stop = 0,
      .poll_count = 0,
      .root_slot = 0,
      .poll_return_address = 0,
  };

  pthread_t worker = 0;
  if (pthread_create(&worker, 0, scoop_test_gc_stackmap_roots_worker_entry, (void *)&shared) != 0) {
    scoop_thread_unregister();
    return -10;
  }

  // 等待 worker 进入 poll 循环并发布必要信息。
  struct timespec start;
#if defined(CLOCK_MONOTONIC)
  (void)clock_gettime(CLOCK_MONOTONIC, &start);
#else
  (void)timespec_get(&start, TIME_UTC);
#endif

  while (__atomic_load_n(&shared.poll_count, __ATOMIC_SEQ_CST) < 128 ||
         __atomic_load_n(&shared.root_slot, __ATOMIC_SEQ_CST) == 0 ||
         __atomic_load_n(&shared.poll_return_address, __ATOMIC_SEQ_CST) == 0) {
    struct timespec now;
#if defined(CLOCK_MONOTONIC)
    (void)clock_gettime(CLOCK_MONOTONIC, &now);
#else
    (void)timespec_get(&now, TIME_UTC);
#endif
    int64_t elapsed_ns = ((int64_t)(now.tv_sec - start.tv_sec) * 1000000000ll) +
                         ((int64_t)now.tv_nsec - (int64_t)start.tv_nsec);
    if (elapsed_ns < 0) {
      elapsed_ns = 0;
    }
    uint64_t elapsed_ms = (uint64_t)(elapsed_ns / 1000000ll);
    if (elapsed_ms > 2000) {
      __atomic_store_n(&shared.stop, 1, __ATOMIC_SEQ_CST);
      (void)pthread_join(worker, 0);
      scoop_thread_unregister();
      return -11;
    }
    sched_yield();
  }

  intptr_t rc = 1;
  uint8_t *section = 0;

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    rc = -12;
    goto done_unlock;
  }

  pthread_t self = pthread_self();
  scoop_gc_immix_lock(state);
  scoop_gc_stop_the_world_begin_unlocked(self);

  ScoopGcThreadRecord *worker_rec = scoop_gc_find_thread_unlocked(worker);
  if (worker_rec == 0) {
    rc = -20;
    goto done;
  }
  if (worker_rec->state != SCOOP_GC_THREAD_PARKED) {
    rc = -21;
    goto done;
  }
  if (worker_rec->stack_walking_ctx == 0) {
    rc = -22;
    goto done;
  }

  const uintptr_t want_ra = (uintptr_t)shared.poll_return_address;
  const uintptr_t slot_addr = (uintptr_t)shared.root_slot;

  ScoopTestGcStackmapFindFrame find = {
      .want_ra = want_ra,
      .slot_addr = slot_addr,
      .found = 0,
      .found_sp = 0,
      .found_ra = 0,
  };
  (void)scoop_platform_unwind_ctx_walk_frames(worker_rec->stack_walking_ctx,
                                              /*skip_frames=*/0,
                                              scoop_test_gc_stackmap_find_frame_visitor,
                                              (void *)&find);
  if (!find.found || find.found_sp == 0 || find.found_ra == 0) {
    rc = -23;
    goto done;
  }

  const intptr_t slot_off = (intptr_t)slot_addr - (intptr_t)find.found_sp;
  if (slot_off < (intptr_t)INT32_MIN || slot_off > (intptr_t)INT32_MAX) {
    rc = -24;
    goto done;
  }

  const uint16_t sp_reg = scoop_test_gc_stackmap_dwarf_reg_sp();
  if (sp_reg == 0) {
    rc = -25;
    goto done;
  }

  // 构造一个最小可解析的 stackmap section：
  // - 1 function, 1 record；
  // - record 含 2 个 Direct locations（模拟 statepoint base/derived 成对 roots），指向 worker stack slot（root）。
  const size_t record_size = 48;
  const size_t section_size = 16 + 24 + record_size;

  section = (uint8_t *)malloc(section_size);
  if (section == 0) {
    rc = -26;
    goto done;
  }
  memset(section, 0, section_size);

  // header
  section[0] = 3; // version
  section[1] = 0;
  scoop_test_gc_stackmap_write_u16_le(section, 2, 0);
  scoop_test_gc_stackmap_write_u32_le(section, 4, 1); // num_functions
  scoop_test_gc_stackmap_write_u32_le(section, 8, 0); // num_constants
  scoop_test_gc_stackmap_write_u32_le(section, 12, 1); // num_records

  // function record
  const size_t func_off = 16;
  scoop_test_gc_stackmap_write_u64_le(section, func_off + 0, (uint64_t)find.found_ra);
  scoop_test_gc_stackmap_write_u64_le(section, func_off + 8, 0); // stack size (unused)
  scoop_test_gc_stackmap_write_u64_le(section, func_off + 16, 1); // record_count

  // record (v3)
  const size_t rec_off = func_off + 24;
  scoop_test_gc_stackmap_write_u64_le(section, rec_off + 0, 1); // patchpoint_id
  scoop_test_gc_stackmap_write_u32_le(section, rec_off + 8, 0); // instruction_offset
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 12, 0); // reserved
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 14, 2); // num_locations

  // Location 0（12 bytes）：Direct roots slot
  // StackMap v3 location encoding: 2 = Direct (see `runtime/c/scoop_stackmap.c`).
  section[rec_off + 16] = 2u;
  section[rec_off + 17] = 0;
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 18, (uint16_t)sizeof(void *));
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 20, sp_reg);
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 22, 0);
  scoop_test_gc_stackmap_write_i32_le(section, rec_off + 24, (int32_t)slot_off);

  // Location 1（12 bytes）：Direct roots slot（重复一次以满足 base/derived 成对语义）
  section[rec_off + 28] = 2u;
  section[rec_off + 29] = 0;
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 30, (uint16_t)sizeof(void *));
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 32, sp_reg);
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 34, 0);
  scoop_test_gc_stackmap_write_i32_le(section, rec_off + 36, (int32_t)slot_off);

  // num_live_outs + reserved（2 locations 后已是 8-byte 对齐）
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 40, 0);
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 42, 0);

  // 注册 synthetic stackmap section（只用于本 smoke；结束后恢复 registry）。
  scoop_stackmap_registry_reset();
  const uint32_t added =
      scoop_stackmap_registry_register_section((const uint8_t *)section, section_size);
  if (added == 0) {
    rc = -27;
    goto done;
  }

  uint64_t slot_visits = 0;
  uint32_t visit_err = SCOOP_STACKMAP_VISIT_OK;
  uint32_t records_hit = 0;
  ScoopGcManagedRootMap root_map =
      scoop_gc_managed_root_map_from_stackmap_ctx(worker_rec->stack_walking_ctx);
  ScoopGcRootMapVisitResult root_map_result = {0};
  (void)scoop_gc_root_map_visit_slots(
      &root_map, scoop_test_gc_stackmap_roots_count_visitor, (void *)&slot_visits, &root_map_result);
  visit_err = root_map_result.visit_error;
  records_hit = root_map_result.units_hit;
  if (visit_err != SCOOP_STACKMAP_VISIT_OK) {
    rc = -28;
    goto done;
  }
  if (records_hit == 0 || slot_visits == 0) {
    rc = -29;
    goto done;
  }

  scoop_gc_stop_the_world_end_unlocked();

done:
  // 若出现早退，确保 STW 不会悬挂。
  if (scoop_gc_stw_requested_load(&scoop_gc_stw)) {
    scoop_gc_stop_the_world_end_unlocked();
  }
  scoop_gc_immix_unlock(state);

  // 恢复 stackmap registry（避免影响同进程内其它测试）。
  scoop_stackmap_registry_reset();
  (void)scoop_stackmap_registry_register_current_process();

  if (section != 0) {
    free(section);
  }

done_unlock:
  __atomic_store_n(&shared.stop, 1, __ATOMIC_SEQ_CST);
  (void)pthread_join(worker, 0);
  scoop_thread_unregister();
  return rc;
}

typedef struct ScoopTestGcStackmapMultiframeShared {
  uint32_t stop;
  uint64_t poll_count;

  // outer frame 中 stack slot（`void* obj`）的地址：用于构造 stackmap location offset。
  void **root_slot;

  // inner frame 内 `scoop_gc_safepoint_poll()` 调用点的“返回地址近似”（label addr）。
  uintptr_t inner_poll_return_address;

  // outer frame 内对 inner 函数调用点的“返回地址近似”（label addr）。
  uintptr_t outer_call_return_address;

  // worker 在退出前写入的校验结果（0=ok；<0=失败）。
  intptr_t worker_rc;
} ScoopTestGcStackmapMultiframeShared;

static uint64_t scoop_test_gc_stackmap_multiframe_release_calls = 0;

static void scoop_test_gc_stackmap_multiframe_release(void *object) {
  (void)object;
  (void)__atomic_fetch_add(&scoop_test_gc_stackmap_multiframe_release_calls, 1, __ATOMIC_SEQ_CST);
}

static const ScoopTypeDescriptor scoop_test_gc_stackmap_multiframe_desc = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = (uint64_t)sizeof(ScoopGcObjectHeader) + 8u,
    .align_bytes = (uint64_t)sizeof(void *),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = 0,
    .release_fn = scoop_test_gc_stackmap_multiframe_release,
    .type_id = 0,
    .parent_type_desc = 0,
    .itable = 0,
    .vtable = 0,
};

typedef struct ScoopTestGcStackmapFindFrameByRa {
  uintptr_t want_ra;
  uint32_t found;
  uintptr_t found_sp;
  uintptr_t found_ra;
} ScoopTestGcStackmapFindFrameByRa;

static uint32_t scoop_test_gc_stackmap_find_frame_by_ra_visitor(uintptr_t sp,
                                                                uintptr_t ra,
                                                                uintptr_t fp,
                                                                void *ctx) {
  (void)fp;
  if (ctx == 0) {
    return 0;
  }

  ScoopTestGcStackmapFindFrameByRa *s = (ScoopTestGcStackmapFindFrameByRa *)ctx;
  if (s->found) {
    return 0;
  }

  intptr_t ra_delta = (intptr_t)ra - (intptr_t)s->want_ra;
  if (ra_delta < 0) {
    ra_delta = -ra_delta;
  }
  if ((uintptr_t)ra_delta > 256u) {
    return 1;
  }

  s->found = 1;
  s->found_sp = sp;
  s->found_ra = ra;
  return 0;
}

static void scoop_test_gc_stackmap_multiframe_inner(ScoopTestGcStackmapMultiframeShared *shared) {
  if (shared == 0) {
    return;
  }

  // 用 label address 近似“call safepoint_poll 的 return address”，供 main 线程定位 inner frame。
  shared->inner_poll_return_address = (uintptr_t)&&after_poll;

  while (!__atomic_load_n(&shared->stop, __ATOMIC_SEQ_CST)) {
    scoop_gc_safepoint_poll();
  after_poll:
    (void)__atomic_fetch_add(&shared->poll_count, 1, __ATOMIC_SEQ_CST);
    sched_yield();
  }
}

static void scoop_test_gc_stackmap_multiframe_outer(ScoopTestGcStackmapMultiframeShared *shared) {
  if (shared == 0) {
    return;
  }

  // `scoop_alloc` 由 `scoop_runtime.c` 实现；这里仅声明供本测试调用。
  void *scoop_alloc(uint64_t size);

  // 1) 分配一个带 release callback 的对象；该对象仅由 outer frame 的 stack slot root 保活。
  const uint64_t obj_size = scoop_test_gc_stackmap_multiframe_desc.size_bytes;
  void *obj = scoop_alloc(obj_size);
  if (obj == 0) {
    __atomic_store_n(&shared->worker_rc, -100, __ATOMIC_SEQ_CST);
    return;
  }

  ScoopGcObjectHeader *hdr = (ScoopGcObjectHeader *)obj;
  hdr->type_desc = &scoop_test_gc_stackmap_multiframe_desc;

  // payload: 8 bytes magic，供 GC 后验证对象仍可访问。
  const uint64_t magic = 0x6d756c7469667261ull; // "multifra" (ASCII)
  *(uint64_t *)((uint8_t *)obj + sizeof(ScoopGcObjectHeader)) = magic;

  // 强制把 root 放在 stack 上（取地址并发布给 main 线程），避免被优化掉。
  shared->root_slot = (void **)&obj;

  // 用 label address 近似 “call inner 的 return address”，供 main 线程定位 outer frame。
  // 注意：当线程停在 inner 的 safepoint poll 时，outer frame 的 return address 仍为该 label。
  shared->outer_call_return_address = (uintptr_t)&&after_call_inner;

  // 2) 进入 inner：在 inner frame 中反复 poll 直到 main 线程触发 GC 并设置 stop。
  scoop_test_gc_stackmap_multiframe_inner(shared);

after_call_inner:
  // 3) inner 退出后（意味着 main 已触发并完成 GC），验证对象仍可访问（moving GC 需读更新后的 slot）。
  if (__atomic_load_n(&scoop_test_gc_stackmap_multiframe_release_calls, __ATOMIC_SEQ_CST) != 0) {
    __atomic_store_n(&shared->worker_rc, -103, __ATOMIC_SEQ_CST);
    obj = 0;
    return;
  }

  void *obj_after_gc = 0;
  if (shared->root_slot != 0) {
    obj_after_gc = *shared->root_slot;
  }
  if (obj_after_gc == 0) {
    __atomic_store_n(&shared->worker_rc, -101, __ATOMIC_SEQ_CST);
  } else {
    const uint64_t got = *(const uint64_t *)((const uint8_t *)obj_after_gc + sizeof(ScoopGcObjectHeader));
    if (got != magic) {
      __atomic_store_n(&shared->worker_rc, -102, __ATOMIC_SEQ_CST);
    } else {
      __atomic_store_n(&shared->worker_rc, 0, __ATOMIC_SEQ_CST);
    }
  }

  // 4) 清空 root：让对象在 worker 退出后可被回收（用于后续的 release callback 断言）。
  obj = 0;
}

static void *scoop_test_gc_stackmap_multiframe_worker_entry(void *raw) {
  void scoop_thread_register(void);
  void scoop_thread_unregister(void);

  ScoopTestGcStackmapMultiframeShared *shared = (ScoopTestGcStackmapMultiframeShared *)raw;
  if (shared == 0) {
    return 0;
  }

  scoop_thread_register();
  scoop_test_gc_stackmap_multiframe_outer(shared);
  scoop_thread_unregister();
  return 0;
}

// Test-only export（T1506c）：端到端验证 “多帧 roots + stackmap lookup”。
//
// 返回：
// - 1：通过
// - 0：当前平台/编译器不支持（例如非 clang/gcc）
// - <0：失败（用于测试诊断）
intptr_t scoop_test_gc_stackmap_multiframe_keepalive(void) {
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  void scoop_thread_unregister(void);

  // `scoop_gc_collect` / debug helper 定义在本 backend 内；这里声明以避免隐式声明。
  void scoop_gc_collect(void);
  uint64_t scoop_gc_debug_heap_object_count(void);

#if !defined(__clang__) && !defined(__GNUC__)
  return 0;
#endif

  scoop_runtime_init();
  scoop_thread_register();

  __atomic_store_n(&scoop_test_gc_stackmap_multiframe_release_calls, 0, __ATOMIC_SEQ_CST);

  // 确保起始为干净状态（即便未来 init 引入 runtime 分配，这里也能自洽）。
  scoop_gc_collect();
  if (scoop_gc_debug_heap_object_count() != 0) {
    scoop_thread_unregister();
    return -1;
  }

  ScoopTestGcStackmapMultiframeShared shared = {
      .stop = 0,
      .poll_count = 0,
      .root_slot = 0,
      .inner_poll_return_address = 0,
      .outer_call_return_address = 0,
      .worker_rc = 0,
  };

  pthread_t worker = 0;
  if (pthread_create(&worker, 0, scoop_test_gc_stackmap_multiframe_worker_entry, (void *)&shared) != 0) {
    scoop_thread_unregister();
    return -10;
  }

  // 等待 worker 进入 poll 循环并发布必要信息。
  struct timespec start;
#if defined(CLOCK_MONOTONIC)
  (void)clock_gettime(CLOCK_MONOTONIC, &start);
#else
  (void)timespec_get(&start, TIME_UTC);
#endif

  while (__atomic_load_n(&shared.poll_count, __ATOMIC_SEQ_CST) < 128 ||
         __atomic_load_n(&shared.root_slot, __ATOMIC_SEQ_CST) == 0 ||
         __atomic_load_n(&shared.inner_poll_return_address, __ATOMIC_SEQ_CST) == 0 ||
         __atomic_load_n(&shared.outer_call_return_address, __ATOMIC_SEQ_CST) == 0) {
    struct timespec now;
#if defined(CLOCK_MONOTONIC)
    (void)clock_gettime(CLOCK_MONOTONIC, &now);
#else
    (void)timespec_get(&now, TIME_UTC);
#endif
    int64_t elapsed_ns = ((int64_t)(now.tv_sec - start.tv_sec) * 1000000000ll) +
                         ((int64_t)now.tv_nsec - (int64_t)start.tv_nsec);
    if (elapsed_ns < 0) {
      elapsed_ns = 0;
    }
    uint64_t elapsed_ms = (uint64_t)(elapsed_ns / 1000000ll);
    if (elapsed_ms > 2000) {
      __atomic_store_n(&shared.stop, 1, __ATOMIC_SEQ_CST);
      (void)pthread_join(worker, 0);
      scoop_thread_unregister();
      return -11;
    }
    sched_yield();
  }

  intptr_t rc = 1;
  uint8_t *section = 0;

  // 1) 触发一次 STW：拿到 worker 的 stack_walking_ctx，并构造一个 synthetic stackmap section，
  //    覆盖 inner/outer 两帧的 return address（records_hit >= 2）。
  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    rc = -12;
    goto done_unlock;
  }

  pthread_t self = pthread_self();
  scoop_gc_immix_lock(state);
  scoop_gc_stop_the_world_begin_unlocked(self);

  ScoopGcThreadRecord *worker_rec = scoop_gc_find_thread_unlocked(worker);
  if (worker_rec == 0) {
    rc = -20;
    goto done;
  }
  if (worker_rec->state != SCOOP_GC_THREAD_PARKED) {
    rc = -21;
    goto done;
  }
  if (worker_rec->stack_walking_ctx == 0) {
    rc = -22;
    goto done;
  }

  const uintptr_t slot_addr = (uintptr_t)shared.root_slot;

  ScoopTestGcStackmapFindFrame outer = {
      .want_ra = (uintptr_t)shared.outer_call_return_address,
      .slot_addr = slot_addr,
      .found = 0,
      .found_sp = 0,
      .found_ra = 0,
  };
  (void)scoop_platform_unwind_ctx_walk_frames(worker_rec->stack_walking_ctx,
                                              /*skip_frames=*/0,
                                              scoop_test_gc_stackmap_find_frame_visitor,
                                              (void *)&outer);
  if (!outer.found || outer.found_sp == 0 || outer.found_ra == 0) {
    rc = -23;
    goto done;
  }

  ScoopTestGcStackmapFindFrameByRa inner = {
      .want_ra = (uintptr_t)shared.inner_poll_return_address,
      .found = 0,
      .found_sp = 0,
      .found_ra = 0,
  };
  (void)scoop_platform_unwind_ctx_walk_frames(worker_rec->stack_walking_ctx,
                                              /*skip_frames=*/0,
                                              scoop_test_gc_stackmap_find_frame_by_ra_visitor,
                                              (void *)&inner);
  if (!inner.found || inner.found_ra == 0) {
    rc = -24;
    goto done;
  }

  const intptr_t slot_off = (intptr_t)slot_addr - (intptr_t)outer.found_sp;
  if (slot_off < (intptr_t)INT32_MIN || slot_off > (intptr_t)INT32_MAX) {
    rc = -25;
    goto done;
  }

  const uint16_t sp_reg = scoop_test_gc_stackmap_dwarf_reg_sp();
  if (sp_reg == 0) {
    rc = -26;
    goto done;
  }

  // outer record 使用 2 个 roots locations（Direct）模拟 statepoint base/derived 成对语义。
  const size_t inner_record_size = 24;
  const size_t outer_record_size = 48;
  const size_t section_size = 16 + (24 * 2) + inner_record_size + outer_record_size;

  section = (uint8_t *)malloc(section_size);
  if (section == 0) {
    rc = -27;
    goto done;
  }
  memset(section, 0, section_size);

  // header
  section[0] = 3; // version
  section[1] = 0;
  scoop_test_gc_stackmap_write_u16_le(section, 2, 0);
  scoop_test_gc_stackmap_write_u32_le(section, 4, 2); // num_functions
  scoop_test_gc_stackmap_write_u32_le(section, 8, 0); // num_constants
  scoop_test_gc_stackmap_write_u32_le(section, 12, 2); // num_records

  // function records
  const size_t func0_off = 16;
  scoop_test_gc_stackmap_write_u64_le(section, func0_off + 0, (uint64_t)inner.found_ra);
  scoop_test_gc_stackmap_write_u64_le(section, func0_off + 8, 0); // stack size (unused)
  scoop_test_gc_stackmap_write_u64_le(section, func0_off + 16, 1); // record_count

  const size_t func1_off = func0_off + 24;
  scoop_test_gc_stackmap_write_u64_le(section, func1_off + 0, (uint64_t)outer.found_ra);
  scoop_test_gc_stackmap_write_u64_le(section, func1_off + 8, 0); // stack size (unused)
  scoop_test_gc_stackmap_write_u64_le(section, func1_off + 16, 1); // record_count

  // records
  const size_t rec0_off = func1_off + 24;
  // inner record (v3): num_locations = 0
  scoop_test_gc_stackmap_write_u64_le(section, rec0_off + 0, 1); // patchpoint_id
  scoop_test_gc_stackmap_write_u32_le(section, rec0_off + 8, 0); // instruction_offset
  scoop_test_gc_stackmap_write_u16_le(section, rec0_off + 12, 0); // reserved
  scoop_test_gc_stackmap_write_u16_le(section, rec0_off + 14, 0); // num_locations
  // num_live_outs + reserved（locations 后已是 8-byte 对齐）
  scoop_test_gc_stackmap_write_u16_le(section, rec0_off + 16, 0);
  scoop_test_gc_stackmap_write_u16_le(section, rec0_off + 18, 0);

  const size_t rec1_off = rec0_off + inner_record_size;
  // outer record (v3): num_locations = 2（base/derived pair）
  scoop_test_gc_stackmap_write_u64_le(section, rec1_off + 0, 2); // patchpoint_id
  scoop_test_gc_stackmap_write_u32_le(section, rec1_off + 8, 0); // instruction_offset
  scoop_test_gc_stackmap_write_u16_le(section, rec1_off + 12, 0); // reserved
  scoop_test_gc_stackmap_write_u16_le(section, rec1_off + 14, 2); // num_locations

  // Location 0（12 bytes）：Direct roots slot
  section[rec1_off + 16] = 2u; // Direct
  section[rec1_off + 17] = 0;
  scoop_test_gc_stackmap_write_u16_le(section, rec1_off + 18, (uint16_t)sizeof(void *));
  scoop_test_gc_stackmap_write_u16_le(section, rec1_off + 20, sp_reg);
  scoop_test_gc_stackmap_write_u16_le(section, rec1_off + 22, 0);
  scoop_test_gc_stackmap_write_i32_le(section, rec1_off + 24, (int32_t)slot_off);

  // Location 1（12 bytes）：Direct roots slot（重复一次以满足 base/derived 成对语义）
  section[rec1_off + 28] = 2u; // Direct
  section[rec1_off + 29] = 0;
  scoop_test_gc_stackmap_write_u16_le(section, rec1_off + 30, (uint16_t)sizeof(void *));
  scoop_test_gc_stackmap_write_u16_le(section, rec1_off + 32, sp_reg);
  scoop_test_gc_stackmap_write_u16_le(section, rec1_off + 34, 0);
  scoop_test_gc_stackmap_write_i32_le(section, rec1_off + 36, (int32_t)slot_off);

  // num_live_outs + reserved（2 locations 后已是 8-byte 对齐）
  scoop_test_gc_stackmap_write_u16_le(section, rec1_off + 40, 0);
  scoop_test_gc_stackmap_write_u16_le(section, rec1_off + 42, 0);

  scoop_stackmap_registry_reset();
  (void)scoop_stackmap_registry_register_current_process();
  const uint32_t added =
      scoop_stackmap_registry_register_section((const uint8_t *)section, section_size);
  if (added == 0) {
    rc = -28;
    goto done;
  }

  uint64_t slot_visits = 0;
  uint32_t visit_err = SCOOP_STACKMAP_VISIT_OK;
  uint32_t records_hit = 0;
  ScoopGcManagedRootMap root_map =
      scoop_gc_managed_root_map_from_stackmap_ctx(worker_rec->stack_walking_ctx);
  ScoopGcRootMapVisitResult root_map_result = {0};
  (void)scoop_gc_root_map_visit_slots(
      &root_map, scoop_test_gc_stackmap_roots_count_visitor, (void *)&slot_visits, &root_map_result);
  visit_err = root_map_result.visit_error;
  records_hit = root_map_result.units_hit;
  if (visit_err != SCOOP_STACKMAP_VISIT_OK) {
    rc = -29;
    goto done;
  }
  if (records_hit < 2 || slot_visits == 0) {
    rc = -30;
    goto done;
  }

  scoop_gc_stop_the_world_end_unlocked();

done:
  // 若出现早退，确保 STW 不会悬挂。
  if (scoop_gc_stw_requested_load(&scoop_gc_stw)) {
    scoop_gc_stop_the_world_end_unlocked();
  }
  scoop_gc_immix_unlock(state);

  if (rc != 1) {
    goto cleanup;
  }

  // 2) 触发一次真实 GC：要求对象不被回收（release callback 不应被调用）。
  scoop_gc_collect();
  if (__atomic_load_n(&scoop_test_gc_stackmap_multiframe_release_calls, __ATOMIC_SEQ_CST) != 0) {
    rc = -40;
    goto cleanup;
  }
  if (scoop_gc_debug_heap_object_count() != 1) {
    rc = -41;
    goto cleanup;
  }

  // 3) 让 worker 退出 inner loop，并在 outer frame 校验对象 payload（moving GC 下要求 slot 已被更新）。
  __atomic_store_n(&shared.stop, 1, __ATOMIC_SEQ_CST);
  (void)pthread_join(worker, 0);

  const intptr_t worker_rc = __atomic_load_n(&shared.worker_rc, __ATOMIC_SEQ_CST);
  if (worker_rc != 0) {
    rc = worker_rc;
    goto cleanup;
  }

  // 4) 再次 GC：对象应可被回收（release callback 只被调用一次）。
  scoop_gc_collect();
  if (__atomic_load_n(&scoop_test_gc_stackmap_multiframe_release_calls, __ATOMIC_SEQ_CST) != 1) {
    rc = -50;
    goto cleanup;
  }
  if (scoop_gc_debug_heap_object_count() != 0) {
    rc = -51;
    goto cleanup;
  }

cleanup:
  // 恢复 stackmap registry（避免影响同进程内其它测试）。
  scoop_stackmap_registry_reset();
  (void)scoop_stackmap_registry_register_current_process();

  if (section != 0) {
    free(section);
  }

done_unlock:
  if (rc != 1) {
    __atomic_store_n(&shared.stop, 1, __ATOMIC_SEQ_CST);
    (void)pthread_join(worker, 0);
  }

  scoop_thread_unregister();
  return rc;
}

void scoop_gc_thread_register(ScoopThreadTls *tls) {
  if (tls == 0) {
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
    existing->tls = tls;
    existing->gc_alloc_block_slot =
        scoop_tls_gc_immix_current_block_slot(tls);
    existing->gc_alloc_block_cache_slot =
        scoop_tls_gc_immix_block_cache_slot(tls);
    existing->gc_alloc_block_cache_len_slot =
        scoop_tls_gc_immix_block_cache_len_slot(tls);
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
  rec->tls = tls;
  rec->gc_alloc_block_slot =
      scoop_tls_gc_immix_current_block_slot(tls);
  rec->gc_alloc_block_cache_slot =
      scoop_tls_gc_immix_block_cache_slot(tls);
  rec->gc_alloc_block_cache_len_slot =
      scoop_tls_gc_immix_block_cache_len_slot(tls);
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

void scoop_gc_thread_unregister(ScoopThreadTls *tls) {
  (void)tls;

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

static void scoop_gc_safepoint_common(uint32_t capture_stack_walking_ctx) {
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
      if (capture_stack_walking_ctx) {
        // T1505b：在进入 Parked 前捕获当前线程的 unwind 上下文，用于后续 stack walking。
        // 说明：此处只保存 opaque ctx；逐帧 unwind 在 T1411b 接入 platform/unwind 完成。
        scoop_platform_unwind_ctx_destroy(rec->stack_walking_ctx);
        rec->stack_walking_ctx = scoop_platform_unwind_ctx_capture();
      }

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

void scoop_gc_safepoint(void) { scoop_gc_safepoint_common(/*capture_stack_walking_ctx=*/0); }

void scoop_gc_safepoint_poll(void) {
  // T1505b：把“park 前捕获 stack walking ctx”的新语义落在 poll 上，避免扩大历史 ABI 的语义漂移。
  scoop_gc_safepoint_common(/*capture_stack_walking_ctx=*/1);
}

static void scoop_gc_immix_nursery_remove_free_block_unlocked(ScoopGcImmixState *state,
                                                              ScoopGcImmixBlock *target) {
  if (state == 0 || target == 0) {
    return;
  }

  ScoopGcImmixBlock **link = &state->nursery_free_blocks;
  while (*link != 0) {
    ScoopGcImmixBlock *it = *link;
    if (it != target) {
      link = &it->next_free;
      continue;
    }

    *link = it->next_free;
    it->next_free = 0;
    return;
  }
}

static void scoop_gc_immix_nursery_promote_block_to_old_unlocked(ScoopGcImmixState *state,
                                                                 ScoopGcImmixBlock *block) {
  if (state == 0 || block == 0) {
    return;
  }
  if (block->generation != (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_NURSERY) {
    return;
  }

  // v0：promote-on-store 采用“整块 block 晋升”的策略：
  // - 不搬迁对象（避免引入 read barrier / forwarding pointer 语义）；
  // - minor GC 只需要 reset 仍为 nursery 的 blocks，从而无需 old→nursery remembered set。
  block->generation = (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_OLD;

  if (state->nursery_blocks > 0) {
    state->nursery_blocks -= 1;
  }

  if (state->nursery_current_block == block) {
    // 避免继续在已晋升为 old 的 block 上走 nursery bump 分配路径。
    state->nursery_current_block = 0;
  }

  // 若该 block 已在 nursery free list 中，则移除，避免后续被当作 nursery 重用。
  scoop_gc_immix_nursery_remove_free_block_unlocked(state, block);
}

void *scoop_gc_write_barrier(void *slot_addr, void *value) {
  if (slot_addr == 0) {
    return value;
  }

  // 与 `scoop_alloc` 一致：写屏障也作为 safepoint（poll）边界，避免 GC 请求 STW 时卡住。
  void scoop_thread_register(void);
  void scoop_gc_safepoint_poll(void);
  scoop_thread_register();
  scoop_gc_safepoint_poll();

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0 || !state->lock_inited) {
    (void)memcpy(slot_addr, &value, sizeof(void *));
    return value;
  }

  // fast path：绝大多数 store 不会形成 old→nursery 指针，尽量不抢全局锁。
  ScoopGcImmixBlock *slot_block = scoop_gc_immix_block_from_object(slot_addr);
  ScoopGcImmixBlock *value_block = scoop_gc_immix_block_from_object(value);

  const uint8_t old = (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_OLD;
  const uint8_t nursery = (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_NURSERY;

  const uint32_t needs_promote =
      (slot_block != 0 && value_block != 0 && slot_block->generation == old &&
       value_block->generation == nursery);

  if (!needs_promote) {
    (void)memcpy(slot_addr, &value, sizeof(void *));
    return value;
  }

  scoop_gc_immix_lock(state);

  // re-check under lock：避免并发 store 重复扣减 nursery_blocks 或重复改写 free list。
  slot_block = scoop_gc_immix_block_from_object(slot_addr);
  value_block = scoop_gc_immix_block_from_object(value);
  if (slot_block != 0 && value_block != 0 && slot_block->generation == old &&
      value_block->generation == nursery) {
    scoop_gc_immix_nursery_promote_block_to_old_unlocked(state, value_block);
  }

  (void)memcpy(slot_addr, &value, sizeof(void *));
  scoop_gc_immix_unlock(state);
  return value;
}

// enter_native/leave_native（TODO T1505c）：
// - 线程进入 native/extern 前调用 enter_native，切换线程状态为 InNative，并登记 native roots；
// - 线程从 native 返回后调用 leave_native，清空 roots 并恢复 Running；
// - STW/GC 在等待其它线程就绪时将 InNative 视为“已就绪”（不要求 park）。
//
// v0：native roots 允许由调用方传入 roots slots 指针数组（元素类型 `void**`）。
void scoop_enter_native(void ***root_slots, uint32_t root_slots_len) {
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  pthread_t self = pthread_self();
  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return;
  }

  scoop_gc_immix_lock(state);

  ScoopGcThreadRecord *rec = scoop_gc_find_thread_unlocked(self);
  if (rec == 0) {
    scoop_gc_immix_unlock(state);
    return;
  }

  // T1512c：线程跨入 native 前仍然保留着更高层 managed caller frames；若只登记当前 call-site 的
  // `native_roots`，GC 将看不到 caller 栈上的 live roots（例如 main 持有的 Thread handle）。
  // 因此 enter_native 必须同时捕获当前 stack walking ctx，并在整个 InNative 期间保留它。
  scoop_platform_unwind_ctx_destroy(rec->stack_walking_ctx);
  rec->stack_walking_ctx = scoop_platform_unwind_ctx_capture();
  if (rec->stack_walking_ctx == 0) {
    scoop_gc_immix_unlock(state);
    (void)fprintf(stderr, "[scooprt][gc][stackmap] enter_native failed to capture unwind ctx\n");
    abort();
  }

  // TLS：保存 native roots buffer（供后续 stackmap roots/handle 协议扩展）。
  ScoopThreadTls *tls = rec->tls;
  if (tls != 0) {
    tls->gc_native_roots = (void *)root_slots;
    tls->gc_native_roots_len = root_slots_len;
  }

  rec->native_roots = (void *)root_slots;
  rec->native_roots_len = root_slots_len;
  rec->state = SCOOP_GC_THREAD_IN_NATIVE;
  rec->last_safepoint_epoch = scoop_gc_stw.epoch;

  // 若当前正处于 stop-the-world，则 enter_native 可以直接把自己切到 InNative ready 状态：
  // - 当前 call-site roots 由 `native_roots` 提供；
  // - 更高层 managed caller frames 由 enter_native 时捕获的 ctx 提供；
  // - `parked_count` 仍需补记一次，告诉 GC “这个线程已就绪，不必再等它 park”。
  if (scoop_gc_stw_requested_load(&scoop_gc_stw) && !pthread_equal(self, scoop_gc_stw.initiator)) {
    if (rec->parked_epoch != scoop_gc_stw.epoch) {
      rec->parked_epoch = scoop_gc_stw.epoch;
      scoop_gc_stw.parked_count += 1;
      (void)pthread_cond_broadcast(&scoop_gc_stw_cond);
    }
  } else {
    rec->parked_epoch = 0;
  }

  scoop_gc_immix_unlock(state);
}

void scoop_leave_native(void) {
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  pthread_t self = pthread_self();
  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return;
  }

  scoop_gc_immix_lock(state);

  ScoopGcThreadRecord *rec = scoop_gc_find_thread_unlocked(self);
  if (rec == 0) {
    scoop_gc_immix_unlock(state);
    return;
  }

  // 若当前有 stop-the-world 请求：
  // - 若线程仍处于 InNative，则保持 roots 不变并等待 STW 结束（避免 GC 扫描期间 roots 被提前清空）；
  // - 若线程不在 InNative（误用/竞态），则需要参与 STW 的 park 计数，避免 GC 等待死锁。
  while (scoop_gc_stw_requested_load(&scoop_gc_stw) && !pthread_equal(self, scoop_gc_stw.initiator)) {
    rec->last_safepoint_epoch = scoop_gc_stw.epoch;

    if (rec->state != SCOOP_GC_THREAD_IN_NATIVE) {
      if (rec->parked_epoch != scoop_gc_stw.epoch) {
        rec->state = SCOOP_GC_THREAD_PARKED;
        rec->parked_epoch = scoop_gc_stw.epoch;
        scoop_gc_stw.parked_count += 1;
        (void)pthread_cond_broadcast(&scoop_gc_stw_cond);
      }
    }

    (void)pthread_cond_wait(&scoop_gc_stw_cond, &state->lock);
  }

  ScoopThreadTls *tls = rec->tls;
  if (tls != 0) {
    tls->gc_native_roots = 0;
    tls->gc_native_roots_len = 0;
  }

  rec->native_roots = 0;
  rec->native_roots_len = 0;
  rec->state = SCOOP_GC_THREAD_RUNNING;
  rec->parked_epoch = 0;
  scoop_platform_unwind_ctx_destroy(rec->stack_walking_ctx);
  rec->stack_walking_ctx = 0;

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

uint64_t scoop_handle_new(void *raw_obj) {
  if (raw_obj == 0) {
    return 0;
  }

  // 说明：对齐 baseline/minimal backend：允许在未显式 init/register 的情况下被调用。
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

  ScoopGcHandleRecord *rec = (ScoopGcHandleRecord *)malloc(sizeof(ScoopGcHandleRecord));
  if (rec == 0) {
    scoop_gc_immix_unlock(state);
    return 0;
  }

  rec->next = scoop_gc_handle_records;
  rec->object = obj;
  scoop_gc_handle_records = rec;

  uint64_t handle = (uint64_t)(uintptr_t)rec;
  scoop_gc_immix_unlock(state);
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

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return 0;
  }

  ScoopGcHandleRecord *needle = (ScoopGcHandleRecord *)(uintptr_t)handle;

  scoop_gc_immix_lock(state);
  for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
    if (it != needle) {
      continue;
    }
    void *obj = (void *)it->object;
    scoop_gc_immix_unlock(state);
    return obj;
  }
  scoop_gc_immix_unlock(state);
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

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return 0;
  }

  ScoopGcHandleRecord *needle = (ScoopGcHandleRecord *)(uintptr_t)handle;

  scoop_gc_immix_lock(state);

  ScoopGcHandleRecord **link = &scoop_gc_handle_records;
  while (*link != 0) {
    ScoopGcHandleRecord *it = *link;
    if (it != needle) {
      link = &it->next;
      continue;
    }

    *link = it->next;
    free(it);
    scoop_gc_immix_unlock(state);
    return 1;
  }

  scoop_gc_immix_unlock(state);
  return 0;
}

void scoop_gc_register_global_root(void *base, const ScoopTypeDescriptor *type_desc) {
  if (base == 0 || type_desc == 0) {
    return;
  }

  void scoop_runtime_init(void);
  scoop_runtime_init();

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return;
  }

  scoop_gc_immix_lock(state);

  for (ScoopGcGlobalRootRecord *it = scoop_gc_global_roots; it != 0; it = it->next) {
    if (it->base != base) {
      continue;
    }
    it->type_desc = type_desc;
    scoop_gc_immix_unlock(state);
    return;
  }

  ScoopGcGlobalRootRecord *rec = (ScoopGcGlobalRootRecord *)malloc(sizeof(ScoopGcGlobalRootRecord));
  if (rec == 0) {
    scoop_gc_immix_unlock(state);
    return;
  }

  rec->next = scoop_gc_global_roots;
  rec->base = base;
  rec->type_desc = type_desc;
  scoop_gc_global_roots = rec;

  scoop_gc_immix_unlock(state);
}

void scoop_gc_heap_register_object(ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return;
  }

  // T1409a：并发 push（分配路径不持锁）。
  scoop_gc_heap_push_object_atomic(obj);
  scoop_gc_heap_bytes_allocated_add(obj->size_bytes);
}

static uint32_t scoop_gc_immix_env_read_u64(const char *name, uint64_t *out) {
  if (name == 0 || out == 0) {
    return 0;
  }

  const char *raw = getenv(name);
  if (raw == 0) {
    return 0;
  }

  // 跳过前导空白（允许 `NAME=" 123"`）。
  while (*raw == ' ' || *raw == '\t' || *raw == '\n' || *raw == '\r') {
    raw++;
  }
  if (*raw == 0) {
    return 0;
  }

  errno = 0;
  char *end = 0;
  unsigned long long v = strtoull(raw, &end, 10);
  if (end == raw || errno != 0) {
    return 0;
  }

  *out = (uint64_t)v;
  return 1;
}

static uint32_t scoop_gc_immix_nursery_max_blocks_from_env(void) {
  // 配置优先级：
  // 1) `SCOOP_GC_IMMIX_NURSERY_BYTES`（更细粒度）
  // 2) `SCOOP_GC_IMMIX_NURSERY_BLOCKS`
  uint64_t bytes = 0;
  if (scoop_gc_immix_env_read_u64("SCOOP_GC_IMMIX_NURSERY_BYTES", &bytes) && bytes > 0) {
    const uint64_t block_bytes = (uint64_t)SCOOP_GC_IMMIX_BLOCK_SIZE;
    uint64_t blocks = (bytes + block_bytes - 1u) / block_bytes;
    if (blocks > (uint64_t)UINT32_MAX) {
      blocks = (uint64_t)UINT32_MAX;
    }
    return (uint32_t)blocks;
  }

  uint64_t blocks = 0;
  if (scoop_gc_immix_env_read_u64("SCOOP_GC_IMMIX_NURSERY_BLOCKS", &blocks) && blocks > 0) {
    if (blocks > (uint64_t)UINT32_MAX) {
      blocks = (uint64_t)UINT32_MAX;
    }
    return (uint32_t)blocks;
  }

  return 0;
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

    // nursery 配置（T1412b）：在首次 heap init 时读取 env 并固化到 state 中。
    // 说明：runtime 当前只支持进程级别的一次初始化；因此无需处理“运行中改 env”。
    if (state->nursery_max_blocks == 0) {
      state->nursery_max_blocks = scoop_gc_immix_nursery_max_blocks_from_env();
    }

    // 把已分配的 blocks 复位并串到 free list，供分配路径复用。
    state->reusable_blocks = 0;
    state->free_blocks = 0;
    state->current_block = 0;
    state->nursery_free_blocks = 0;
    state->nursery_current_block = 0;
    state->nursery_blocks = 0;
    for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
      scoop_gc_immix_block_reset(it);

      // 重建 nursery blocks 计数与 free list（按 block.generation 分类）。
      if (it->generation == (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_NURSERY) {
        state->nursery_blocks += 1;
        it->next_free = state->nursery_free_blocks;
        state->nursery_free_blocks = it;
      } else {
        it->next_free = state->free_blocks;
        state->free_blocks = it;
      }
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

// --- Roots membership 过滤索引（TODO T1412a） ---
//
// 目的：
// - roots/slot tracing（stackmap records + type trace）里可能存在“pointer-sized 但不是 GC object”的值；
// - 为避免把这些值当作 `ScoopGcObjectHeader*` 解引用导致崩溃，需要做 membership 过滤；
// - 历史实现对每个 slot 线性遍历 `heap.objects`，在 slot 数量放大时会把 STW 时间放大到 O(n*m)。
//
// 方案（v0）：
// - 每轮 GC 初始化一次：把 `heap.objects` 快照为数组，按地址排序；
// - membership 判定走二分查找（O(log n)）；
// - 若内存不足无法构建索引，则退化为线性扫描（slow path；保持正确性）。

static int scoop_gc_ptr_cmp(const void *a, const void *b);

typedef struct ScoopGcHeapMembershipIndex {
  ScoopGcObjectHeader **sorted;
  size_t len;
  uint32_t built;
} ScoopGcHeapMembershipIndex;

static void scoop_gc_heap_membership_index_destroy(ScoopGcHeapMembershipIndex *idx) {
  if (idx == 0) {
    return;
  }
  if (idx->sorted != 0) {
    free(idx->sorted);
  }
  idx->sorted = 0;
  idx->len = 0;
  idx->built = 0;
}

static uint32_t scoop_gc_heap_membership_index_build_unlocked(ScoopGcHeapMembershipIndex *out,
                                                              ScoopGcHeap *heap) {
  if (out == 0 || heap == 0) {
    return 0;
  }

  out->sorted = 0;
  out->len = 0;
  out->built = 0;

  size_t count = 0;
  for (ScoopGcObjectHeader *it = heap->objects; it != 0; it = it->next) {
    count += 1;
  }

  if (count == 0) {
    out->built = 1;
    return 1;
  }

  if (count > (SIZE_MAX / sizeof(ScoopGcObjectHeader *))) {
    return 0;
  }

  ScoopGcObjectHeader **sorted =
      (ScoopGcObjectHeader **)malloc(count * sizeof(ScoopGcObjectHeader *));
  if (sorted == 0) {
    return 0;
  }

  size_t idx = 0;
  for (ScoopGcObjectHeader *it = heap->objects; it != 0 && idx < count; it = it->next) {
    sorted[idx++] = it;
  }
  count = idx;

  qsort(sorted, count, sizeof(ScoopGcObjectHeader *), scoop_gc_ptr_cmp);

  out->sorted = sorted;
  out->len = count;
  out->built = 1;
  return 1;
}

static uint32_t scoop_gc_heap_membership_index_contains(const ScoopGcHeapMembershipIndex *idx,
                                                        ScoopGcHeap *heap,
                                                        ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return 0;
  }

  if (idx != 0 && idx->built) {
    if (idx->sorted == 0 || idx->len == 0) {
      return 0;
    }

    size_t lo = 0;
    size_t hi = idx->len;
    const uintptr_t target = (uintptr_t)obj;

    while (lo < hi) {
      const size_t mid = lo + ((hi - lo) / 2u);
      const uintptr_t cur = (uintptr_t)idx->sorted[mid];
      if (cur == target) {
        return 1;
      }
      if (cur < target) {
        lo = mid + 1u;
      } else {
        hi = mid;
      }
    }

    return 0;
  }

  // fallback：无法构建索引（例如 OOM）时退化为线性扫描；保持正确性。
  if (heap == 0) {
    return 0;
  }
  for (ScoopGcObjectHeader *it = heap->objects; it != 0; it = it->next) {
    if (it == obj) {
      return 1;
    }
  }
  return 0;
}

typedef struct ScoopGcMarkCtx {
  ScoopGcHeap *heap;
  uint32_t mark_value;
  ScoopGcMarkStack *stack;
  const ScoopGcHeapMembershipIndex *membership;
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
    uint64_t raw_size = obj->size_bytes;
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

  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)raw;

  // 重要：stackmap records 里可能包含“刚好是 pointer-sized 但不是 GC roots”的 slot
  // （例如 call target / return address / deopt metadata 等）。
  // 为避免把这些值当作 heap 对象解引用并崩溃，这里做一次 membership 过滤。
  if (!scoop_gc_heap_membership_index_contains(ctx->membership, ctx->heap, obj)) {
    return;
  }

  scoop_gc_mark_object_if_needed(ctx, obj);
}

// --- 并行标记（TODO T1409c1；实验性，可开关） ---
//
// 说明：
// - 并行标记只在 stop-the-world 达成后运行：mutator 已暂停，因此无需写屏障；
// - marker workers 之间需要保证 `obj->mark` 与 line mark bits 的写入是线程安全的；
// - v0 采用全局 work stack（mutex/cond），以 correctness 为先；性能优化留给后续任务。

typedef struct ScoopGcParallelMarkWork {
  pthread_mutex_t lock;
  pthread_cond_t cond;

  ScoopGcObjectHeader **items;
  size_t len;
  size_t cap;

  uint32_t inited;
  uint32_t done;
  uint64_t in_flight;
} ScoopGcParallelMarkWork;

static uint32_t scoop_gc_parallel_mark_work_init(ScoopGcParallelMarkWork *work) {
  if (work == 0) {
    return 0;
  }
  (void)memset(work, 0, sizeof(*work));

  if (pthread_mutex_init(&work->lock, 0) != 0) {
    return 0;
  }
  if (pthread_cond_init(&work->cond, 0) != 0) {
    (void)pthread_mutex_destroy(&work->lock);
    return 0;
  }

  work->inited = 1;
  work->done = 0;
  __atomic_store_n(&work->in_flight, 0, __ATOMIC_RELAXED);
  return 1;
}

static void scoop_gc_parallel_mark_work_destroy(ScoopGcParallelMarkWork *work) {
  if (work == 0) {
    return;
  }

  if (work->items != 0) {
    free(work->items);
  }
  work->items = 0;
  work->len = 0;
  work->cap = 0;

  if (work->inited) {
    (void)pthread_cond_destroy(&work->cond);
    (void)pthread_mutex_destroy(&work->lock);
  }

  (void)memset(work, 0, sizeof(*work));
}

static uint32_t scoop_gc_parallel_mark_work_push(ScoopGcParallelMarkWork *work,
                                                 ScoopGcObjectHeader *obj) {
  if (work == 0 || obj == 0 || !work->inited) {
    return 0;
  }

  // 先增计数：避免在 push 过程中被误判为 “in_flight==0 => done”。
  (void)__atomic_fetch_add(&work->in_flight, 1, __ATOMIC_RELAXED);

  (void)pthread_mutex_lock(&work->lock);

  if (work->done) {
    (void)pthread_mutex_unlock(&work->lock);
    (void)__atomic_fetch_sub(&work->in_flight, 1, __ATOMIC_RELAXED);
    return 0;
  }

  if (work->len == work->cap) {
    size_t new_cap = (work->cap == 0) ? 1024u : work->cap * 2u;
    if (new_cap < work->cap) {
      (void)pthread_mutex_unlock(&work->lock);
      (void)__atomic_fetch_sub(&work->in_flight, 1, __ATOMIC_RELAXED);
      return 0;
    }
    if (new_cap > (SIZE_MAX / sizeof(ScoopGcObjectHeader *))) {
      (void)pthread_mutex_unlock(&work->lock);
      (void)__atomic_fetch_sub(&work->in_flight, 1, __ATOMIC_RELAXED);
      return 0;
    }

    void *p = realloc(work->items, new_cap * sizeof(ScoopGcObjectHeader *));
    if (p == 0) {
      (void)pthread_mutex_unlock(&work->lock);
      (void)__atomic_fetch_sub(&work->in_flight, 1, __ATOMIC_RELAXED);
      return 0;
    }
    work->items = (ScoopGcObjectHeader **)p;
    work->cap = new_cap;
  }

  work->items[work->len++] = obj;
  (void)pthread_cond_signal(&work->cond);
  (void)pthread_mutex_unlock(&work->lock);
  return 1;
}

static ScoopGcObjectHeader *scoop_gc_parallel_mark_work_pop(ScoopGcParallelMarkWork *work) {
  if (work == 0 || !work->inited) {
    return 0;
  }

  (void)pthread_mutex_lock(&work->lock);
  while (work->len == 0 && !work->done) {
    (void)pthread_cond_wait(&work->cond, &work->lock);
  }

  if (work->len == 0) {
    (void)pthread_mutex_unlock(&work->lock);
    return 0;
  }

  work->len -= 1;
  ScoopGcObjectHeader *obj = work->items[work->len];
  (void)pthread_mutex_unlock(&work->lock);
  return obj;
}

static void scoop_gc_parallel_mark_work_finish_one(ScoopGcParallelMarkWork *work) {
  if (work == 0 || !work->inited) {
    return;
  }

  uint64_t prev = __atomic_fetch_sub(&work->in_flight, 1, __ATOMIC_ACQ_REL);
  if (prev != 1) {
    return;
  }

  // in_flight -> 0：通知所有等待 worker 退出。
  (void)pthread_mutex_lock(&work->lock);
  work->done = 1;
  (void)pthread_cond_broadcast(&work->cond);
  (void)pthread_mutex_unlock(&work->lock);
}

static inline void scoop_gc_immix_block_mark_marked_range_atomic(ScoopGcImmixBlock *block,
                                                                 const uint8_t *start,
                                                                 size_t size) {
  if (block == 0 || start == 0 || size == 0) {
    return;
  }

  uintptr_t base = (uintptr_t)block;
  uintptr_t p0 = (uintptr_t)start;
  uintptr_t p1 = p0 + (uintptr_t)size - 1u;

  if (p0 < base) {
    return;
  }
  if (p1 < p0) {
    return;
  }

  size_t start_line = (size_t)((p0 - base) / (uintptr_t)SCOOP_GC_IMMIX_LINE_SIZE);
  size_t end_line = (size_t)((p1 - base) / (uintptr_t)SCOOP_GC_IMMIX_LINE_SIZE);
  if (start_line >= (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK) {
    return;
  }
  if (end_line >= (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK) {
    end_line = (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK - 1u;
  }

  for (size_t line = start_line; line <= end_line; line++) {
    size_t idx = line / 64u;
    if (idx >= (size_t)SCOOP_GC_IMMIX_BITMAP_WORDS) {
      break;
    }
    uint64_t mask = (uint64_t)1u << (uint64_t)(line % 64u);
    (void)__atomic_fetch_or(&block->line_mark_bits[idx], mask, __ATOMIC_RELAXED);
  }
}

typedef struct ScoopGcParallelMarkCtx {
  ScoopGcHeap *heap;
  uint32_t mark_value;
  ScoopGcParallelMarkWork *work;
  const ScoopGcHeapMembershipIndex *membership;
} ScoopGcParallelMarkCtx;

static void scoop_gc_parallel_mark_object_if_needed(ScoopGcParallelMarkCtx *ctx,
                                                    ScoopGcObjectHeader *obj) {
  if (ctx == 0 || obj == 0) {
    return;
  }

  uint32_t expected = __atomic_load_n(&obj->mark, __ATOMIC_RELAXED);
  while (expected != ctx->mark_value) {
    if (__atomic_compare_exchange_n(&obj->mark,
                                    &expected,
                                    ctx->mark_value,
                                    0,
                                    __ATOMIC_RELAXED,
                                    __ATOMIC_RELAXED)) {
      ScoopGcImmixBlock *block = scoop_gc_immix_block_from_object((void *)obj);
      if (block != 0) {
        uint64_t raw_size = obj->size_bytes;
        size_t size = (raw_size > (uint64_t)SIZE_MAX) ? (size_t)SIZE_MAX : (size_t)raw_size;
        scoop_gc_immix_block_mark_marked_range_atomic(block, (const uint8_t *)obj, size);
      }
      (void)scoop_gc_parallel_mark_work_push(ctx->work, obj);
      return;
    }
    // CAS 失败：expected 已被更新为当前值；继续循环。
  }
}

static void scoop_gc_parallel_mark_visitor(void **slot, void *raw_ctx) {
  if (slot == 0 || raw_ctx == 0) {
    return;
  }

  ScoopGcParallelMarkCtx *ctx = (ScoopGcParallelMarkCtx *)raw_ctx;
  void *raw = *slot;
  if (raw == 0) {
    return;
  }

  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)raw;
  if (!scoop_gc_heap_membership_index_contains(ctx->membership, ctx->heap, obj)) {
    return;
  }
  scoop_gc_parallel_mark_object_if_needed(ctx, obj);
}

static void *scoop_gc_parallel_mark_worker(void *raw_ctx) {
  if (raw_ctx == 0) {
    return 0;
  }

  ScoopGcParallelMarkCtx *ctx = (ScoopGcParallelMarkCtx *)raw_ctx;
  ScoopGcParallelMarkWork *work = ctx->work;

  while (1) {
    ScoopGcObjectHeader *obj = scoop_gc_parallel_mark_work_pop(work);
    if (obj == 0) {
      break;
    }

    if (obj->type_desc != 0) {
      (void)scoop_gc_type_descriptor_trace(obj->type_desc,
                                           (void *)obj,
                                           scoop_gc_parallel_mark_visitor,
                                           (void *)ctx);
    }

    scoop_gc_parallel_mark_work_finish_one(work);
  }

  return 0;
}

static uint32_t scoop_gc_immix_parallel_mark_worker_count(void) {
  const char *raw = getenv("SCOOP_GC_IMMIX_PARALLEL_MARK");
  if (raw == 0 || raw[0] == 0) {
    return 0;
  }

  errno = 0;
  char *end = 0;
  unsigned long v = strtoul(raw, &end, 10);
  if (end == raw || errno != 0) {
    return 0;
  }
  if (v == 0) {
    return 0;
  }
  if (v == 1) {
    v = 4;
  }
  if (v > 32) {
    v = 32;
  }
  if (v > (unsigned long)UINT32_MAX) {
    v = (unsigned long)UINT32_MAX;
  }
  if (v < 2) {
    v = 2;
  }

  return (uint32_t)v;
}

static uint32_t scoop_gc_immix_parallel_sweep_worker_count(void) {
  const char *raw = getenv("SCOOP_GC_IMMIX_PARALLEL_SWEEP");
  if (raw == 0 || raw[0] == 0) {
    return 0;
  }

  errno = 0;
  char *end = 0;
  unsigned long v = strtoul(raw, &end, 10);
  if (end == raw || errno != 0) {
    return 0;
  }
  if (v == 0) {
    return 0;
  }
  if (v == 1) {
    v = 4;
  }
  if (v > 32) {
    v = 32;
  }
  if (v > (unsigned long)UINT32_MAX) {
    v = (unsigned long)UINT32_MAX;
  }
  if (v < 2) {
    v = 2;
  }

  return (uint32_t)v;
}

static uint32_t scoop_gc_env_flag_is_truthy(const char *name) {
  if (name == 0) {
    return 0;
  }
  const char *raw = getenv(name);
  if (raw == 0) {
    return 0;
  }
  // 跳过前导空白。
  while (*raw == ' ' || *raw == '\t' || *raw == '\n' || *raw == '\r') {
    raw++;
  }
  if (*raw == 0) {
    return 0;
  }
  // 常见 false/0 语义（大小写不敏感的子集）。
  if ((raw[0] == '0') && (raw[1] == 0)) {
    return 0;
  }
  if ((raw[0] == 'f' || raw[0] == 'F') && (raw[1] == 'a' || raw[1] == 'A') &&
      (raw[2] == 'l' || raw[2] == 'L') && (raw[3] == 's' || raw[3] == 'S') &&
      (raw[4] == 'e' || raw[4] == 'E') && (raw[5] == 0)) {
    return 0;
  }
  return 1;
}

static uint32_t scoop_gc_verify_roots_enabled(void) {
  return scoop_gc_env_flag_is_truthy("SCOOP_GC_VERIFY_ROOTS");
}

// --- GC roots 强校验（GC-FIX Phase B2c） ---
//
// 目的：
// - 用于诊断/回归：避免 “roots 枚举不全 / roots 更新不全” 导致的 silent mis-collection。
// - 通过 env `SCOOP_GC_VERIFY_ROOTS=1` 启用（slow path；不追求性能）。
//
// 校验内容（v0）：
// - GC 完成（sweep + region sweep + compaction + roots update）后，再次枚举所有 roots slots；
// - 要求：每个非 NULL roots 值必须指向当前 heap.objects 中的某个 live 对象（对象头地址）；
// - 对 stackmap roots：要求 stackmap lookup 至少命中 1 条 record（否则视为“未产生/未注册 stackmaps”）。
//
// 注意：
// - 该校验在 stop-the-world 期间运行（持有 Immix state->lock），因此可以安全读取 Parked 线程的 stack slots；
// - 对 InNative 线程：按当前协议仅验证 `native_roots`（不尝试 walk 其 managed frames）。

typedef struct ScoopGcVerifyRootsState {
  uint32_t errors;
  uint32_t max_errors;
} ScoopGcVerifyRootsState;

typedef struct ScoopGcVerifySlotCtx {
  ScoopGcVerifyRootsState *state;
  const char *kind;
  uintptr_t thread_id;
  ScoopGcHeap *heap;
  const ScoopGcHeapMembershipIndex *membership;
} ScoopGcVerifySlotCtx;

static void scoop_gc_verify_roots_record_error(ScoopGcVerifyRootsState *st,
                                               const char *kind,
                                               uintptr_t thread_id,
                                               const void *slot_addr,
                                               const void *value,
                                               const char *msg) {
  if (st == 0) {
    return;
  }

  if (st->errors < st->max_errors) {
    if (thread_id != 0) {
      (void)fprintf(stderr,
                    "[scooprt][gc][verify-roots] %s: kind=%s thread=0x%" PRIxPTR
                    " slot=%p value=%p\n",
                    (msg != 0) ? msg : "error",
                    (kind != 0) ? kind : "unknown",
                    thread_id,
                    slot_addr,
                    value);
    } else {
      (void)fprintf(stderr,
                    "[scooprt][gc][verify-roots] %s: kind=%s slot=%p value=%p\n",
                    (msg != 0) ? msg : "error",
                    (kind != 0) ? kind : "unknown",
                    slot_addr,
                    value);
    }
  }

  st->errors += 1;
}

static void scoop_gc_verify_root_slot_visitor(void **slot, void *raw_ctx) {
  if (slot == 0 || raw_ctx == 0) {
    return;
  }

  ScoopGcVerifySlotCtx *ctx = (ScoopGcVerifySlotCtx *)raw_ctx;
  if (ctx->state == 0) {
    return;
  }

  void *raw = *slot;
  if (raw == 0) {
    return;
  }

  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)raw;
  if (!scoop_gc_heap_membership_index_contains(ctx->membership, ctx->heap, obj)) {
    scoop_gc_verify_roots_record_error(
        ctx->state, ctx->kind, ctx->thread_id, (const void *)slot, (const void *)raw, "invalid root");

    // 诊断辅助：判断该值是否“看起来像某个 GC 对象内部的 derived pointer”。
    //
    // 说明：
    // - stackmap roots 在 v0 约定下应当只包含对象头指针（`ScoopGcObjectHeader*`）；
    // - 若 value 指向某个对象的 payload/字段中间位置，说明 codegen/statepoint roots 里混入了
    //   derived pointer（常见于 `getelementptr` 结果跨越了 safepoint 并被当作 root 溢出到 spill slot）。
    if (ctx->heap != 0) {
      const uintptr_t addr = (uintptr_t)raw;
      ScoopGcObjectHeader *container = 0;
      for (ScoopGcObjectHeader *it = ctx->heap->objects; it != 0; it = it->next) {
        const uintptr_t start = (uintptr_t)it;
        const uintptr_t size = (uintptr_t)it->size_bytes;
        if (size == 0) {
          continue;
        }
        const uintptr_t end = start + size;
        if (addr >= start && addr < end) {
          container = it;
          break;
        }
      }

      if (container != 0) {
        const uintptr_t start = (uintptr_t)container;
        const uintptr_t off = addr - start;
        const uint64_t type_id = (container->type_desc != 0) ? container->type_desc->type_id : 0;
        (void)fprintf(stderr,
                      "[scooprt][gc][verify-roots] note: value looks like interior ptr: obj=%p off=0x%" PRIxPTR
                      " size=0x%" PRIx64 " type_id=0x%" PRIx64 "\n",
                      (void *)container,
                      off,
                      container->size_bytes,
                      type_id);
      } else {
        (void)fprintf(stderr,
                      "[scooprt][gc][verify-roots] note: value not in GC heap list\n");
      }
    }
  }
}

static void scoop_gc_verify_roots_after_gc_unlocked(ScoopGcHeap *heap,
                                                    pthread_t initiator,
                                                    void *initiator_stack_walking_ctx) {
  if (heap == 0) {
    return;
  }

  ScoopGcVerifyRootsState st = {
      .errors = 0,
      .max_errors = 16,
  };

  ScoopGcHeapMembershipIndex membership = {0};
  (void)scoop_gc_heap_membership_index_build_unlocked(&membership, heap);

  for (ScoopGcPinnedRecord *it = scoop_gc_pinned_objects; it != 0; it = it->next) {
    if (it->object == 0 || it->pin_count == 0) {
      continue;
    }
    if (!scoop_gc_heap_membership_index_contains(&membership, heap, it->object)) {
      scoop_gc_verify_roots_record_error(
          &st, "pin", /*thread_id=*/0, (const void *)&it->object, (const void *)it->object, "pinned root not live");
    }
  }

  for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
    if (it->object == 0) {
      continue;
    }
    if (!scoop_gc_heap_membership_index_contains(&membership, heap, it->object)) {
      scoop_gc_verify_roots_record_error(&st,
                                         "handle",
                                         /*thread_id=*/0,
                                         (const void *)&it->object,
                                         (const void *)it->object,
                                         "handle root not live");
    }
  }
  {
    ScoopGcVerifySlotCtx v = {
        .state = &st,
        .kind = "global_root",
        .thread_id = 0,
        .heap = heap,
        .membership = &membership,
    };
    (void)scoop_gc_global_roots_visit_unlocked(scoop_gc_verify_root_slot_visitor, (void *)&v);
  }

  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    const uintptr_t tid = scoop_gc_thread_id_for_diag(it->thread);

    if (pthread_equal(it->thread, initiator)) {
      if (initiator_stack_walking_ctx != 0) {
        ScoopGcVerifySlotCtx v = {
            .state = &st,
            .kind = "stackmap",
            .thread_id = tid,
            .heap = heap,
            .membership = &membership,
        };
        ScoopGcManagedRootMap root_map =
            scoop_gc_managed_root_map_from_stackmap_ctx(initiator_stack_walking_ctx);
        ScoopGcRootMapVisitResult root_map_result = {0};
        uint32_t records_hit = 0;
        (void)scoop_gc_root_map_visit_slots(
            &root_map, scoop_gc_verify_root_slot_visitor, (void *)&v, &root_map_result);
        uint32_t err = root_map_result.visit_error;
        records_hit = root_map_result.units_hit;
        if (err != SCOOP_STACKMAP_VISIT_OK) {
          scoop_gc_verify_roots_record_error(
              &st, "stackmap", tid, /*slot_addr=*/0, /*value=*/0, "stackmap visit failed");
        } else if (records_hit == 0) {
          scoop_gc_verify_roots_record_error(
              &st, "stackmap", tid, /*slot_addr=*/0, /*value=*/0, "stackmap hit 0 records");
        }
      }

      if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
        if (it->stack_walking_ctx == 0) {
          scoop_gc_verify_roots_record_error(
              &st, "stackmap", tid, /*slot_addr=*/0, /*value=*/0, "in-native thread missing stack_walking_ctx");
        } else {
          ScoopGcVerifySlotCtx v = {
              .state = &st,
              .kind = "stackmap",
              .thread_id = tid,
              .heap = heap,
              .membership = &membership,
          };
          ScoopGcManagedRootMap root_map =
              scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
          ScoopGcRootMapVisitResult root_map_result = {0};
          (void)scoop_gc_root_map_visit_slots(
              &root_map, scoop_gc_verify_root_slot_visitor, (void *)&v, &root_map_result);
          uint32_t err = root_map_result.visit_error;
          if (err != SCOOP_STACKMAP_VISIT_OK) {
            scoop_gc_verify_roots_record_error(
                &st, "stackmap", tid, /*slot_addr=*/0, /*value=*/0, "stackmap visit failed");
          }
        }

        ScoopGcVerifySlotCtx v = {
            .state = &st,
            .kind = "native_roots",
            .thread_id = tid,
            .heap = heap,
            .membership = &membership,
        };
        (void)scoop_gc_native_roots_visit_slots(
            it->native_roots, it->native_roots_len, scoop_gc_verify_root_slot_visitor, (void *)&v);
      }
      continue;
    }

    if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
      if (it->stack_walking_ctx == 0) {
        scoop_gc_verify_roots_record_error(
            &st, "stackmap", tid, /*slot_addr=*/0, /*value=*/0, "in-native thread missing stack_walking_ctx");
      } else {
        ScoopGcVerifySlotCtx v = {
            .state = &st,
            .kind = "stackmap",
            .thread_id = tid,
            .heap = heap,
            .membership = &membership,
        };
        ScoopGcManagedRootMap root_map =
            scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
        ScoopGcRootMapVisitResult root_map_result = {0};
        (void)scoop_gc_root_map_visit_slots(
            &root_map, scoop_gc_verify_root_slot_visitor, (void *)&v, &root_map_result);
        uint32_t err = root_map_result.visit_error;
        if (err != SCOOP_STACKMAP_VISIT_OK) {
          scoop_gc_verify_roots_record_error(
              &st, "stackmap", tid, /*slot_addr=*/0, /*value=*/0, "stackmap visit failed");
        }
      }

      ScoopGcVerifySlotCtx v = {
          .state = &st,
          .kind = "native_roots",
          .thread_id = tid,
          .heap = heap,
          .membership = &membership,
      };
      (void)scoop_gc_native_roots_visit_slots(
          it->native_roots, it->native_roots_len, scoop_gc_verify_root_slot_visitor, (void *)&v);
      continue;
    }

    if (it->state == SCOOP_GC_THREAD_PARKED) {
      if (it->stack_walking_ctx == 0) {
        scoop_gc_verify_roots_record_error(
            &st, "stackmap", tid, /*slot_addr=*/0, /*value=*/0, "parked thread missing stack_walking_ctx");
        continue;
      }

      ScoopGcVerifySlotCtx v = {
          .state = &st,
          .kind = "stackmap",
          .thread_id = tid,
          .heap = heap,
          .membership = &membership,
      };
      ScoopGcManagedRootMap root_map =
          scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
      ScoopGcRootMapVisitResult root_map_result = {0};
      uint32_t records_hit = 0;
      (void)scoop_gc_root_map_visit_slots(
          &root_map, scoop_gc_verify_root_slot_visitor, (void *)&v, &root_map_result);
      uint32_t err = root_map_result.visit_error;
      records_hit = root_map_result.units_hit;
      if (err != SCOOP_STACKMAP_VISIT_OK) {
        scoop_gc_verify_roots_record_error(
            &st, "stackmap", tid, /*slot_addr=*/0, /*value=*/0, "stackmap visit failed");
      } else if (records_hit == 0) {
        scoop_gc_verify_roots_record_error(
            &st, "stackmap", tid, /*slot_addr=*/0, /*value=*/0, "stackmap hit 0 records");
      }
      continue;
    }

    // STW 已达成：除 initiator 与 InNative 线程外，其余线程必须为 Parked。
    scoop_gc_verify_roots_record_error(
        &st, "thread_state", tid, /*slot_addr=*/0, /*value=*/0, "unexpected thread state during verify");
  }

  scoop_gc_heap_membership_index_destroy(&membership);

  if (st.errors != 0) {
    (void)fprintf(stderr,
                  "[scooprt][gc][verify-roots] found %u error(s); aborting\n",
                  (unsigned)st.errors);
    abort();
  }
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

typedef struct ScoopGcImmixUpdateCtx {
  // live 集合（from-space 指针），按地址排序，用于 roots update 的 membership 过滤。
  //
  // 说明：
  // - compaction 会复用 `obj->next` 字段存放 forwarding pointer，因此 heap.objects 链表在 commit 后
  //   会暂时失效；roots update 阶段不能再通过遍历 heap.objects 做 membership 判断；
  // - stackmap roots（尤其是 statepoint/patchpoint records）可能包含 pointer-sized 但不是 GC object 的值，
  //   必须过滤，否则对这些值解引用读取 `obj->next` 会崩溃。
  ScoopGcObjectHeader **live_sorted;
  size_t live_len;
} ScoopGcImmixUpdateCtx;

static int scoop_gc_ptr_cmp(const void *a, const void *b) {
  const ScoopGcObjectHeader *pa = *(ScoopGcObjectHeader *const *)a;
  const ScoopGcObjectHeader *pb = *(ScoopGcObjectHeader *const *)b;
  const uintptr_t ua = (uintptr_t)pa;
  const uintptr_t ub = (uintptr_t)pb;
  if (ua < ub) {
    return -1;
  }
  if (ua > ub) {
    return 1;
  }
  return 0;
}

static uint32_t scoop_gc_immix_live_set_contains(const ScoopGcImmixUpdateCtx *ctx,
                                                 ScoopGcObjectHeader *obj) {
  if (ctx == 0 || obj == 0 || ctx->live_sorted == 0 || ctx->live_len == 0) {
    return 0;
  }

  size_t lo = 0;
  size_t hi = ctx->live_len;
  const uintptr_t target = (uintptr_t)obj;

  while (lo < hi) {
    const size_t mid = lo + ((hi - lo) / 2u);
    const uintptr_t cur = (uintptr_t)ctx->live_sorted[mid];
    if (cur == target) {
      return 1;
    }
    if (cur < target) {
      lo = mid + 1u;
    } else {
      hi = mid;
    }
  }

  return 0;
}

static void scoop_gc_immix_update_slot_visitor(void **slot, void *raw_ctx) {
  if (slot == 0) {
    return;
  }
  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)(*slot);
  if (obj == 0) {
    return;
  }

  const ScoopGcImmixUpdateCtx *ctx = (const ScoopGcImmixUpdateCtx *)raw_ctx;
  if (ctx != 0 && !scoop_gc_immix_live_set_contains(ctx, obj)) {
    // 重要：只对 live 集合中的对象指针做 forwarding follow（避免 stackmap 假 roots 崩溃）。
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
    // 回收：放回 free list，避免 abort 后 state->free_blocks “悄悄丢块”。
    rb->next_free = state->free_blocks;
    state->free_blocks = rb;
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
  state->nursery_free_blocks = 0;
  state->nursery_current_block = 0;

  for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
    it->next_free = 0;

    // nursery blocks：bump-only，不进入 reusable list（避免 holes 复用带来的不可控成本）。
    if (it->generation == (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_NURSERY) {
      if (it->live_objects == 0) {
        scoop_gc_immix_block_reset(it);
        it->next_free = state->nursery_free_blocks;
        state->nursery_free_blocks = it;
      } else {
        // 非空 nursery：保持 cursor 单调递增；不回退到 holes。
        if (it->cursor < it->payload_start) {
          it->cursor = it->payload_start;
        }
        if (it->cursor > it->limit) {
          it->cursor = it->limit;
        }
        it->hole_limit = it->limit;
      }
      continue;
    }

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
                                   ScoopGcImmixBlock *evac_blocks,
                                   pthread_t initiator,
                                   void *initiator_stack_walking_ctx) {
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

    uint64_t raw_size = from->size_bytes;
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

  // roots update 的 membership 过滤依赖 live 集合（避免 stackmap 假 roots 触发崩溃）。
  // 由于 forwarding pointer 复用 `obj->next` 字段，commit 后 heap.objects 链表会暂时失效，
  // 因此这里对 live 数组就地排序并用二分查找做判定。
  qsort(live, live_len, sizeof(ScoopGcObjectHeader *), scoop_gc_ptr_cmp);
  ScoopGcImmixUpdateCtx update_ctx = {
      .live_sorted = live,
      .live_len = live_len,
  };

  // 3a) roots update：roots slots 原地改写为新地址（moving GC 的关键语义）。
  //
  // 注意：必须更新“所有已注册线程”的 roots；否则在多线程 + moving/compaction 下会产生悬挂指针。
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    // T1505c：InNative 线程 roots 来自 native_roots buffer（同样需要在 moving GC 中被更新）。
    if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
      if (it->stack_walking_ctx == 0) {
        (void)fprintf(stderr,
                      "[scooprt][gc][stackmap] missing in-native ctx for roots update (thread=0x%" PRIxPTR
                      ")\n",
                      (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
        abort();
      }

      (void)scoop_gc_native_roots_visit_slots(
          it->native_roots, it->native_roots_len, scoop_gc_immix_update_slot_visitor, &update_ctx);
      {
        ScoopGcManagedRootMap root_map =
            scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
        ScoopGcRootMapVisitResult root_map_result = {0};
        (void)scoop_gc_root_map_visit_slots(
            &root_map, scoop_gc_immix_update_slot_visitor, &update_ctx, &root_map_result);
        uint32_t err = root_map_result.visit_error;
        if (err != SCOOP_STACKMAP_VISIT_OK) {
          (void)fprintf(stderr,
                        "[scooprt][gc][stackmap] update in-native caller roots failed: err=%u "
                        "(thread=0x%" PRIxPTR ")\n",
                        (unsigned)err,
                        (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
          abort();
        }
      }
      continue;
    }

    // B2b：initiator/parked 线程 roots 更新仅走 stackmap spill slots（statepoint + gc.relocate 依赖该语义）。
    if (pthread_equal(it->thread, initiator)) {
      if (initiator_stack_walking_ctx == 0) {
        (void)fprintf(stderr, "[scooprt][gc][stackmap] missing initiator ctx for roots update\n");
        abort();
      }
      ScoopGcManagedRootMap root_map =
          scoop_gc_managed_root_map_from_stackmap_ctx(initiator_stack_walking_ctx);
      ScoopGcRootMapVisitResult root_map_result = {0};
      (void)scoop_gc_root_map_visit_slots(
          &root_map, scoop_gc_immix_update_slot_visitor, &update_ctx, &root_map_result);
      uint32_t err = root_map_result.visit_error;
      if (err != SCOOP_STACKMAP_VISIT_OK) {
        (void)fprintf(stderr,
                      "[scooprt][gc][stackmap] update initiator roots failed: err=%u\n",
                      (unsigned)err);
        abort();
      }
      continue;
    }

    if (it->state == SCOOP_GC_THREAD_PARKED) {
      // 若该线程通过 `scoop_gc_safepoint()`（非 poll）进入 Parked，则可能没有 ctx；
      // 此时无法 walk stackmap roots，只能退化为“该线程无 stackmap roots 可枚举”。
      if (it->stack_walking_ctx != 0) {
        ScoopGcManagedRootMap root_map =
            scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
        ScoopGcRootMapVisitResult root_map_result = {0};
        (void)scoop_gc_root_map_visit_slots(
            &root_map, scoop_gc_immix_update_slot_visitor, &update_ctx, &root_map_result);
        uint32_t err = root_map_result.visit_error;
        if (err != SCOOP_STACKMAP_VISIT_OK) {
          (void)fprintf(stderr,
                        "[scooprt][gc][stackmap] update roots failed: err=%u (thread=0x%" PRIxPTR
                        ")\n",
                        (unsigned)err,
                        (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
          abort();
        }
      }
      continue;
    }

    // STW 已达成：除 initiator 与 InNative 线程外，其余线程必须为 Parked。
    (void)fprintf(stderr,
                  "[scooprt][gc] unexpected thread state during roots update: state=%u "
                  "(thread=0x%" PRIxPTR ")\n",
                  (unsigned)it->state,
                  (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
    abort();
  }

  // 3a2) stable handle table update：handle->obj 槽位同样属于 roots，需要在 moving/compaction 后被改写。
  for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
    if (it->object == 0) {
      continue;
    }
    scoop_gc_immix_update_slot_visitor((void **)&it->object, &update_ctx);
  }

  // 3a3) module-global roots update：module-local backing globals 同样属于永久 roots。
  (void)scoop_gc_global_roots_visit_unlocked(scoop_gc_immix_update_slot_visitor, &update_ctx);

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
                                         &update_ctx);
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
      // nursery blocks 不应被释放（它们作为“可配置上限”的一部分提供硬边界）。
      if (eb->generation == (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_NURSERY) {
        scoop_gc_immix_block_reset(eb);
        eb->next_free = state->nursery_free_blocks;
        state->nursery_free_blocks = eb;
      } else {
        scoop_gc_immix_state_remove_and_free_block(state, eb);
      }
    }
    eb = next;
  }

  // 7) 重新构建 free/reusable block lists，确保 allocator 能继续工作且不包含悬挂指针。
  scoop_gc_immix_state_rebuild_block_lists(state);

  free(moves);
  free(live);
}

// --- Minor GC（TODO T1412c）：nursery evacuation（stop-the-world） ---
//
// 设计要点（v0）：
// - 只追踪 nursery 可达对象（roots + nursery 内部引用）；不扫描老年代对象字段；
// - 存活对象复制到老年代（to-space），再整体 reset nursery blocks；
// - roots / handle 槽位原地更新为新地址（forwarding pointer）；
// - pinned 对象不得移动：若 pinned 位于 nursery，则先把其所在 block 晋升为 old（避免 reset）。

typedef struct ScoopGcImmixMinorLiveSet {
  ScoopGcObjectHeader **items;
  size_t len;
  size_t cap;
} ScoopGcImmixMinorLiveSet;

static uint32_t scoop_gc_immix_minor_live_set_push(ScoopGcImmixMinorLiveSet *set,
                                                   ScoopGcObjectHeader *obj) {
  if (set == 0 || obj == 0) {
    return 0;
  }

  if (set->len == set->cap) {
    size_t new_cap = (set->cap == 0) ? 1024u : set->cap * 2u;
    if (new_cap < set->cap) {
      return 0;
    }
    if (new_cap > (SIZE_MAX / sizeof(ScoopGcObjectHeader *))) {
      return 0;
    }
    void *p = realloc(set->items, new_cap * sizeof(ScoopGcObjectHeader *));
    if (p == 0) {
      return 0;
    }
    set->items = (ScoopGcObjectHeader **)p;
    set->cap = new_cap;
  }

  set->items[set->len++] = obj;
  return 1;
}

typedef struct ScoopGcImmixMinorWorkStack {
  ScoopGcObjectHeader **items;
  size_t len;
  size_t cap;
} ScoopGcImmixMinorWorkStack;

static uint32_t scoop_gc_immix_minor_work_stack_push(ScoopGcImmixMinorWorkStack *stack,
                                                     ScoopGcObjectHeader *obj) {
  if (stack == 0 || obj == 0) {
    return 0;
  }

  if (stack->len == stack->cap) {
    size_t new_cap = (stack->cap == 0) ? 1024u : stack->cap * 2u;
    if (new_cap < stack->cap) {
      return 0;
    }
    if (new_cap > (SIZE_MAX / sizeof(ScoopGcObjectHeader *))) {
      return 0;
    }
    void *p = realloc(stack->items, new_cap * sizeof(ScoopGcObjectHeader *));
    if (p == 0) {
      return 0;
    }
    stack->items = (ScoopGcObjectHeader **)p;
    stack->cap = new_cap;
  }

  stack->items[stack->len++] = obj;
  return 1;
}

static ScoopGcObjectHeader *scoop_gc_immix_minor_work_stack_pop(ScoopGcImmixMinorWorkStack *stack) {
  if (stack == 0 || stack->len == 0) {
    return 0;
  }
  stack->len -= 1;
  return stack->items[stack->len];
}

typedef struct ScoopGcImmixMinorMarkCtx {
  ScoopGcHeap *heap;
  uint32_t mark_value;
  ScoopGcImmixMinorWorkStack *stack;
  ScoopGcImmixMinorLiveSet *live;
  const ScoopGcHeapMembershipIndex *membership;
  size_t small_object_cap;
  uint32_t oom;
} ScoopGcImmixMinorMarkCtx;

static void scoop_gc_immix_minor_mark_object_if_needed(ScoopGcImmixMinorMarkCtx *ctx,
                                                       ScoopGcObjectHeader *obj) {
  if (ctx == 0 || obj == 0) {
    return;
  }
  if (ctx->oom) {
    return;
  }

  if (obj->mark == ctx->mark_value) {
    return;
  }

  obj->mark = ctx->mark_value;
  if (!scoop_gc_immix_minor_work_stack_push(ctx->stack, obj) ||
      !scoop_gc_immix_minor_live_set_push(ctx->live, obj)) {
    ctx->oom = 1;
    return;
  }
}

static void scoop_gc_immix_minor_mark_slot_visitor(void **slot, void *raw_ctx) {
  if (slot == 0 || raw_ctx == 0) {
    return;
  }

  ScoopGcImmixMinorMarkCtx *ctx = (ScoopGcImmixMinorMarkCtx *)raw_ctx;
  if (ctx->oom) {
    return;
  }
  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)(*slot);
  if (obj == 0) {
    return;
  }

  // 重要：stackmap records 里可能包含“刚好是 pointer-sized 但不是 GC roots”的 slot；
  // 需要先做 membership 过滤，避免把垃圾值当作 `ScoopGcObjectHeader*` 解引用并崩溃。
  if (!scoop_gc_heap_membership_index_contains(ctx->membership, ctx->heap, obj)) {
    return;
  }

  // nursery 只包含 small objects（位于 Immix blocks 内）；large object 直接视为 old。
  uint64_t raw_size = obj->size_bytes;
  if (raw_size == 0 || raw_size > (uint64_t)SIZE_MAX) {
    return;
  }
  if ((size_t)raw_size > ctx->small_object_cap) {
    return;
  }

  ScoopGcImmixBlock *block = scoop_gc_immix_block_from_object((void *)obj);
  if (block == 0) {
    return;
  }
  if (block->generation != (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_NURSERY) {
    return;
  }

  scoop_gc_immix_minor_mark_object_if_needed(ctx, obj);
}

static void scoop_gc_immix_minor_promote_pinned_nursery_blocks_unlocked(
    ScoopGcImmixState *state,
    ScoopGcHeap *heap,
    const ScoopGcHeapMembershipIndex *membership,
    size_t small_object_cap) {
  if (state == 0 || heap == 0 || membership == 0) {
    return;
  }

  for (ScoopGcPinnedRecord *it = scoop_gc_pinned_objects; it != 0; it = it->next) {
    if (it->object == 0) {
      continue;
    }
    if (it->pin_count == 0) {
      continue;
    }

    ScoopGcObjectHeader *obj = it->object;
    if (!scoop_gc_heap_membership_index_contains(membership, heap, obj)) {
      continue;
    }

    uint64_t raw_size = obj->size_bytes;
    if (raw_size == 0 || raw_size > (uint64_t)SIZE_MAX) {
      continue;
    }
    if ((size_t)raw_size > small_object_cap) {
      continue;
    }

    ScoopGcImmixBlock *block = scoop_gc_immix_block_from_object((void *)obj);
    if (block == 0) {
      continue;
    }
    if (block->generation != (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_NURSERY) {
      continue;
    }

    // pinned 对象不得移动：把所在 nursery block 晋升为 old，避免 minor reset。
    scoop_gc_immix_nursery_promote_block_to_old_unlocked(state, block);
  }
}

static void scoop_gc_immix_minor_tospace_commit_blocks(ScoopGcImmixToSpace *tospace,
                                                       ScoopGcImmixState *state) {
  if (tospace == 0 || state == 0) {
    return;
  }

  // 1) 将 “借用的 free blocks” 作为 reusable blocks 归还：它们已写入 live objects。
  ScoopGcImmixBlock *rb = tospace->reused_blocks;
  while (rb != 0) {
    ScoopGcImmixBlock *next = rb->next_free;
    rb->next_free = 0;

    if (rb->cursor < rb->limit) {
      rb->next_free = state->reusable_blocks;
      state->reusable_blocks = rb;
    } else if (rb->live_objects == 0) {
      rb->next_free = state->free_blocks;
      state->free_blocks = rb;
    }

    rb = next;
  }

  // 2) 挂入新分配的 blocks，并按 “是否仍有空间” 放入 reusable/free list。
  ScoopGcImmixBlock *nb = tospace->new_blocks;
  while (nb != 0) {
    ScoopGcImmixBlock *next = nb->next_all;

    nb->next_all = state->all_blocks;
    state->all_blocks = nb;

    nb->next_free = 0;
    if (nb->cursor < nb->limit) {
      nb->next_free = state->reusable_blocks;
      state->reusable_blocks = nb;
    } else if (nb->live_objects == 0) {
      nb->next_free = state->free_blocks;
      state->free_blocks = nb;
    }

    nb = next;
  }

  tospace->current = 0;
  tospace->reused_blocks = 0;
  tospace->new_blocks = 0;
}

static void scoop_gc_immix_minor_reset_nursery_unlocked(ScoopGcImmixState *state) {
  if (state == 0) {
    return;
  }

  state->nursery_free_blocks = 0;
  state->nursery_current_block = 0;

  uint32_t blocks = 0;
  for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
    it->next_free = 0;

    if (it->generation != (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_NURSERY) {
      continue;
    }

    scoop_gc_immix_block_reset(it);
    it->next_free = state->nursery_free_blocks;
    state->nursery_free_blocks = it;

    if (blocks != UINT32_MAX) {
      blocks += 1;
    }
  }

  // 防御性同步：避免 `nursery_blocks` 与实际 generation 分类漂移。
  state->nursery_blocks = blocks;
}

static uint32_t scoop_gc_collect_minor_internal(uint32_t use_deadline, uint32_t deadline_ms) {
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  ScoopGcImmixState *state = scoop_gc_immix_state();
  if (state == 0) {
    return 0;
  }
  if (!state->lock_inited) {
    return 0;
  }

  // 未启用 nursery：minor 为 no-op（保持语义可预期）。
  if (state->nursery_max_blocks == 0) {
    return 0;
  }

  pthread_t self = pthread_self();

  scoop_gc_immix_lock(state);

  // 若别的线程已经发起了 STW，本线程此时不能只是“等它结束”：
  // 那会把自己留在 Running 状态，导致 initiator 永远等不到 parked_count。
  // 这里直接把这次 minor collect 退化为一次 safepoint poll，让当前线程先参与对方的 STW。
  if (scoop_gc_stw_requested_load(&scoop_gc_stw) && !pthread_equal(self, scoop_gc_stw.initiator)) {
    scoop_gc_immix_unlock(state);
    scoop_gc_safepoint_poll();
    return 0;
  }

  // 保证同一时刻只允许一个 GC 周期（major/minor 都走同一 STW）。
  while (scoop_gc_stw_requested_load(&scoop_gc_stw)) {
    (void)pthread_cond_wait(&scoop_gc_stw_cond, &state->lock);
  }

  if (use_deadline) {
    if (!scoop_gc_stop_the_world_try_begin_unlocked(self, deadline_ms)) {
      scoop_gc_immix_unlock(state);
      return 0;
    }
  } else {
    scoop_gc_stop_the_world_begin_unlocked(self);
  }

  uint32_t did_commit = 0;

  ScoopGcHeap *heap = &scoop_gc_heap;
  const size_t small_object_cap = scoop_gc_immix_block_payload_capacity();

  // 0) 构建 heap membership 索引：
  // - roots/slot tracing 需要过滤 stackmap 假 roots；
  // - minor commit 后会写入 forwarding pointer 破坏 heap.objects 链表，因此需要提前快照。
  ScoopGcHeapMembershipIndex membership = {0};
  if (!scoop_gc_heap_membership_index_build_unlocked(&membership, heap)) {
    scoop_gc_stop_the_world_end_unlocked();
    scoop_gc_immix_unlock(state);
    return 0;
  }

  // B2b：Immix roots 枚举不再扫描 shadow stack。
  //
  // 约定：
  // - InNative 线程：roots 来自 `native_roots` slots（enter_native 注册），以及 pinned/handles；
  // - 其余线程：roots 来自 stackmap（需要可用的 unwind ctx）。
  uint32_t initiator_needs_stackmap_roots = 1;
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    if (!pthread_equal(it->thread, self)) {
      continue;
    }
    if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
      initiator_needs_stackmap_roots = 0;
    }
    break;
  }

  void *initiator_stack_walking_ctx = 0;
  if (initiator_needs_stackmap_roots) {
    initiator_stack_walking_ctx = scoop_platform_unwind_ctx_capture();
    if (initiator_stack_walking_ctx == 0) {
      (void)fprintf(stderr, "[scooprt][gc][minor] failed to capture unwind ctx\n");
      abort();
    }
  }

  // pinned 对象不得移动：若 pinned 位于 nursery，则先把其所在 block 晋升为 old（避免 reset）。
  scoop_gc_immix_minor_promote_pinned_nursery_blocks_unlocked(
      state, heap, &membership, small_object_cap);

  // 1) mark nursery live set（仅追踪 nursery；不扫描老年代对象字段）
  uint32_t mark_value = scoop_gc_collect_next_mark_value(heap);
  ScoopGcImmixMinorWorkStack stack = {0};
  ScoopGcImmixMinorLiveSet live = {0};
  ScoopGcImmixMinorMarkCtx mark_ctx = {
      .heap = heap,
      .mark_value = mark_value,
      .stack = &stack,
      .live = &live,
      .membership = &membership,
      .small_object_cap = small_object_cap,
      .oom = 0,
  };

  // 2) to-space 分配与拷贝（可回滚）会用到的临时结构：
  // - 注意：这些变量必须在任何 `goto cleanup_and_return` 之前完成初始化。
  ScoopGcImmixMoveRecord *moves = 0;
  size_t move_len = 0;
  ScoopGcImmixToSpace tospace = {0};

  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    // T1505c：InNative 线程 roots 来自 native_roots buffer。
    if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
      if (it->stack_walking_ctx == 0) {
        (void)fprintf(stderr,
                      "[scooprt][gc][stackmap] missing in-native ctx for minor mark (thread=0x%" PRIxPTR
                      ")\n",
                      (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
        abort();
      }

      (void)scoop_gc_native_roots_visit_slots(
          it->native_roots, it->native_roots_len, scoop_gc_immix_minor_mark_slot_visitor, (void *)&mark_ctx);
      {
        ScoopGcManagedRootMap root_map =
            scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
        ScoopGcRootMapVisitResult root_map_result = {0};
        (void)scoop_gc_root_map_visit_slots(
            &root_map,
            scoop_gc_immix_minor_mark_slot_visitor,
            (void *)&mark_ctx,
            &root_map_result);
        uint32_t err = root_map_result.visit_error;
        if (err != SCOOP_STACKMAP_VISIT_OK) {
          (void)fprintf(stderr,
                        "[scooprt][gc][stackmap] minor mark in-native caller roots failed: err=%u "
                        "(thread=0x%" PRIxPTR ")\n",
                        (unsigned)err,
                        (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
          abort();
        }
      }
      continue;
    }

    if (initiator_needs_stackmap_roots && initiator_stack_walking_ctx != 0 &&
        pthread_equal(it->thread, self)) {
      ScoopGcManagedRootMap root_map =
          scoop_gc_managed_root_map_from_stackmap_ctx(initiator_stack_walking_ctx);
      ScoopGcRootMapVisitResult root_map_result = {0};
      (void)scoop_gc_root_map_visit_slots(
          &root_map, scoop_gc_immix_minor_mark_slot_visitor, (void *)&mark_ctx, &root_map_result);
      uint32_t err = root_map_result.visit_error;
      if (err != SCOOP_STACKMAP_VISIT_OK) {
        (void)fprintf(stderr,
                      "[scooprt][gc][stackmap] minor mark initiator roots failed: err=%u\n",
                      (unsigned)err);
        abort();
      }
      continue;
    }

    if (it->state == SCOOP_GC_THREAD_PARKED) {
      if (it->stack_walking_ctx != 0) {
        ScoopGcManagedRootMap root_map =
            scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
        ScoopGcRootMapVisitResult root_map_result = {0};
        (void)scoop_gc_root_map_visit_slots(
            &root_map,
            scoop_gc_immix_minor_mark_slot_visitor,
            (void *)&mark_ctx,
            &root_map_result);
        uint32_t err = root_map_result.visit_error;
        if (err != SCOOP_STACKMAP_VISIT_OK) {
          (void)fprintf(stderr,
                        "[scooprt][gc][stackmap] minor mark roots failed: err=%u (thread=0x%" PRIxPTR
                        ")\n",
                        (unsigned)err,
                        (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
          abort();
        }
      }
      continue;
    }

    (void)fprintf(stderr,
                  "[scooprt][gc][minor] unexpected thread state during mark roots: state=%u "
                  "(thread=0x%" PRIxPTR ")\n",
                  (unsigned)it->state,
                  (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
    abort();
  }

  // 1c) mark stable handles（spec §15.10.1）：handle 表也可能直接引用 nursery 对象。
  for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
    if (it->object == 0) {
      continue;
    }
    scoop_gc_immix_minor_mark_slot_visitor((void **)&it->object, (void *)&mark_ctx);
  }

  // 1c2) mark module-global roots：module-local backing globals 也可能直接引用 nursery 对象。
  (void)scoop_gc_global_roots_visit_unlocked(scoop_gc_immix_minor_mark_slot_visitor,
                                             (void *)&mark_ctx);

  // 1d) mark transitive closure（nursery 内引用）
  while (!mark_ctx.oom && stack.len > 0) {
    ScoopGcObjectHeader *obj = scoop_gc_immix_minor_work_stack_pop(&stack);
    if (obj == 0) {
      continue;
    }
    if (obj->type_desc == 0) {
      continue;
    }

    (void)scoop_gc_type_descriptor_trace(obj->type_desc,
                                         (void *)obj,
                                         scoop_gc_immix_minor_mark_slot_visitor,
                                         (void *)&mark_ctx);
  }

  if (mark_ctx.oom) {
    goto cleanup_and_return;
  }

  // 2) to-space 分配与拷贝（可回滚）：失败则放弃本轮 minor，不修改 heap。
  move_len = live.len;

  if (move_len > 0) {
    if (move_len <= (SIZE_MAX / sizeof(ScoopGcImmixMoveRecord))) {
      moves = (ScoopGcImmixMoveRecord *)malloc(move_len * sizeof(ScoopGcImmixMoveRecord));
    }
    if (moves == 0) {
      goto cleanup_and_return;
    }

    for (size_t i = 0; i < move_len; i++) {
      ScoopGcObjectHeader *from = live.items[i];
      if (from == 0) {
        continue;
      }

      uint64_t raw_size = from->size_bytes;
      void *p = scoop_gc_immix_tospace_alloc(&tospace, state, raw_size);
      if (p == 0) {
        scoop_gc_immix_tospace_abort(&tospace, state);
        goto cleanup_and_return;
      }

      size_t size = (raw_size > (uint64_t)SIZE_MAX) ? (size_t)SIZE_MAX : (size_t)raw_size;
      (void)memcpy(p, (const void *)from, size);

      ScoopGcObjectHeader *to = (ScoopGcObjectHeader *)p;
      to->next = 0;

      moves[i].from = from;
      moves[i].to = to;
      moves[i].from_block = 0;
      moves[i].size = raw_size;
    }

    // 3) commit：写入 forwarding pointer + 更新 roots + 修复对象内部引用槽位。
    for (size_t i = 0; i < move_len; i++) {
      if (moves[i].from == 0 || moves[i].to == 0) {
        continue;
      }
      scoop_gc_immix_object_set_forwarding_ptr(moves[i].from, moves[i].to);
    }

    // roots update 的 membership 过滤依赖“被搬迁的 nursery live 集合”，避免 stackmap 假 roots 崩溃。
    // 这里对 live 数组就地排序并用二分查找做判定（复用 compaction 的 update visitor）。
    qsort(live.items, live.len, sizeof(ScoopGcObjectHeader *), scoop_gc_ptr_cmp);
    ScoopGcImmixUpdateCtx update_ctx = {
        .live_sorted = live.items,
        .live_len = live.len,
    };

    // 3a) roots update（stackmap/native roots slots 原地改写为新地址）
    for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
      if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
        if (it->stack_walking_ctx == 0) {
          (void)fprintf(stderr,
                        "[scooprt][gc][stackmap] missing in-native ctx for minor roots update "
                        "(thread=0x%" PRIxPTR ")\n",
                        (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
          abort();
        }

        (void)scoop_gc_native_roots_visit_slots(
            it->native_roots, it->native_roots_len, scoop_gc_immix_update_slot_visitor, &update_ctx);
        {
          ScoopGcManagedRootMap root_map =
              scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
          ScoopGcRootMapVisitResult root_map_result = {0};
          (void)scoop_gc_root_map_visit_slots(
              &root_map, scoop_gc_immix_update_slot_visitor, &update_ctx, &root_map_result);
          uint32_t err = root_map_result.visit_error;
          if (err != SCOOP_STACKMAP_VISIT_OK) {
            (void)fprintf(stderr,
                          "[scooprt][gc][stackmap] minor update in-native caller roots failed: err=%u "
                          "(thread=0x%" PRIxPTR ")\n",
                          (unsigned)err,
                          (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
            abort();
          }
        }
        continue;
      }

      if (initiator_needs_stackmap_roots && initiator_stack_walking_ctx != 0 &&
          pthread_equal(it->thread, self)) {
        ScoopGcManagedRootMap root_map =
            scoop_gc_managed_root_map_from_stackmap_ctx(initiator_stack_walking_ctx);
        ScoopGcRootMapVisitResult root_map_result = {0};
        (void)scoop_gc_root_map_visit_slots(
            &root_map, scoop_gc_immix_update_slot_visitor, &update_ctx, &root_map_result);
        uint32_t err = root_map_result.visit_error;
        if (err != SCOOP_STACKMAP_VISIT_OK) {
          (void)fprintf(stderr,
                        "[scooprt][gc][stackmap] minor update initiator roots failed: err=%u\n",
                        (unsigned)err);
          abort();
        }
        continue;
      }

      if (it->state == SCOOP_GC_THREAD_PARKED) {
        if (it->stack_walking_ctx != 0) {
          ScoopGcManagedRootMap root_map =
              scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
          ScoopGcRootMapVisitResult root_map_result = {0};
          (void)scoop_gc_root_map_visit_slots(
              &root_map, scoop_gc_immix_update_slot_visitor, &update_ctx, &root_map_result);
          uint32_t err = root_map_result.visit_error;
          if (err != SCOOP_STACKMAP_VISIT_OK) {
            (void)fprintf(
                stderr,
                "[scooprt][gc][stackmap] minor update roots failed: err=%u (thread=0x%" PRIxPTR
                ")\n",
                (unsigned)err,
                (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
            abort();
          }
        }
        continue;
      }

      (void)fprintf(stderr,
                    "[scooprt][gc][minor] unexpected thread state during roots update: state=%u "
                    "(thread=0x%" PRIxPTR ")\n",
                    (unsigned)it->state,
                    (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
      abort();
    }

    // 3a2) stable handle table update：handle->obj 槽位同样属于 roots。
    for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
      if (it->object == 0) {
        continue;
      }
      scoop_gc_immix_update_slot_visitor((void **)&it->object, &update_ctx);
    }

    (void)scoop_gc_global_roots_visit_unlocked(scoop_gc_immix_update_slot_visitor, &update_ctx);

    // 3b) moved object fields update：只扫描 to-space 副本（不扫描老年代）。
    for (size_t i = 0; i < move_len; i++) {
      ScoopGcObjectHeader *to = moves[i].to;
      if (to == 0) {
        continue;
      }
      if (to->type_desc == 0) {
        continue;
      }

      (void)scoop_gc_type_descriptor_trace(to->type_desc,
                                           (void *)to,
                                           scoop_gc_immix_update_slot_visitor,
                                           &update_ctx);
    }

    // 4) 重建 heap.objects：移除全部 nursery（from-space）对象 + 追加 to-space 副本。
    ScoopGcObjectHeader *new_list = 0;
    for (size_t i = 0; i < membership.len; i++) {
      ScoopGcObjectHeader *obj = membership.sorted[i];
      if (obj == 0) {
        continue;
      }
      if (scoop_gc_immix_object_is_forwarded(obj)) {
        continue;
      }

      // nursery objects 一律移除：live 已搬迁；dead 将在 reset 中回收。
      uint64_t raw_size = obj->size_bytes;
      if (raw_size != 0 && raw_size <= (uint64_t)SIZE_MAX && (size_t)raw_size <= small_object_cap) {
        ScoopGcImmixBlock *block = scoop_gc_immix_block_from_object((void *)obj);
        if (block != 0 && block->generation == (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_NURSERY) {
          continue;
        }
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

    // 5) commit to-space blocks：保证 allocator 可继续工作（不依赖全量 rebuild）。
    scoop_gc_immix_minor_tospace_commit_blocks(&tospace, state);
  } else {
    // 无 live nursery：仅做“清空 nursery + 从 heap.objects 移除 nursery 对象”。
    ScoopGcObjectHeader *new_list = 0;
    for (size_t i = 0; i < membership.len; i++) {
      ScoopGcObjectHeader *obj = membership.sorted[i];
      if (obj == 0) {
        continue;
      }

      uint64_t raw_size = obj->size_bytes;
      if (raw_size != 0 && raw_size <= (uint64_t)SIZE_MAX && (size_t)raw_size <= small_object_cap) {
        ScoopGcImmixBlock *block = scoop_gc_immix_block_from_object((void *)obj);
        if (block != 0 && block->generation == (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_NURSERY) {
          continue;
        }
      }

      obj->next = new_list;
      new_list = obj;
    }
    heap->objects = new_list;
  }

  // 6) reset nursery blocks：整体复位（工作量与 nursery blocks 数近似线性）。
  did_commit = 1;
  scoop_gc_immix_minor_reset_nursery_unlocked(state);

cleanup_and_return:
  if (moves != 0) {
    free(moves);
  }
  if (live.items != 0) {
    free(live.items);
  }
  if (stack.items != 0) {
    free(stack.items);
  }

  scoop_gc_heap_membership_index_destroy(&membership);
  scoop_platform_unwind_ctx_destroy(initiator_stack_walking_ctx);

  scoop_gc_stop_the_world_end_unlocked();
  scoop_gc_immix_unlock(state);
  return did_commit;
}

void scoop_gc_collect_minor(void) { (void)scoop_gc_collect_minor_internal(/*use_deadline=*/0, /*deadline_ms=*/0); }

uint32_t scoop_gc_try_collect_minor(uint32_t deadline_ms) {
  return scoop_gc_collect_minor_internal(/*use_deadline=*/1, deadline_ms);
}

// --- 并行 region sweep（TODO T1409c2） ---
//
// 说明：
// - region sweep 只依赖 per-block 的 bitmap/holes 状态，因此可按 blocks 分片并行；
// - 该实现默认关闭，通过 env `SCOOP_GC_IMMIX_PARALLEL_SWEEP` 打开（`1`=默认 4 workers，`N>=2`=指定）；
// - 为保持实现简单：
//   - 仍保持 heap.objects 的 sweep 单线程（步骤 3）；
//   - 仅并行化步骤 4 的 per-block 计算与分类；
//   - 最终由主线程合并 free/reusable/evac lists，并保持 compaction 语义不变。
typedef struct ScoopGcParallelSweepLists {
  ScoopGcImmixBlock *free_blocks;
  ScoopGcImmixBlock *reusable_blocks;
  ScoopGcImmixBlock *evac_blocks;
  ScoopGcImmixBlock *nursery_free_blocks;
} ScoopGcParallelSweepLists;

typedef struct ScoopGcParallelSweepJob {
  ScoopGcImmixBlock **blocks;
  size_t start;
  size_t end;
  ScoopGcParallelSweepLists out;
} ScoopGcParallelSweepJob;

static inline void scoop_gc_immix_region_sweep_merge_list(ScoopGcImmixBlock **dst,
                                                          ScoopGcImmixBlock *src) {
  while (src != 0) {
    ScoopGcImmixBlock *next = src->next_free;
    src->next_free = *dst;
    *dst = src;
    src = next;
  }
}

static inline void scoop_gc_immix_region_sweep_one_block(ScoopGcImmixBlock *block,
                                                         ScoopGcParallelSweepLists *lists) {
  if (block == 0 || lists == 0) {
    return;
  }

  block->next_free = 0;

  // nursery blocks：bump-only，不复用 holes；只在变空时 reset 并回收到 nursery free list。
  if (block->generation == (uint8_t)SCOOP_GC_IMMIX_BLOCK_GEN_NURSERY) {
    if (block->live_objects == 0) {
      scoop_gc_immix_block_reset(block);
      block->next_free = lists->nursery_free_blocks;
      lists->nursery_free_blocks = block;
      return;
    }

    // 非空 nursery：仍需要把 mark bits 融合回 alloc bits，并清空 mark bits，
    // 以保证下一轮 GC 的 mark-region / debug 观测一致；但不重建 holes/cursor。
    size_t reserved = scoop_gc_immix_block_reserved_lines(block);
    for (size_t line = reserved; line < (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK; line++) {
      uint32_t live = scoop_gc_immix_bitmap_test_bit(
          block->line_mark_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
      if (live) {
        scoop_gc_immix_bitmap_set_bit(block->line_alloc_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
      } else {
        scoop_gc_immix_bitmap_clear_bit(block->line_alloc_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
      }
      scoop_gc_immix_bitmap_clear_bit(block->line_mark_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
    }

    if (block->cursor < block->payload_start) {
      block->cursor = block->payload_start;
    }
    if (block->cursor > block->limit) {
      block->cursor = block->limit;
    }
    block->hole_limit = block->limit;
    return;
  }

  if (block->live_objects == 0) {
    scoop_gc_immix_block_reset(block);
    block->next_free = lists->free_blocks;
    lists->free_blocks = block;
    return;
  }

  // 把 live lines 保留为 alloc bits；dead lines 清零为 hole；并清空 mark bits。
  size_t reserved = scoop_gc_immix_block_reserved_lines(block);
  size_t live_lines = 0;
  for (size_t line = reserved; line < (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK; line++) {
    uint32_t live =
        scoop_gc_immix_bitmap_test_bit(block->line_mark_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
    if (live) {
      live_lines += 1;
      scoop_gc_immix_bitmap_set_bit(block->line_alloc_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
    } else {
      scoop_gc_immix_bitmap_clear_bit(block->line_alloc_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
    }
    scoop_gc_immix_bitmap_clear_bit(block->line_mark_bits, SCOOP_GC_IMMIX_BITMAP_WORDS, line);
  }

  // 准备第一个 hole：之后分配可以在 hole 内 bump。
  scoop_gc_immix_block_setup_first_hole(block);

  // moving/compaction（T1407）：选择性 block evacuation。
  size_t total_lines = (size_t)SCOOP_GC_IMMIX_LINES_PER_BLOCK - reserved;
  uint32_t is_sparse = 0;
  if (total_lines > 0 && live_lines > 0) {
    is_sparse = (live_lines * 4u) <= total_lines;
  }
  uint32_t has_pinned = scoop_gc_immix_block_contains_pinned_unlocked(block);

  if (block->cursor < block->limit) {
    if (is_sparse && !has_pinned) {
      block->next_free = lists->evac_blocks;
      lists->evac_blocks = block;
    } else {
      block->next_free = lists->reusable_blocks;
      lists->reusable_blocks = block;
    }
  }
}

static void *scoop_gc_immix_parallel_region_sweep_worker(void *raw_job) {
  if (raw_job == 0) {
    return 0;
  }
  ScoopGcParallelSweepJob *job = (ScoopGcParallelSweepJob *)raw_job;
  if (job->blocks == 0) {
    return 0;
  }

  for (size_t i = job->start; i < job->end; i++) {
    scoop_gc_immix_region_sweep_one_block(job->blocks[i], &job->out);
  }

  return 0;
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

  // 与 minor collect 同理：若别的线程已发起 STW，本线程必须先作为 mutator 参与 safepoint，
  // 而不是在 GC 入口处被动等待，否则 initiator 会永远等不到它 park。
  if (scoop_gc_stw_requested_load(&scoop_gc_stw) && !pthread_equal(self, scoop_gc_stw.initiator)) {
    scoop_gc_immix_unlock(state);
    scoop_gc_safepoint_poll();
    return;
  }

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

  // T1409b：同样需要清空 thread-local block cache（head + len），否则缓存中可能持有
  // 已被 compaction/free 的 block 指针，导致 mutator 在 GC 结束后使用悬挂指针。
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    if (it->gc_alloc_block_cache_slot != 0) {
      *(it->gc_alloc_block_cache_slot) = 0;
    }
    if (it->gc_alloc_block_cache_len_slot != 0) {
      *(it->gc_alloc_block_cache_len_slot) = 0;
    }
  }

  ScoopGcHeap *heap = &scoop_gc_heap;
  uint32_t mark_value = scoop_gc_collect_next_mark_value(heap);
  ScoopGcHeapMembershipIndex membership = {0};
  (void)scoop_gc_heap_membership_index_build_unlocked(&membership, heap);

  // B2b：Immix roots 枚举不再扫描 shadow stack。
  //
  // 约定：
  // - InNative 线程：roots 来自 `native_roots` slots（enter_native 注册），以及 pinned/handles；
  // - 其余线程：roots 来自 stackmap（需要可用的 unwind ctx）。
  uint32_t initiator_needs_stackmap_roots = 1;
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    if (!pthread_equal(it->thread, self)) {
      continue;
    }
    if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
      initiator_needs_stackmap_roots = 0;
    }
    break;
  }

  void *initiator_stack_walking_ctx = 0;
  if (initiator_needs_stackmap_roots) {
    initiator_stack_walking_ctx = scoop_platform_unwind_ctx_capture();
    if (initiator_stack_walking_ctx == 0) {
      (void)fprintf(stderr, "[scooprt][gc][stackmap] failed to capture unwind ctx\n");
      abort();
    }
  }

  // 0) clear per-block mark bitmap（避免上一轮残留影响 region sweep）
  for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
    scoop_gc_immix_block_clear_mark_bits(it);
  }

  uint32_t did_parallel_mark = 0;
  uint32_t parallel_mark_workers = scoop_gc_immix_parallel_mark_worker_count();

  // 1) mark roots（仅走 stackmap/native/handle/pin；不再扫描 shadow stack）

  if (parallel_mark_workers > 1) {
    ScoopGcParallelMarkWork work = {0};
    if (scoop_gc_parallel_mark_work_init(&work)) {
      ScoopGcParallelMarkCtx ctx = {heap, mark_value, &work, &membership};

      for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
        // T1505c：InNative 线程 roots 来自 native_roots buffer。
        if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
          if (it->stack_walking_ctx == 0) {
            (void)fprintf(stderr,
                          "[scooprt][gc][stackmap] missing in-native ctx for mark roots "
                          "(thread=0x%" PRIxPTR ")\n",
                          (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
            abort();
          }

          (void)scoop_gc_native_roots_visit_slots(
              it->native_roots, it->native_roots_len, scoop_gc_parallel_mark_visitor, (void *)&ctx);
          {
            ScoopGcManagedRootMap root_map =
                scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
            ScoopGcRootMapVisitResult root_map_result = {0};
            (void)scoop_gc_root_map_visit_slots(
                &root_map, scoop_gc_parallel_mark_visitor, (void *)&ctx, &root_map_result);
            uint32_t err = root_map_result.visit_error;
            if (err != SCOOP_STACKMAP_VISIT_OK) {
              (void)fprintf(stderr,
                            "[scooprt][gc][stackmap] mark in-native caller roots failed: err=%u "
                            "(thread=0x%" PRIxPTR ")\n",
                            (unsigned)err,
                            (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
              abort();
            }
          }
          continue;
        }

        // B2b：initiator roots 同样必须来自 stackmap（覆盖完整 managed 栈）。
        if (initiator_needs_stackmap_roots && initiator_stack_walking_ctx != 0 &&
            pthread_equal(it->thread, self)) {
          ScoopGcManagedRootMap root_map =
              scoop_gc_managed_root_map_from_stackmap_ctx(initiator_stack_walking_ctx);
          ScoopGcRootMapVisitResult root_map_result = {0};
          (void)scoop_gc_root_map_visit_slots(
              &root_map, scoop_gc_parallel_mark_visitor, (void *)&ctx, &root_map_result);
          uint32_t err = root_map_result.visit_error;
          if (err != SCOOP_STACKMAP_VISIT_OK) {
            (void)fprintf(stderr,
                          "[scooprt][gc][stackmap] visit initiator roots failed: err=%u\n",
                          (unsigned)err);
            abort();
          }
          continue;
        }

        // T1506b：Parked 线程 stack walking ctx + stackmap roots。
        if (it->state == SCOOP_GC_THREAD_PARKED) {
          // 若该线程通过 `scoop_gc_safepoint()`（非 poll）进入 Parked，则可能没有 ctx；
          // 此时无法 walk stackmap roots，只能退化为“该线程无 stackmap roots 可枚举”。
          if (it->stack_walking_ctx != 0) {
            ScoopGcManagedRootMap root_map =
                scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
            ScoopGcRootMapVisitResult root_map_result = {0};
            (void)scoop_gc_root_map_visit_slots(
                &root_map, scoop_gc_parallel_mark_visitor, (void *)&ctx, &root_map_result);
            uint32_t err = root_map_result.visit_error;
            if (err != SCOOP_STACKMAP_VISIT_OK) {
              (void)fprintf(
                  stderr,
                  "[scooprt][gc][stackmap] mark roots failed: err=%u (thread=0x%" PRIxPTR ")\n",
                  (unsigned)err,
                  (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
              abort();
            }
          }
          continue;
        }

        // STW 已达成：除 initiator 与 InNative 线程外，其余线程必须为 Parked。
        (void)fprintf(stderr,
                      "[scooprt][gc] unexpected thread state during mark roots: state=%u "
                      "(thread=0x%" PRIxPTR ")\n",
                      (unsigned)it->state,
                      (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
        abort();
      }

      // 1b) mark pinned roots（spec §15.10）
      for (ScoopGcPinnedRecord *it = scoop_gc_pinned_objects; it != 0; it = it->next) {
        if (it->object == 0) {
          continue;
        }
        if (it->pin_count == 0) {
          continue;
        }
        scoop_gc_parallel_mark_object_if_needed(&ctx, it->object);
      }

      // 1c) mark stable handles（spec §15.10.1）
      for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
        if (it->object == 0) {
          continue;
        }
        scoop_gc_parallel_mark_object_if_needed(&ctx, it->object);
      }
      (void)scoop_gc_global_roots_visit_unlocked(scoop_gc_parallel_mark_visitor, (void *)&ctx);

      uint64_t in_flight = __atomic_load_n(&work.in_flight, __ATOMIC_ACQUIRE);
      if (in_flight > 0) {
        size_t helper_count = (size_t)parallel_mark_workers - 1u;
        pthread_t *threads = 0;
        if (helper_count > 0) {
          if (helper_count <= (SIZE_MAX / sizeof(pthread_t))) {
            threads = (pthread_t *)malloc(helper_count * sizeof(pthread_t));
          }
        }

        size_t started = 0;
        if (threads != 0) {
          for (size_t i = 0; i < helper_count; i++) {
            if (pthread_create(&threads[i], 0, scoop_gc_parallel_mark_worker, (void *)&ctx) != 0) {
              break;
            }
            started += 1;
          }
        }

        // 作为 worker0 在当前线程参与标记。
        (void)scoop_gc_parallel_mark_worker((void *)&ctx);

        for (size_t i = 0; i < started; i++) {
          (void)pthread_join(threads[i], 0);
        }
        if (threads != 0) {
          free(threads);
        }
      }

      scoop_gc_parallel_mark_work_destroy(&work);
      did_parallel_mark = 1;
    }
  }

  if (!did_parallel_mark) {
    ScoopGcMarkStack stack = {0};
    ScoopGcMarkCtx ctx = {heap, mark_value, &stack, &membership};

    for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
      // T1505c：InNative 线程 roots 来自 native_roots buffer。
      if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
        if (it->stack_walking_ctx == 0) {
          (void)fprintf(stderr,
                        "[scooprt][gc][stackmap] missing in-native ctx for mark roots "
                        "(thread=0x%" PRIxPTR ")\n",
                        (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
          abort();
        }

        (void)scoop_gc_native_roots_visit_slots(
            it->native_roots, it->native_roots_len, scoop_gc_mark_visitor, (void *)&ctx);
        {
          ScoopGcManagedRootMap root_map =
              scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
          ScoopGcRootMapVisitResult root_map_result = {0};
          (void)scoop_gc_root_map_visit_slots(
              &root_map, scoop_gc_mark_visitor, (void *)&ctx, &root_map_result);
          uint32_t err = root_map_result.visit_error;
          if (err != SCOOP_STACKMAP_VISIT_OK) {
            (void)fprintf(stderr,
                          "[scooprt][gc][stackmap] mark in-native caller roots failed: err=%u "
                          "(thread=0x%" PRIxPTR ")\n",
                          (unsigned)err,
                          (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
            abort();
          }
        }
        continue;
      }

      // B2b：initiator roots 同样必须来自 stackmap（覆盖完整 managed 栈）。
    if (initiator_needs_stackmap_roots && initiator_stack_walking_ctx != 0 &&
        pthread_equal(it->thread, self)) {
      ScoopGcManagedRootMap root_map =
          scoop_gc_managed_root_map_from_stackmap_ctx(initiator_stack_walking_ctx);
      ScoopGcRootMapVisitResult root_map_result = {0};
      (void)scoop_gc_root_map_visit_slots(
          &root_map, scoop_gc_mark_visitor, (void *)&ctx, &root_map_result);
      uint32_t err = root_map_result.visit_error;
      if (err != SCOOP_STACKMAP_VISIT_OK) {
        (void)fprintf(stderr,
                      "[scooprt][gc][stackmap] visit initiator roots failed: err=%u\n",
                        (unsigned)err);
          abort();
        }
        continue;
      }

      // T1506b：Parked 线程 stack walking ctx + stackmap roots。
    if (it->state == SCOOP_GC_THREAD_PARKED) {
      // 若该线程通过 `scoop_gc_safepoint()`（非 poll）进入 Parked，则可能没有 ctx；
      // 此时无法 walk stackmap roots，只能退化为“该线程无 stackmap roots 可枚举”。
      if (it->stack_walking_ctx != 0) {
        ScoopGcManagedRootMap root_map =
            scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
        ScoopGcRootMapVisitResult root_map_result = {0};
        (void)scoop_gc_root_map_visit_slots(
            &root_map, scoop_gc_mark_visitor, (void *)&ctx, &root_map_result);
        uint32_t err = root_map_result.visit_error;
        if (err != SCOOP_STACKMAP_VISIT_OK) {
          (void)fprintf(stderr,
                        "[scooprt][gc][stackmap] mark roots failed: err=%u (thread=0x%" PRIxPTR
                          ")\n",
                          (unsigned)err,
                          (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
            abort();
          }
        }
        continue;
      }

      (void)fprintf(stderr,
                    "[scooprt][gc] unexpected thread state during mark roots: state=%u "
                    "(thread=0x%" PRIxPTR ")\n",
                    (unsigned)it->state,
                    (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
      abort();
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

    // 1c) mark stable handles（spec §15.10.1）
    for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
      if (it->object == 0) {
        continue;
      }
      scoop_gc_mark_object_if_needed(&ctx, it->object);
    }

    (void)scoop_gc_global_roots_visit_unlocked(scoop_gc_mark_visitor, (void *)&ctx);

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
  }

  scoop_gc_heap_membership_index_destroy(&membership);

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

    heap->bytes_freed += obj->size_bytes;

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
  state->nursery_free_blocks = 0;
  state->nursery_current_block = 0;
  ScoopGcImmixBlock *evac_blocks = 0;

  uint32_t parallel_sweep_workers = scoop_gc_immix_parallel_sweep_worker_count();
  uint32_t did_parallel_sweep = 0;

  if (parallel_sweep_workers > 1) {
    // snapshot blocks：避免并行 worker 读取 next_all 链表时引入多处依赖。
    size_t block_count = 0;
    for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
      block_count += 1;
    }

    if (block_count > 1) {
      size_t worker_count = (size_t)parallel_sweep_workers;
      if (worker_count > block_count) {
        worker_count = block_count;
      }
      if (worker_count > 32) {
        worker_count = 32;
      }

      ScoopGcImmixBlock **blocks = 0;
      if (block_count <= (SIZE_MAX / sizeof(ScoopGcImmixBlock *))) {
        blocks = (ScoopGcImmixBlock **)malloc(block_count * sizeof(ScoopGcImmixBlock *));
      }

      if (blocks != 0) {
        size_t idx = 0;
        for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
          if (idx >= block_count) {
            break;
          }
          blocks[idx] = it;
          idx += 1;
        }

        ScoopGcParallelSweepJob jobs[32];
        (void)memset(jobs, 0, sizeof(jobs));
        pthread_t threads[32];
        uint8_t started[32];
        (void)memset(threads, 0, sizeof(threads));
        (void)memset(started, 0, sizeof(started));

        size_t chunk = (block_count + worker_count - 1u) / worker_count;
        size_t start = 0;
        for (size_t w = 0; w < worker_count; w++) {
          size_t end = start + chunk;
          if (end > block_count) {
            end = block_count;
          }
          jobs[w].blocks = blocks;
          jobs[w].start = start;
          jobs[w].end = end;
          start = end;
        }

        // helper threads：当前线程作为 worker0 参与 sweep，降低开销。
        for (size_t w = 1; w < worker_count; w++) {
          if (pthread_create(&threads[w], 0, scoop_gc_immix_parallel_region_sweep_worker, &jobs[w]) ==
              0) {
            started[w] = 1;
          }
        }
        (void)scoop_gc_immix_parallel_region_sweep_worker(&jobs[0]);

        for (size_t w = 1; w < worker_count; w++) {
          if (started[w]) {
            (void)pthread_join(threads[w], 0);
          } else {
            // 创建失败：退化为当前线程执行该分片，保证不漏扫。
            (void)scoop_gc_immix_parallel_region_sweep_worker(&jobs[w]);
          }
        }

        for (size_t w = 0; w < worker_count; w++) {
          scoop_gc_immix_region_sweep_merge_list(&state->free_blocks, jobs[w].out.free_blocks);
          scoop_gc_immix_region_sweep_merge_list(&state->reusable_blocks,
                                                 jobs[w].out.reusable_blocks);
          scoop_gc_immix_region_sweep_merge_list(&state->nursery_free_blocks,
                                                 jobs[w].out.nursery_free_blocks);
          scoop_gc_immix_region_sweep_merge_list(&evac_blocks, jobs[w].out.evac_blocks);
        }

        did_parallel_sweep = 1;
        free(blocks);
      }
    }
  }

  if (!did_parallel_sweep) {
    ScoopGcParallelSweepLists lists = {0};
    for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
      scoop_gc_immix_region_sweep_one_block(it, &lists);
    }
    state->free_blocks = lists.free_blocks;
    state->reusable_blocks = lists.reusable_blocks;
    state->nursery_free_blocks = lists.nursery_free_blocks;
    evac_blocks = lists.evac_blocks;
  }

  // 5) moving/compaction：对候选 blocks 做 evacuation，并更新 roots 与 heap 引用槽位。
  if (evac_blocks != 0) {
    scoop_gc_immix_compact(state, heap, evac_blocks, self, initiator_stack_walking_ctx);
  }

  if (scoop_gc_verify_roots_enabled()) {
    scoop_gc_verify_roots_after_gc_unlocked(heap, self, initiator_stack_walking_ctx);
  }

  if (initiator_stack_walking_ctx != 0) {
    scoop_platform_unwind_ctx_destroy(initiator_stack_walking_ctx);
    initiator_stack_walking_ctx = 0;
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

      uint64_t size = obj->size_bytes;
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
