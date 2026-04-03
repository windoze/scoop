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
#include "scoop_stackmap.h"
#include "scoop_tls_internal.h"

#include <errno.h>
#include <limits.h>
#include <pthread.h>
#include <sched.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

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

typedef struct ScoopGcStackmapRootsVisitCtx {
  ScoopGcTraceVisitor visitor;
  void *visitor_ctx;

  uint64_t slots_visited;
  uint32_t records_hit;
  uint32_t visit_error;
} ScoopGcStackmapRootsVisitCtx;

static uint32_t scoop_gc_stackmap_roots_frame_visitor(uintptr_t sp, uintptr_t ra, void *user_data) {
  if (user_data == 0) {
    return 0;
  }

  ScoopGcStackmapRootsVisitCtx *ctx = (ScoopGcStackmapRootsVisitCtx *)user_data;
  if (ctx->visit_error != SCOOP_STACKMAP_VISIT_OK) {
    return 0;
  }

  ScoopStackmapRecordRef rec = {0};
  if (!scoop_stackmap_registry_lookup(ra, &rec)) {
    // 约定（T1506b）：先跳过 runtime/系统帧，直到首次命中 record；随后一旦未命中 record，
    // 视为“离开 managed frames”，停止继续 walk（避免对 pthread/libc 等帧 fail-fast）。
    if (ctx->records_hit > 0) {
      return 0;
    }
    return 1;
  }

  ctx->records_hit += 1;

  uint32_t visit_err = SCOOP_STACKMAP_VISIT_OK;
  ctx->slots_visited += scoop_stackmap_record_visit_root_slots(
      &rec, sp, ctx->visitor, ctx->visitor_ctx, &visit_err);

  if (visit_err != SCOOP_STACKMAP_VISIT_OK) {
    ctx->visit_error = visit_err;
    return 0;
  }

  return 1;
}

static uint64_t scoop_gc_stackmap_visit_roots_from_ctx(void *stack_walking_ctx,
                                                       ScoopGcTraceVisitor visitor,
                                                       void *ctx,
                                                       uint32_t *out_error,
                                                       uint32_t *out_records_hit) {
  if (out_error != 0) {
    *out_error = SCOOP_STACKMAP_VISIT_OK;
  }
  if (out_records_hit != 0) {
    *out_records_hit = 0;
  }

  if (stack_walking_ctx == 0 || visitor == 0) {
    if (out_error != 0) {
      *out_error = SCOOP_STACKMAP_VISIT_ERR_INVALID_ARGUMENT;
    }
    return 0;
  }

  ScoopGcStackmapRootsVisitCtx walk = {
      .visitor = visitor,
      .visitor_ctx = ctx,
      .slots_visited = 0,
      .records_hit = 0,
      .visit_error = SCOOP_STACKMAP_VISIT_OK,
  };

  const uint32_t skip_frames = 0;
  (void)scoop_platform_unwind_ctx_walk_frames(
      stack_walking_ctx, skip_frames, scoop_gc_stackmap_roots_frame_visitor, (void *)&walk);

  if (out_error != 0) {
    *out_error = walk.visit_error;
  }
  if (out_records_hit != 0) {
    *out_records_hit = walk.records_hit;
  }
  return walk.slots_visited;
}

static void scoop_gc_stop_the_world_begin_unlocked(pthread_t initiator) {
  scoop_gc_stw_requested_store(&scoop_gc_stw, 1);
  scoop_gc_stw.initiator = initiator;
  scoop_gc_stw.epoch += 1;
  scoop_gc_stw.parked_count = 0;

  // 重置线程状态，避免上一轮残留（健壮性；对齐未来 T1505 的状态机语义）。
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    // T1505c：保留 InNative 线程状态；否则 GC 会错误等待其 park，导致死锁。
    if (it->state != SCOOP_GC_THREAD_IN_NATIVE) {
      it->state = SCOOP_GC_THREAD_RUNNING;
    }
    it->parked_epoch = 0;
    // 释放上一轮残留的 ctx（按协议 STW end 会清空；这里做防御式兜底）。
    scoop_platform_unwind_ctx_destroy(it->stack_walking_ctx);
    it->stack_walking_ctx = 0;
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
    // T1505c：InNative 状态由 `leave_native()` 显式恢复，不在 STW end 中强制切回 Running。
    if (it->state == SCOOP_GC_THREAD_PARKED) {
      it->state = SCOOP_GC_THREAD_RUNNING;
    }
    it->parked_epoch = 0;
    // T1505b：STW 结束后清空 stack walking ctx，避免悬挂指针或误用旧 ctx。
    scoop_platform_unwind_ctx_destroy(it->stack_walking_ctx);
    it->stack_walking_ctx = 0;
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

static uint32_t scoop_test_gc_unwind_frame_visitor(uintptr_t sp, uintptr_t ra, void *user_data) {
  (void)ra;
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

static uint32_t scoop_test_gc_stackmap_find_frame_visitor(uintptr_t sp, uintptr_t ra, void *ctx) {
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
  // - record 含 1 个 Direct location，指向 worker stack slot（root）。
  const size_t record_size = 40;
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
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 14, 1); // num_locations

  // Location（12 bytes）
  // StackMap v3 location encoding: 2 = Direct (see `runtime/c/scoop_stackmap.c`).
  section[rec_off + 16] = 2u;
  section[rec_off + 17] = 0;
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 18, (uint16_t)sizeof(void *));
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 20, sp_reg);
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 22, 0);
  scoop_test_gc_stackmap_write_i32_le(section, rec_off + 24, (int32_t)slot_off);
  // padding to 8
  // num_live_outs + reserved
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 32, 0);
  scoop_test_gc_stackmap_write_u16_le(section, rec_off + 34, 0);
  // tail padding already zeroed

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
  (void)scoop_gc_stackmap_visit_roots_from_ctx(worker_rec->stack_walking_ctx,
                                               scoop_test_gc_stackmap_roots_count_visitor,
                                               (void *)&slot_visits,
                                               &visit_err,
                                               &records_hit);
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
    existing->gc_alloc_block_cache_slot =
        scoop_tls_gc_immix_block_cache_slot_from_current_frame_slot(current_frame_slot);
    existing->gc_alloc_block_cache_len_slot =
        scoop_tls_gc_immix_block_cache_len_slot_from_current_frame_slot(current_frame_slot);
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
  rec->gc_alloc_block_cache_slot =
      scoop_tls_gc_immix_block_cache_slot_from_current_frame_slot(current_frame_slot);
  rec->gc_alloc_block_cache_len_slot =
      scoop_tls_gc_immix_block_cache_len_slot_from_current_frame_slot(current_frame_slot);
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

  // 若当前正处于 stop-the-world，则 enter_native 必须先参与本轮 STW（park），否则 GC 可能会等待该线程
  // 进入 safepoint 而永远等不到（deadlock）。
  while (scoop_gc_stw_requested_load(&scoop_gc_stw) && !pthread_equal(self, scoop_gc_stw.initiator)) {
    rec->last_safepoint_epoch = scoop_gc_stw.epoch;

    if (rec->parked_epoch != scoop_gc_stw.epoch) {
      rec->state = SCOOP_GC_THREAD_PARKED;
      rec->parked_epoch = scoop_gc_stw.epoch;
      scoop_gc_stw.parked_count += 1;
      (void)pthread_cond_broadcast(&scoop_gc_stw_cond);
    }

    (void)pthread_cond_wait(&scoop_gc_stw_cond, &state->lock);
  }

  // TLS：保存 native roots buffer（供后续 stackmap roots/handle 协议扩展）。
  ScoopThreadTls *tls = scoop_tls_from_gc_current_frame_slot(rec->current_frame_slot);
  if (tls != 0) {
    tls->gc_native_roots = (void *)root_slots;
    tls->gc_native_roots_len = root_slots_len;
  }

  rec->native_roots = (void *)root_slots;
  rec->native_roots_len = root_slots_len;
  rec->state = SCOOP_GC_THREAD_IN_NATIVE;
  rec->parked_epoch = 0;
  rec->last_safepoint_epoch = scoop_gc_stw.epoch;

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

  ScoopThreadTls *tls = scoop_tls_from_gc_current_frame_slot(rec->current_frame_slot);
  if (tls != 0) {
    tls->gc_native_roots = 0;
    tls->gc_native_roots_len = 0;
  }

  rec->native_roots = 0;
  rec->native_roots_len = 0;
  rec->state = SCOOP_GC_THREAD_RUNNING;
  rec->parked_epoch = 0;

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
  scoop_gc_heap_bytes_allocated_add(obj->size_bytes);
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

  scoop_gc_mark_object_if_needed(ctx, (ScoopGcObjectHeader *)raw);
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

  scoop_gc_parallel_mark_object_if_needed(ctx, (ScoopGcObjectHeader *)raw);
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

  // 3a) roots update：shadow stack slots 原地改写为新地址（moving GC 的关键语义）。
  //
  // 注意：必须更新“所有已注册线程”的 roots；否则在多线程 + moving/compaction 下会产生悬挂指针。
  uint64_t scoop_gc_shadow_stack_visit_roots_from_frame(ScoopGcFrame *frame,
                                                        ScoopGcTraceVisitor visitor,
                                                        void *ctx);
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    // T1505c：InNative 线程 roots 来自 native_roots buffer（同样需要在 moving GC 中被更新）。
    if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
      (void)scoop_gc_native_roots_visit_slots(
          it->native_roots, it->native_roots_len, scoop_gc_immix_update_slot_visitor, 0);
      continue;
    }

    // T1506b：Parked 线程若提供了 stack_walking_ctx，则优先走 stackmap spill slots 更新；
    // 若未提供或未命中 record，则回退到 shadow stack（保持早期 runtime/Rust 测试兼容）。
    if (it->state == SCOOP_GC_THREAD_PARKED && it->stack_walking_ctx != 0) {
      uint32_t err = SCOOP_STACKMAP_VISIT_OK;
      uint32_t records_hit = 0;
      (void)scoop_gc_stackmap_visit_roots_from_ctx(it->stack_walking_ctx,
                                                   scoop_gc_immix_update_slot_visitor,
                                                   0,
                                                   &err,
                                                   &records_hit);
      if (err != SCOOP_STACKMAP_VISIT_OK) {
        (void)fprintf(stderr,
                      "[scooprt][gc][stackmap] update roots failed: err=%u (thread=0x%" PRIxPTR
                      ")\n",
                      (unsigned)err,
                      (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
        abort();
      }

      if (records_hit > 0) {
        continue;
      }
    }

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

  // 0) clear per-block mark bitmap（避免上一轮残留影响 region sweep）
  for (ScoopGcImmixBlock *it = state->all_blocks; it != 0; it = it->next_all) {
    scoop_gc_immix_block_clear_mark_bits(it);
  }

  uint32_t did_parallel_mark = 0;
  uint32_t parallel_mark_workers = scoop_gc_immix_parallel_mark_worker_count();

  // 1) mark roots（扫描所有已注册线程的 shadow stack）
  uint64_t scoop_gc_shadow_stack_visit_roots_from_frame(ScoopGcFrame *frame,
                                                        ScoopGcTraceVisitor visitor,
                                                        void *ctx);

  if (parallel_mark_workers > 1) {
    ScoopGcParallelMarkWork work = {0};
    if (scoop_gc_parallel_mark_work_init(&work)) {
      ScoopGcParallelMarkCtx ctx = {heap, mark_value, &work};

      for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
        // T1505c：InNative 线程 roots 来自 native_roots buffer。
        if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
          (void)scoop_gc_native_roots_visit_slots(
              it->native_roots, it->native_roots_len, scoop_gc_parallel_mark_visitor, (void *)&ctx);
          continue;
        }

        // T1506b：Parked 线程若提供了 stack_walking_ctx，则优先走 stackmap roots；
        // 若未提供或未命中 record，则回退到 shadow stack（保持早期 runtime/Rust 测试兼容）。
        if (it->state == SCOOP_GC_THREAD_PARKED && it->stack_walking_ctx != 0) {
          uint32_t err = SCOOP_STACKMAP_VISIT_OK;
          uint32_t records_hit = 0;
          (void)scoop_gc_stackmap_visit_roots_from_ctx(it->stack_walking_ctx,
                                                       scoop_gc_parallel_mark_visitor,
                                                       (void *)&ctx,
                                                       &err,
                                                       &records_hit);
          if (err != SCOOP_STACKMAP_VISIT_OK) {
            (void)fprintf(
                stderr,
                "[scooprt][gc][stackmap] mark roots failed: err=%u (thread=0x%" PRIxPTR ")\n",
                (unsigned)err,
                (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
            abort();
          }
          if (records_hit > 0) {
            continue;
          }
        }

        if (it->current_frame_slot == 0) {
          continue;
        }
        ScoopGcFrame *frame = *(it->current_frame_slot);
        (void)scoop_gc_shadow_stack_visit_roots_from_frame(frame,
                                                           scoop_gc_parallel_mark_visitor,
                                                           (void *)&ctx);
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
    ScoopGcMarkCtx ctx = {heap, mark_value, &stack};

    for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
      // T1505c：InNative 线程 roots 来自 native_roots buffer。
      if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
        (void)scoop_gc_native_roots_visit_slots(
            it->native_roots, it->native_roots_len, scoop_gc_mark_visitor, (void *)&ctx);
        continue;
      }

      // T1506b：Parked 线程若提供了 stack_walking_ctx，则优先走 stackmap roots；
      // 若未提供或未命中 record，则回退到 shadow stack（保持早期 runtime/Rust 测试兼容）。
      if (it->state == SCOOP_GC_THREAD_PARKED && it->stack_walking_ctx != 0) {
        uint32_t err = SCOOP_STACKMAP_VISIT_OK;
        uint32_t records_hit = 0;
        (void)scoop_gc_stackmap_visit_roots_from_ctx(
            it->stack_walking_ctx, scoop_gc_mark_visitor, (void *)&ctx, &err, &records_hit);
        if (err != SCOOP_STACKMAP_VISIT_OK) {
          (void)fprintf(
              stderr,
              "[scooprt][gc][stackmap] mark roots failed: err=%u (thread=0x%" PRIxPTR ")\n",
              (unsigned)err,
              (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
          abort();
        }
        if (records_hit > 0) {
          continue;
        }
      }

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
    evac_blocks = lists.evac_blocks;
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
