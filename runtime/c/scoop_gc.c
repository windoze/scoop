// Scoop GC runtime (early stage).
//
// TODO T0904：mark-sweep GC 的数据结构骨架。
// TODO T0910：实现最小可用的单线程 mark-sweep（手动触发）。

#include "scoop_gc_backend.h"

#if SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_BASELINE

#include "scoop_gc.h"

#include <pthread.h>
#include <sched.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>

#include "scoop_gc_root_map_internal.h"
#include "platform/unwind.h"
#include "scoop_gc_stw_internal.h"
#include "scoop_stackmap.h"
#include "scoop_tls_internal.h"

// --- 线程注册 + stop-the-world（TODO T0911） ---
//
// 设计说明（early stage, GC-FIX Phase B2a baseline）：
// - baseline backend 的 GC roots 不再来自 shadow stack（`ScoopGcFrame`）；
// - roots 仅来自：
//   - Parked 线程：stackmap roots（通过 `scoop_gc_safepoint_poll()` park 前捕获的 unwind ctx）；
//   - InNative 线程：enter_native 注册的 `native_roots` slots；
//   - pinned objects / stable handles（进程全局 roots）。
// - 该实现采用“协作式 STW”：线程必须在 safepoint 调用 `scoop_gc_safepoint(_poll)` 才会被暂停；
//   后续编译器会在需要的位置插入 poll（例如分配/循环回边等）。
//
// 约束：
// - 该实现优先满足“可验证且不崩溃”的语义，不追求性能。
// - 线程必须显式调用 `scoop_thread_register/unregister`（由 runtime 侧提供）以参与 GC STW。

static pthread_mutex_t scoop_gc_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t scoop_gc_cond = PTHREAD_COND_INITIALIZER;

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

// runtime 侧在 `scoop_thread_register/unregister` 中调用这些函数，把线程纳入 GC 的 STW 范围。
void scoop_gc_thread_register(ScoopThreadTls *tls) {
  if (tls == 0) {
    return;
  }

  pthread_t self = pthread_self();

  (void)pthread_mutex_lock(&scoop_gc_lock);

  ScoopGcThreadRecord *existing = scoop_gc_find_thread_unlocked(self);
  if (existing != 0) {
    existing->tls = tls;
    existing->state = SCOOP_GC_THREAD_RUNNING;
    existing->last_safepoint_epoch = scoop_gc_stw.epoch;
    existing->parked_epoch = 0;
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
  rec->tls = tls;
  rec->gc_alloc_block_slot = 0;
  rec->gc_alloc_block_cache_slot = 0;
  rec->gc_alloc_block_cache_len_slot = 0;
  rec->state = SCOOP_GC_THREAD_RUNNING;
  rec->last_safepoint_epoch = scoop_gc_stw.epoch;
  rec->parked_epoch = 0;
  rec->stack_walking_ctx = 0;
  rec->native_roots = 0;
  rec->native_roots_len = 0;

  scoop_gc_threads = rec;
  scoop_gc_thread_count += 1;

  (void)pthread_mutex_unlock(&scoop_gc_lock);
}

void scoop_gc_thread_unregister(ScoopThreadTls *tls) {
  (void)tls;
  pthread_t self = pthread_self();

  (void)pthread_mutex_lock(&scoop_gc_lock);

  // 若当前有其它线程正在进行 STW，则等它结束后再注销，避免破坏 stop-the-world 计数。
  while (scoop_gc_stw_requested_load(&scoop_gc_stw) && !pthread_equal(self, scoop_gc_stw.initiator)) {
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
static void scoop_gc_safepoint_common(uint32_t capture_stack_walking_ctx) {
  // T1505a：fast path（无 STW 时不抢全局锁）。
  if (!scoop_gc_stw_requested_load(&scoop_gc_stw)) {
    return;
  }

  pthread_t self = pthread_self();

  (void)pthread_mutex_lock(&scoop_gc_lock);

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
      (void)pthread_cond_broadcast(&scoop_gc_cond);
    }

    (void)pthread_cond_wait(&scoop_gc_cond, &scoop_gc_lock);
  }

  (void)pthread_mutex_unlock(&scoop_gc_lock);
}

void scoop_gc_safepoint(void) { scoop_gc_safepoint_common(/*capture_stack_walking_ctx=*/0); }

void scoop_gc_safepoint_poll(void) {
  // T1505b：把“park 前捕获 stack walking ctx”的新语义落在 poll 上，避免扩大历史 ABI 的语义漂移。
  scoop_gc_safepoint_common(/*capture_stack_walking_ctx=*/1);
}

void *scoop_gc_write_barrier(void *slot_addr, void *value) {
  if (slot_addr == 0) {
    return value;
  }

  // baseline backend：无 nursery/minor 语义，写屏障退化为“写入 slot”本身。
  (void)memcpy(slot_addr, &value, sizeof(void *));
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

  (void)pthread_mutex_lock(&scoop_gc_lock);

  ScoopGcThreadRecord *rec = scoop_gc_find_thread_unlocked(self);
  if (rec == 0) {
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return;
  }

  // T1512c：InNative 线程除了当前 call-site 的 `native_roots` 外，还必须保留 enter_native
  // 边界之上的 managed caller frames；否则 GC 会看不到跨 wait/join 等 native 调用继续存活的上层 roots。
  scoop_platform_unwind_ctx_destroy(rec->stack_walking_ctx);
  rec->stack_walking_ctx = scoop_platform_unwind_ctx_capture();
  if (rec->stack_walking_ctx == 0) {
    (void)pthread_mutex_unlock(&scoop_gc_lock);
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

  if (scoop_gc_stw_requested_load(&scoop_gc_stw) && !pthread_equal(self, scoop_gc_stw.initiator)) {
    if (rec->parked_epoch != scoop_gc_stw.epoch) {
      rec->parked_epoch = scoop_gc_stw.epoch;
      scoop_gc_stw.parked_count += 1;
      (void)pthread_cond_broadcast(&scoop_gc_cond);
    }
  } else {
    rec->parked_epoch = 0;
  }

  (void)pthread_mutex_unlock(&scoop_gc_lock);
}

void scoop_leave_native(void) {
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  pthread_t self = pthread_self();

  (void)pthread_mutex_lock(&scoop_gc_lock);

  ScoopGcThreadRecord *rec = scoop_gc_find_thread_unlocked(self);
  if (rec == 0) {
    (void)pthread_mutex_unlock(&scoop_gc_lock);
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
        (void)pthread_cond_broadcast(&scoop_gc_cond);
      }
    }

    (void)pthread_cond_wait(&scoop_gc_cond, &scoop_gc_lock);
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

  (void)pthread_mutex_unlock(&scoop_gc_lock);
}

// scope helper：进入 stop-the-world（等待其它线程 park）。
static void scoop_gc_stop_the_world_begin_unlocked(pthread_t initiator) {
  scoop_gc_stw_requested_store(&scoop_gc_stw, 1);
  scoop_gc_stw.initiator = initiator;
  scoop_gc_stw.epoch += 1;
  scoop_gc_stw.parked_count = 0;

  // 重置线程状态，避免上一轮残留（健壮性；对齐未来 T1505 的状态机语义）。
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    // T1505c：若线程处于 InNative，则它已是“可枚举 roots 的就绪态”，不能被重置为 Running；
    // 否则 GC 可能会错误等待它 park，导致死锁。
    if (it->state != SCOOP_GC_THREAD_IN_NATIVE) {
      it->state = SCOOP_GC_THREAD_RUNNING;
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

  while (scoop_gc_stw.parked_count < need_to_park) {
    struct timespec ts;
    scoop_gc_stw_timespec_after_ms((uint32_t)SCOOP_GC_STW_DIAG_INTERVAL_MS, &ts);

    int rc = pthread_cond_timedwait(&scoop_gc_cond, &scoop_gc_lock, &ts);
    if (rc == ETIMEDOUT) {
      scoop_gc_stw_diag_dump_threads_unlocked(&scoop_gc_stw, scoop_gc_threads, need_to_park);
    }
  }
}

static void scoop_gc_stop_the_world_end_unlocked(void) {
  scoop_gc_stw_requested_store(&scoop_gc_stw, 0);
  scoop_gc_stw.parked_count = 0;

  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    // T1505c：InNative 线程不会进入 park/wait，因此不能在 STW end 时被强制切回 Running；
    // 它的状态将由 `leave_native()` 显式恢复。
    if (it->state == SCOOP_GC_THREAD_PARKED) {
      it->state = SCOOP_GC_THREAD_RUNNING;
    }
    it->parked_epoch = 0;
    if (it->state != SCOOP_GC_THREAD_IN_NATIVE) {
      scoop_platform_unwind_ctx_destroy(it->stack_walking_ctx);
      it->stack_walking_ctx = 0;
    }
  }

  (void)pthread_cond_broadcast(&scoop_gc_cond);
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

  pthread_t self = pthread_self();
  (void)pthread_mutex_lock(&scoop_gc_lock);
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
  (void)pthread_mutex_unlock(&scoop_gc_lock);

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

  pthread_t self = pthread_self();
  (void)pthread_mutex_lock(&scoop_gc_lock);
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

  ScoopTestGcUnwindFramesState state = {
      .frame_count = 0,
      .query_count = 0,
      .sp_non_decreasing = 1,
      .last_sp = 0,
  };
  const uint32_t skip_frames = 0;
  const uint32_t visited = scoop_platform_unwind_ctx_walk_frames(
      worker_rec->stack_walking_ctx, skip_frames, scoop_test_gc_unwind_frame_visitor, (void *)&state);

  if (visited < 3 || state.frame_count < 3 || state.query_count < 3) {
    rc = -30;
    goto done;
  }
  if (!state.sp_non_decreasing) {
    rc = -31;
    goto done;
  }
  if (visited != state.frame_count || visited != state.query_count) {
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
  (void)pthread_mutex_unlock(&scoop_gc_lock);

  __atomic_store_n(&stop, 1, __ATOMIC_SEQ_CST);
  (void)pthread_join(worker, 0);
  scoop_thread_unregister();
  return rc;
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

// --- Stable handles（spec §15.10.1 / TODO T1510a） ---
//
// 说明：
// - handle 是把 heap 对象“以整数 token 形式”交给 native/外部系统的机制；
// - handle 表必须被 GC 当作 roots；moving/compaction 时还需要更新 handle->obj 槽位
//   （Immix backend 的实现见 `scoop_gc_backend_immix.c`）。
//
// v0 实现：
// - 用链表保存 handle records；
// - handle 值为 record 指针（`uintptr_t` cast），并通过线性扫描验证其合法性（避免对无效指针解引用）。
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

static uint32_t scoop_gc_heap_contains_object_in_heap_unlocked(ScoopGcHeap *heap,
                                                               ScoopGcObjectHeader *obj) {
  if (heap == 0 || obj == 0) {
    return 0;
  }

  for (ScoopGcObjectHeader *it = heap->objects; it != 0; it = it->next) {
    if (it == obj) {
      return 1;
    }
  }
  return 0;
}

static uint32_t scoop_gc_heap_contains_object_unlocked(ScoopGcObjectHeader *obj) {
  return scoop_gc_heap_contains_object_in_heap_unlocked(&scoop_gc_heap, obj);
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

uint64_t scoop_handle_new(void *raw_obj) {
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

  // 健壮性：只允许为 heap 内对象创建 handle，避免 GC 扫描 handle roots 时对非法指针解引用。
  if (!scoop_gc_heap_contains_object_unlocked(obj)) {
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return 0;
  }

  ScoopGcHandleRecord *rec = (ScoopGcHandleRecord *)malloc(sizeof(ScoopGcHandleRecord));
  if (rec == 0) {
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return 0;
  }

  rec->next = scoop_gc_handle_records;
  rec->object = obj;
  scoop_gc_handle_records = rec;

  uint64_t handle = (uint64_t)(uintptr_t)rec;
  (void)pthread_mutex_unlock(&scoop_gc_lock);
  return handle;
}

void *scoop_handle_get(uint64_t handle) {
  if (handle == 0) {
    return 0;
  }

  // 说明：保持与其它 runtime API 一致：允许在未显式 init/register 的情况下被调用。
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  scoop_runtime_init();
  scoop_thread_register();

  ScoopGcHandleRecord *needle = (ScoopGcHandleRecord *)(uintptr_t)handle;

  (void)pthread_mutex_lock(&scoop_gc_lock);
  for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
    if (it != needle) {
      continue;
    }
    void *obj = (void *)it->object;
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return obj;
  }
  (void)pthread_mutex_unlock(&scoop_gc_lock);
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

  (void)pthread_mutex_lock(&scoop_gc_lock);

  ScoopGcHandleRecord **link = &scoop_gc_handle_records;
  while (*link != 0) {
    ScoopGcHandleRecord *it = *link;
    if (it != needle) {
      link = &it->next;
      continue;
    }

    *link = it->next;
    free(it);
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return 1;
  }

  (void)pthread_mutex_unlock(&scoop_gc_lock);
  return 0;
}

void scoop_gc_register_global_root(void *base, const ScoopTypeDescriptor *type_desc) {
  if (base == 0 || type_desc == 0) {
    return;
  }

  void scoop_runtime_init(void);
  scoop_runtime_init();

  (void)pthread_mutex_lock(&scoop_gc_lock);

  for (ScoopGcGlobalRootRecord *it = scoop_gc_global_roots; it != 0; it = it->next) {
    if (it->base != base) {
      continue;
    }
    it->type_desc = type_desc;
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return;
  }

  ScoopGcGlobalRootRecord *rec = (ScoopGcGlobalRootRecord *)malloc(sizeof(ScoopGcGlobalRootRecord));
  if (rec == 0) {
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    return;
  }

  rec->next = scoop_gc_global_roots;
  rec->base = base;
  rec->type_desc = type_desc;
  scoop_gc_global_roots = rec;

  (void)pthread_mutex_unlock(&scoop_gc_lock);
}

void scoop_gc_heap_register_object(ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return;
  }

  // 说明：heap 链表与统计字段是进程全局共享状态；在多线程下需加锁保护。
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

  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)raw;
  if (!scoop_gc_heap_contains_object_in_heap_unlocked(ctx->heap, obj)) {
    // 重要：stackmap records 里可能包含“刚好是 pointer-sized 但不是 GC roots”的 slot
    // （例如 call target / return address / deopt metadata 等）。
    // 为避免把这些值当作 heap 对象解引用并崩溃，这里做一次 membership 过滤。
    return;
  }

  scoop_gc_mark_object_if_needed(ctx, obj);
}

// --- Moving GC helpers（T1511；baseline backend 以 env 开关方式提供 “move + fixup”） ---
//
// 说明：
// - baseline backend 默认仍为 non-moving mark-sweep（保持既有行为与性能/诊断稳定）；
// - 当设置 env `SCOOP_GC_MOVE=1` 时，`scoop_gc_collect()` 会在 sweep 期间额外执行一次
//   “copy & fixup”：
//   - 为所有 live 且非 pinned 的对象分配新副本并写入 forwarding pointer；
//   - 更新：stackmap spill slots、native_roots、stable handles、以及 heap 内引用字段；
//   - 最后释放旧对象副本并重建 heap 链表。
//
// 该实现的目标是提供一个“可强制触发搬迁”的安全网，用于在启用 Immix compaction 前回归
// roots update 闭环（栈/堆/handle/native）。

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

static uint32_t scoop_gc_baseline_moving_enabled(void) {
  return scoop_gc_env_flag_is_truthy("SCOOP_GC_MOVE");
}

static uint32_t scoop_gc_verify_roots_enabled(void) {
  return scoop_gc_env_flag_is_truthy("SCOOP_GC_VERIFY_ROOTS");
}

// forwarding pointer：复用对象头的 `next` 字段，并用低位 tag 区分“链表 next”与“转发指针”。
#define SCOOP_GC_BASELINE_FORWARDING_TAG ((uintptr_t)1u)

static inline uint32_t scoop_gc_baseline_object_is_forwarded(const ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return 0;
  }
  return (((uintptr_t)obj->next) & SCOOP_GC_BASELINE_FORWARDING_TAG) != 0;
}

static inline ScoopGcObjectHeader *scoop_gc_baseline_object_forwarding_ptr(
    const ScoopGcObjectHeader *obj) {
  if (obj == 0) {
    return 0;
  }
  uintptr_t raw = (uintptr_t)obj->next;
  raw &= ~SCOOP_GC_BASELINE_FORWARDING_TAG;
  return (ScoopGcObjectHeader *)raw;
}

static inline void scoop_gc_baseline_object_set_forwarding_ptr(ScoopGcObjectHeader *obj,
                                                               ScoopGcObjectHeader *to) {
  if (obj == 0) {
    return;
  }
  obj->next = (ScoopGcObjectHeader *)(((uintptr_t)to) | SCOOP_GC_BASELINE_FORWARDING_TAG);
}

static inline ScoopGcObjectHeader *scoop_gc_baseline_follow_forwarding(ScoopGcObjectHeader *obj) {
  // 防御：限制 forwarding chain 长度，避免错误写入导致死循环。
  for (uint32_t hops = 0; hops < 8; hops++) {
    if (obj == 0) {
      return 0;
    }
    if (!scoop_gc_baseline_object_is_forwarded(obj)) {
      return obj;
    }
    obj = scoop_gc_baseline_object_forwarding_ptr(obj);
  }
  return obj;
}

typedef struct ScoopGcBaselineUpdateCtx {
  ScoopGcObjectHeader **live_sorted;
  size_t live_len;
} ScoopGcBaselineUpdateCtx;

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

static uint32_t scoop_gc_baseline_live_set_contains(const ScoopGcBaselineUpdateCtx *ctx,
                                                    ScoopGcObjectHeader *obj) {
  if (ctx == 0 || obj == 0 || ctx->live_sorted == 0 || ctx->live_len == 0) {
    return 0;
  }

  // `live_sorted` 是按地址排序的对象指针数组；用二分查找快速判定。
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

static void scoop_gc_baseline_update_slot_visitor(void **slot, void *raw_ctx) {
  if (slot == 0) {
    return;
  }
  ScoopGcObjectHeader *obj = (ScoopGcObjectHeader *)(*slot);
  if (obj == 0) {
    return;
  }

  const ScoopGcBaselineUpdateCtx *ctx = (const ScoopGcBaselineUpdateCtx *)raw_ctx;
  if (ctx != 0 && !scoop_gc_baseline_live_set_contains(ctx, obj)) {
    // 重要：stackmap spill slots 可能包含“pointer-sized 但不是 GC object pointer”的值。
    // roots update 阶段若直接解引用该指针读取 `obj->next` 会崩溃，因此只对 live 集合中的
    // 对象指针做 forwarding follow。
    return;
  }

  ScoopGcObjectHeader *updated = scoop_gc_baseline_follow_forwarding(obj);
  if (updated != 0 && updated != obj) {
    *slot = (void *)updated;
  }
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

// --- GC roots 强校验（GC-FIX Phase B2c） ---
//
// 目的：
// - 该模式用于诊断/回归：避免 “roots 枚举不全 / roots 更新不全” 导致的 silent mis-collection。
// - 通过 env `SCOOP_GC_VERIFY_ROOTS=1` 启用（slow path；不追求性能）。
//
// 校验内容（v0）：
// - GC 完成（sweep/move+fixup）后，再次枚举所有 roots slots（stackmap/native_roots/handles/pin）；
// - 要求：每个非 NULL roots 值必须指向当前 heap.objects 中的某个 live 对象（对象头地址）；
// - 对 stackmap roots：要求 stackmap lookup 至少命中 1 条 record（否则视为“未产生/未注册 stackmaps”）。
//
// 注意：
// - 该校验在 stop-the-world 期间运行（持有 GC 锁），因此可以安全读取 Parked 线程的 stack slots；
// - 对 InNative 线程：按当前协议仅验证 `native_roots`（不尝试 walk 其 managed frames）。

typedef struct ScoopGcVerifyRootsState {
  ScoopGcHeap *heap;
  ScoopGcBaselineUpdateCtx live_set;

  uint32_t errors;
  uint32_t max_errors;
} ScoopGcVerifyRootsState;

typedef struct ScoopGcVerifySlotCtx {
  ScoopGcVerifyRootsState *state;
  const char *kind;
  uintptr_t thread_id;
} ScoopGcVerifySlotCtx;

static uint32_t scoop_gc_verify_live_set_contains(const ScoopGcVerifyRootsState *st,
                                                  ScoopGcObjectHeader *obj) {
  if (st == 0 || obj == 0) {
    return 0;
  }
  if (st->live_set.live_sorted != 0 && st->live_set.live_len != 0) {
    return scoop_gc_baseline_live_set_contains(&st->live_set, obj);
  }
  // fallback：O(n)；避免因 OOM 无法分配 live_set 数组导致“全误报”。
  return scoop_gc_heap_contains_object_in_heap_unlocked(st->heap, obj);
}

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

    // 诊断辅助：判断该值是否“看起来像某个 GC 对象内部的 derived pointer”。
    //
    // 说明：
    // - stackmap roots 在 v0 约定下应当只包含对象头指针（`ScoopGcObjectHeader*`）；
    // - 若 value 指向某个对象的 payload/字段中间位置，说明 codegen/statepoint roots 里混入了
    //   derived pointer（常见于 `getelementptr` 结果跨越了 safepoint 并被当作 root 溢出到 spill slot）。
    if (st->heap != 0 && value != 0) {
      const uintptr_t addr = (uintptr_t)value;
      ScoopGcObjectHeader *container = 0;
      for (ScoopGcObjectHeader *it = st->heap->objects; it != 0; it = it->next) {
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
  if (!scoop_gc_verify_live_set_contains(ctx->state, obj)) {
    scoop_gc_verify_roots_record_error(
        ctx->state, ctx->kind, ctx->thread_id, (const void *)slot, (const void *)raw, "invalid root");
  }
}

static void scoop_gc_verify_roots_after_gc_unlocked(ScoopGcHeap *heap,
                                                    pthread_t initiator,
                                                    void *initiator_stack_walking_ctx) {
  if (heap == 0) {
    return;
  }

  // snapshot live set
  size_t live_len = 0;
  for (ScoopGcObjectHeader *it = heap->objects; it != 0; it = it->next) {
    live_len += 1;
  }

  ScoopGcObjectHeader **live_sorted = 0;
  if (live_len > 0 && live_len <= (SIZE_MAX / sizeof(ScoopGcObjectHeader *))) {
    live_sorted = (ScoopGcObjectHeader **)malloc(live_len * sizeof(ScoopGcObjectHeader *));
  }

  if (live_sorted != 0) {
    size_t idx = 0;
    for (ScoopGcObjectHeader *it = heap->objects; it != 0 && idx < live_len; it = it->next) {
      live_sorted[idx++] = it;
    }
    live_len = idx;
    qsort(live_sorted, live_len, sizeof(ScoopGcObjectHeader *), scoop_gc_ptr_cmp);
  } else {
    // fallback：不分配 live_sorted，后续 membership 判定退化为遍历 heap.objects。
    live_len = 0;
  }

  ScoopGcVerifyRootsState st = {
      .heap = heap,
      .live_set =
          {
              .live_sorted = live_sorted,
              .live_len = live_len,
          },
      .errors = 0,
      .max_errors = 16,
  };

  // pinned roots / stable handles 也必须指向 live 对象（否则 moving/compaction 后为悬挂指针）。
  for (ScoopGcPinnedRecord *it = scoop_gc_pinned_objects; it != 0; it = it->next) {
    if (it->object == 0 || it->pin_count == 0) {
      continue;
    }
    if (!scoop_gc_verify_live_set_contains(&st, it->object)) {
      scoop_gc_verify_roots_record_error(
          &st, "pin", /*thread_id=*/0, (const void *)&it->object, (const void *)it->object, "pinned root not live");
    }
  }
  for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
    if (it->object == 0) {
      continue;
    }
    if (!scoop_gc_verify_live_set_contains(&st, it->object)) {
      scoop_gc_verify_roots_record_error(&st,
                                         "handle",
                                         /*thread_id=*/0,
                                         (const void *)&it->object,
                                         (const void *)it->object,
                                         "handle root not live");
    }
  }
  {
    ScoopGcVerifySlotCtx v = {.state = &st, .kind = "global_root", .thread_id = 0};
    (void)scoop_gc_global_roots_visit_unlocked(scoop_gc_verify_root_slot_visitor, (void *)&v);
  }

  // per-thread roots
  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    const uintptr_t tid = scoop_gc_thread_id_for_diag(it->thread);

    // initiator：若捕获了 stack walking ctx，则同时校验 stackmap roots（moving GC 依赖 spill slots 更新）。
    if (pthread_equal(it->thread, initiator)) {
      if (initiator_stack_walking_ctx != 0) {
        ScoopGcVerifySlotCtx v = {.state = &st, .kind = "stackmap", .thread_id = tid};
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
          ScoopGcVerifySlotCtx v = {.state = &st, .kind = "stackmap", .thread_id = tid};
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
          }
        }

        ScoopGcVerifySlotCtx v = {.state = &st, .kind = "native_roots", .thread_id = tid};
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
        ScoopGcVerifySlotCtx v = {.state = &st, .kind = "stackmap", .thread_id = tid};
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
        }
      }

      ScoopGcVerifySlotCtx v = {.state = &st, .kind = "native_roots", .thread_id = tid};
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

      ScoopGcVerifySlotCtx v = {.state = &st, .kind = "stackmap", .thread_id = tid};
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
        scoop_gc_verify_roots_record_error(&st, "stackmap", tid, /*slot_addr=*/0, /*value=*/0, "stackmap hit 0 records");
      }
      continue;
    }

    // STW 已达成：除 initiator 与 InNative 线程外，其余线程必须为 Parked。
    scoop_gc_verify_roots_record_error(
        &st, "thread_state", tid, /*slot_addr=*/0, /*value=*/0, "unexpected thread state during verify");
  }

  if (st.errors != 0) {
    (void)fprintf(stderr,
                  "[scooprt][gc][verify-roots] found %u error(s); aborting\n",
                  (unsigned)st.errors);
    if (live_sorted != 0) {
      free(live_sorted);
    }
    abort();
  }

  if (live_sorted != 0) {
    free(live_sorted);
  }
}

typedef struct ScoopGcBaselineMoveRecord {
  ScoopGcObjectHeader *from;
  ScoopGcObjectHeader *to;
  uint64_t size;
} ScoopGcBaselineMoveRecord;

static uint32_t scoop_gc_baseline_object_is_pinned_unlocked(ScoopGcObjectHeader *obj) {
  ScoopGcPinnedRecord *rec = scoop_gc_find_pinned_unlocked(obj);
  if (rec == 0) {
    return 0;
  }
  return rec->pin_count > 0;
}

// baseline moving sweep（T1511）：
// - 释放 unreachable；
// - 对 live 且非 pinned 的对象分配新副本并写 forwarding；
// - 更新：stackmap spill slots/native_roots/handles/heap fields；
// - 重建 heap 链表并释放旧副本。
static void scoop_gc_collect_baseline_moving_unlocked(ScoopGcHeap *heap,
                                                      uint32_t mark_value,
                                                      pthread_t initiator,
                                                      void *initiator_stack_walking_ctx) {
  if (heap == 0) {
    return;
  }

  // 1) sweep unreachable + 收集 live 列表（避免后续写 forwarding pointer 破坏 heap 链表遍历）。
  size_t live_cap = 0;
  size_t live_len = 0;
  ScoopGcObjectHeader **live = 0;

  for (ScoopGcObjectHeader *obj = heap->objects; obj != 0;) {
    ScoopGcObjectHeader *next = obj->next;

    if (obj->mark != mark_value) {
      if (obj->type_desc != 0 && obj->type_desc->release_fn != 0) {
        obj->type_desc->release_fn((void *)obj);
      }

      heap->bytes_freed += obj->size_bytes;
      free(obj);
      obj = next;
      continue;
    }

    if (live_len >= live_cap) {
      size_t new_cap = (live_cap == 0) ? 128u : (live_cap * 2u);
      if (new_cap < live_len + 1u) {
        new_cap = live_len + 1u;
      }
      if (new_cap > (SIZE_MAX / sizeof(ScoopGcObjectHeader *))) {
        // OOM/overflow：退化为 non-moving sweep（保留 live 对象，但不搬迁）。
        break;
      }
      void *p = realloc(live, new_cap * sizeof(ScoopGcObjectHeader *));
      if (p == 0) {
        // OOM：同样退化为 non-moving sweep。
        break;
      }
      live = (ScoopGcObjectHeader **)p;
      live_cap = new_cap;
    }

    live[live_len] = obj;
    live_len += 1;
    obj = next;
  }

  // 若 live 数组分配失败（live==NULL 但 live_len>0），回退为“只做 sweep unreachable”。
  if (live_len > 0 && live == 0) {
    // 防御：重建 heap 链表（只包含当前仍存活的对象），避免 dangling next 指针。
    ScoopGcObjectHeader *new_list = 0;
    for (ScoopGcObjectHeader *it = heap->objects; it != 0; it = it->next) {
      if (it->mark != mark_value) {
        continue;
      }
      it->next = new_list;
      new_list = it;
    }
    heap->objects = new_list;
    return;
  }

  // 2) to-space 分配与拷贝（可回滚）：先分配完所有副本，确保不会出现“半搬迁”。
  size_t move_len = 0;
  ScoopGcBaselineMoveRecord *moves = 0;

  if (live_len > 0 && live_len <= (SIZE_MAX / sizeof(ScoopGcBaselineMoveRecord))) {
    moves = (ScoopGcBaselineMoveRecord *)malloc(live_len * sizeof(ScoopGcBaselineMoveRecord));
  }

  uint32_t move_failed = 0;
  if (moves == 0 && live_len > 0) {
    move_failed = 1;
  }

  if (!move_failed) {
    for (size_t i = 0; i < live_len; i++) {
      ScoopGcObjectHeader *from = live[i];
      if (from == 0) {
        continue;
      }
      if (scoop_gc_baseline_object_is_pinned_unlocked(from)) {
        continue;
      }

      uint64_t raw_size = from->size_bytes;
      if (raw_size == 0 || raw_size > (uint64_t)SIZE_MAX) {
        // 健壮性：无法搬迁的对象直接跳过（仍可被 roots/heap 扫描）。
        continue;
      }

      void *p = malloc((size_t)raw_size);
      if (p == 0) {
        move_failed = 1;
        break;
      }

      (void)memcpy(p, (const void *)from, (size_t)raw_size);
      ScoopGcObjectHeader *to = (ScoopGcObjectHeader *)p;
      // 后续会重建 heap 链表，因此先清空 next，避免携带旧链表指针。
      to->next = 0;

      moves[move_len].from = from;
      moves[move_len].to = to;
      moves[move_len].size = raw_size;
      move_len += 1;
    }
  }

  if (move_failed) {
    // 回滚：释放已分配的 to-space 副本，并退化为 non-moving sweep（保留 live 对象）。
    if (moves != 0) {
      for (size_t i = 0; i < move_len; i++) {
        if (moves[i].to != 0) {
          free(moves[i].to);
        }
      }
      free(moves);
    }

    ScoopGcObjectHeader *new_list = 0;
    for (size_t i = 0; i < live_len; i++) {
      ScoopGcObjectHeader *obj = live[i];
      if (obj == 0) {
        continue;
      }
      obj->next = new_list;
      new_list = obj;
    }
    heap->objects = new_list;

    if (live != 0) {
      free(live);
    }
    return;
  }

  if (move_len == 0) {
    // 没有任何对象可搬迁（例如全 pinned）：仅重建 heap 链表后返回。
    ScoopGcObjectHeader *new_list = 0;
    for (size_t i = 0; i < live_len; i++) {
      ScoopGcObjectHeader *obj = live[i];
      if (obj == 0) {
        continue;
      }
      obj->next = new_list;
      new_list = obj;
    }
    heap->objects = new_list;
    if (moves != 0) {
      free(moves);
    }
    if (live != 0) {
      free(live);
    }
    return;
  }

  // 3) 提交：写入 forwarding pointer。
  for (size_t i = 0; i < move_len; i++) {
    ScoopGcObjectHeader *from = moves[i].from;
    ScoopGcObjectHeader *to = moves[i].to;
    scoop_gc_baseline_object_set_forwarding_ptr(from, to);
  }

  // 为 roots update 做 membership 过滤：stackmap records 可能枚举到“看起来像指针但并非 GC 对象”的值。
  // 在 update visitor 中必须避免对这些值解引用读取 `obj->next` 导致崩溃。
  //
  // 这里复用 live 数组并就地排序：顺序不影响后续 sweep/重建 heap 链表。
  qsort(live, live_len, sizeof(ScoopGcObjectHeader *), scoop_gc_ptr_cmp);
  ScoopGcBaselineUpdateCtx update_ctx = {
      .live_sorted = live,
      .live_len = live_len,
  };

  // 4) roots update：
  // - initiator：使用本次 GC 内部捕获的 ctx（可枚举完整 managed 调用栈的 stackmap roots slots）；
  // - Parked：使用 park 前捕获到 TLS 的 ctx；
  // - InNative：使用 native_roots buffer；
  // - B2a：baseline 不再更新 shadow stack roots（GC roots 不再来源于 `ScoopGcFrame`）。

  // 4a) initiator stackmap roots update（moving GC 的关键语义：spill slots 原地改写为新地址）。
  {
    ScoopGcManagedRootMap root_map =
        scoop_gc_managed_root_map_from_stackmap_ctx(initiator_stack_walking_ctx);
    ScoopGcRootMapVisitResult root_map_result = {0};
    uint32_t err = SCOOP_GC_ROOT_MAP_VISIT_ERR_INVALID_ARGUMENT;
    uint32_t records_hit = 0;
    if (initiator_stack_walking_ctx != 0) {
      (void)scoop_gc_root_map_visit_slots(
          &root_map, scoop_gc_baseline_update_slot_visitor, (void *)&update_ctx, &root_map_result);
      err = root_map_result.visit_error;
      records_hit = root_map_result.units_hit;
    }

    if (err != SCOOP_STACKMAP_VISIT_OK) {
      (void)fprintf(stderr,
                    "[scooprt][gc][move] initiator stackmap roots update failed: err=%u\n",
                    (unsigned)err);
      abort();
    }

    // 重要：moving GC 必须更新 stackmap spill slots（statepoint + gc.relocate 依赖它）。
    // 若未命中任何 record，说明当前进程未生成/未注册 stackmaps，或线程未停在可识别的 managed 帧；
    // 在这种情况下继续搬迁会导致 mutator 恢复后仍持有旧指针（悬挂指针），因此直接 fail-fast。
    if (records_hit == 0) {
      (void)fprintf(stderr,
                    "[scooprt][gc][move] initiator stackmap roots update hit 0 records; "
                    "moving GC requires statepoint stackmaps (set SCOOP_GC_MOVE=0 to disable)\n");
      abort();
    }

    // 若 initiator 处于 InNative，则 spill slots 之外还必须更新 `native_roots` buffer：
    // - enter_native 传入的是 “void** slots” 的指针数组；
    // - moving GC 结束后该线程会返回到 managed code，locals alloca 槽位必须已写回新地址。
    for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
      if (!pthread_equal(it->thread, initiator)) {
        continue;
      }
      if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
        if (it->stack_walking_ctx == 0) {
          (void)fprintf(stderr,
                        "[scooprt][gc][stackmap] missing in-native ctx for initiator roots update\n");
          abort();
        }
        (void)scoop_gc_native_roots_visit_slots(it->native_roots,
                                                it->native_roots_len,
                                                scoop_gc_baseline_update_slot_visitor,
                                                (void *)&update_ctx);
        {
          ScoopGcManagedRootMap root_map =
              scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
          ScoopGcRootMapVisitResult root_map_result = {0};
          uint32_t records_hit = 0;
          (void)scoop_gc_root_map_visit_slots(
              &root_map, scoop_gc_baseline_update_slot_visitor, (void *)&update_ctx, &root_map_result);
          uint32_t err = root_map_result.visit_error;
          records_hit = root_map_result.units_hit;
          if (err != SCOOP_STACKMAP_VISIT_OK) {
            (void)fprintf(stderr,
                          "[scooprt][gc][move] initiator in-native caller roots update failed: err=%u\n",
                          (unsigned)err);
            abort();
          }
        }
      }
      break;
    }
  }

  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    if (pthread_equal(it->thread, initiator)) {
      continue;
    }

    if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
      if (it->stack_walking_ctx == 0) {
        (void)fprintf(stderr,
                      "[scooprt][gc][move] missing in-native ctx for roots update (thread=0x%" PRIxPTR
                      ")\n",
                      (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
        abort();
      }
      (void)scoop_gc_native_roots_visit_slots(it->native_roots,
                                              it->native_roots_len,
                                              scoop_gc_baseline_update_slot_visitor,
                                              (void *)&update_ctx);
      {
        ScoopGcManagedRootMap root_map =
            scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
        ScoopGcRootMapVisitResult root_map_result = {0};
        uint32_t records_hit = 0;
        (void)scoop_gc_root_map_visit_slots(
            &root_map, scoop_gc_baseline_update_slot_visitor, (void *)&update_ctx, &root_map_result);
        uint32_t err = root_map_result.visit_error;
        records_hit = root_map_result.units_hit;
        if (err != SCOOP_STACKMAP_VISIT_OK) {
          (void)fprintf(stderr,
                        "[scooprt][gc][move] in-native caller roots update failed: err=%u "
                        "(thread=0x%" PRIxPTR ")\n",
                        (unsigned)err,
                        (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
          abort();
        }
      }
      continue;
    }

    if (it->state == SCOOP_GC_THREAD_PARKED && it->stack_walking_ctx != 0) {
      ScoopGcManagedRootMap root_map =
          scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
      ScoopGcRootMapVisitResult root_map_result = {0};
      uint32_t records_hit = 0;
      (void)scoop_gc_root_map_visit_slots(
          &root_map, scoop_gc_baseline_update_slot_visitor, (void *)&update_ctx, &root_map_result);
      uint32_t err = root_map_result.visit_error;
      records_hit = root_map_result.units_hit;
      if (err != SCOOP_STACKMAP_VISIT_OK) {
        (void)fprintf(stderr,
                      "[scooprt][gc][stackmap] update roots failed: err=%u (thread=0x%" PRIxPTR
                      ")\n",
                      (unsigned)err,
                      (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
        abort();
      }
    }
  }

  // 4b) stable handle table update：handle->obj 槽位同样属于 roots。
  for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
    if (it->object == 0) {
      continue;
    }
    scoop_gc_baseline_update_slot_visitor((void **)&it->object, (void *)&update_ctx);
  }

  // 4b2) module-global roots update：module-local backing globals 同样属于永久 roots。
  (void)scoop_gc_global_roots_visit_unlocked(scoop_gc_baseline_update_slot_visitor,
                                             (void *)&update_ctx);

  // 4c) heap object fields update：扫描所有 live 对象（对已搬迁对象扫描其 to-space 副本）。
  for (size_t i = 0; i < live_len; i++) {
    ScoopGcObjectHeader *obj = live[i];
    if (obj == 0) {
      continue;
    }

    ScoopGcObjectHeader *current = scoop_gc_baseline_follow_forwarding(obj);
    if (current == 0) {
      continue;
    }
    if (current->type_desc == 0) {
      continue;
    }

    (void)scoop_gc_type_descriptor_trace(current->type_desc,
                                         (void *)current,
                                         scoop_gc_baseline_update_slot_visitor,
                                         (void *)&update_ctx);
  }

  // 5) 重建 heap.objects：保留 pinned/未搬迁对象 + 追加 to-space 副本；from-space 旧对象从 heap 链表中移除。
  ScoopGcObjectHeader *new_list = 0;
  for (size_t i = 0; i < live_len; i++) {
    ScoopGcObjectHeader *obj = live[i];
    if (obj == 0) {
      continue;
    }

    ScoopGcObjectHeader *current = scoop_gc_baseline_follow_forwarding(obj);
    if (current == 0) {
      continue;
    }
    current->next = new_list;
    new_list = current;
  }
  heap->objects = new_list;

  // 6) 释放旧副本（from-space）：注意不计入 `bytes_freed`（对象仍存活；这里只是搬迁内部实现细节）。
  for (size_t i = 0; i < move_len; i++) {
    if (moves[i].from != 0) {
      free(moves[i].from);
    }
  }

  if (moves != 0) {
    free(moves);
  }
  if (live != 0) {
    free(live);
  }
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

  // 若别的线程已经发起了 STW，本线程此时必须先参与 safepoint；
  // 否则它会停留在 Running 状态，而 initiator 会永远等不到 parked_count。
  if (scoop_gc_stw_requested_load(&scoop_gc_stw) && !pthread_equal(self, scoop_gc_stw.initiator)) {
    (void)pthread_mutex_unlock(&scoop_gc_lock);
    scoop_gc_safepoint_poll();
    return;
  }

  // 保证同一时刻只允许一个 GC 周期。
  while (scoop_gc_stw_requested_load(&scoop_gc_stw)) {
    (void)pthread_cond_wait(&scoop_gc_cond, &scoop_gc_lock);
  }

  scoop_gc_stop_the_world_begin_unlocked(self);

  ScoopGcHeap *heap = &scoop_gc_heap;
  uint32_t mark_value = scoop_gc_collect_next_mark_value(heap);

  // T1511：baseline moving 模式（可通过 env 强制开启），需要在 GC 内捕获 initiator 的 stack walking ctx，
  // 用于 stackmap spill slots 更新（statepoint + gc.relocate 依赖该语义）。
  uint32_t moving_enabled = scoop_gc_baseline_moving_enabled();
  // B2a：baseline roots 枚举不再扫描 shadow stack（`ScoopGcFrame`）。
  //
  // 约定：
  // - in-native 线程：roots 来自 `native_roots` slots（enter_native 注册），以及 pinned/handles；
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
  if (moving_enabled || initiator_needs_stackmap_roots) {
    initiator_stack_walking_ctx = scoop_platform_unwind_ctx_capture();
    if (initiator_stack_walking_ctx == 0) {
      (void)fprintf(stderr, "[scooprt][gc][stackmap] failed to capture unwind ctx\n");
      abort();
    }
  }

  ScoopGcMarkStack stack = {0};
  ScoopGcMarkCtx ctx = {heap, mark_value, &stack};

  for (ScoopGcThreadRecord *it = scoop_gc_threads; it != 0; it = it->next) {
    // T1505c：InNative 线程不要求 park，roots 来自 TLS `native_roots` buffer。
    if (it->state == SCOOP_GC_THREAD_IN_NATIVE) {
      if (it->stack_walking_ctx == 0) {
        (void)fprintf(stderr,
                      "[scooprt][gc][stackmap] missing in-native ctx for mark roots (thread=0x%" PRIxPTR
                      ")\n",
                      (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
        abort();
      }
      (void)scoop_gc_native_roots_visit_slots(
          it->native_roots, it->native_roots_len, scoop_gc_mark_visitor, (void *)&ctx);
      {
        ScoopGcManagedRootMap root_map =
            scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
        ScoopGcRootMapVisitResult root_map_result = {0};
        uint32_t records_hit = 0;
        (void)scoop_gc_root_map_visit_slots(
            &root_map, scoop_gc_mark_visitor, (void *)&ctx, &root_map_result);
        uint32_t err = root_map_result.visit_error;
        records_hit = root_map_result.units_hit;

        if (err != SCOOP_STACKMAP_VISIT_OK) {
          (void)fprintf(stderr,
                        "[scooprt][gc][stackmap] visit in-native caller roots failed: err=%u "
                        "(thread=0x%" PRIxPTR ")\n",
                        (unsigned)err,
                        (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
          abort();
        }
      }
      continue;
    }

    // B2a：initiator roots 同样必须来自 stackmap（覆盖完整 managed 栈）。
    if (initiator_needs_stackmap_roots && initiator_stack_walking_ctx != 0 &&
        pthread_equal(it->thread, self)) {
      ScoopGcManagedRootMap root_map =
          scoop_gc_managed_root_map_from_stackmap_ctx(initiator_stack_walking_ctx);
      ScoopGcRootMapVisitResult root_map_result = {0};
      uint32_t records_hit = 0;
      (void)scoop_gc_root_map_visit_slots(
          &root_map, scoop_gc_mark_visitor, (void *)&ctx, &root_map_result);
      uint32_t err = root_map_result.visit_error;
      records_hit = root_map_result.units_hit;

      if (err != SCOOP_STACKMAP_VISIT_OK) {
        (void)fprintf(stderr,
                      "[scooprt][gc][stackmap] visit initiator roots failed: err=%u\n",
                      (unsigned)err);
        abort();
      }
      continue;
    }

    // T1506b：Parked 线程若提供了 stack_walking_ctx，则走 stackmap roots。
    if (it->state == SCOOP_GC_THREAD_PARKED && it->stack_walking_ctx != 0) {
      ScoopGcManagedRootMap root_map =
          scoop_gc_managed_root_map_from_stackmap_ctx(it->stack_walking_ctx);
      ScoopGcRootMapVisitResult root_map_result = {0};
      uint32_t records_hit = 0;
      (void)scoop_gc_root_map_visit_slots(
          &root_map, scoop_gc_mark_visitor, (void *)&ctx, &root_map_result);
      uint32_t err = root_map_result.visit_error;
      records_hit = root_map_result.units_hit;

      if (err != SCOOP_STACKMAP_VISIT_OK) {
        // locations 解析失败/遇到寄存器 roots 等：视为编译器/管线错误，fail-fast。
        (void)fprintf(stderr,
                      "[scooprt][gc][stackmap] visit roots failed: err=%u (thread=0x%" PRIxPTR
                      ")\n",
                      (unsigned)err,
                      (uintptr_t)scoop_gc_thread_id_for_diag(it->thread));
        abort();
      }
    }
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

  // 1c) mark stable handles（spec §15.10.1）：handle 表中的对象必须保活，即使没有 roots 引用。
  for (ScoopGcHandleRecord *it = scoop_gc_handle_records; it != 0; it = it->next) {
    if (it->object == 0) {
      continue;
    }
    scoop_gc_mark_object_if_needed(&ctx, it->object);
  }

  // 1d) mark module-global roots：object/top-level backing globals 也必须保活其引用对象。
  (void)scoop_gc_global_roots_visit_unlocked(scoop_gc_mark_visitor, (void *)&ctx);

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

  // 3) sweep（可选 moving + fixup）
  if (moving_enabled) {
    scoop_gc_collect_baseline_moving_unlocked(
        heap, mark_value, self, initiator_stack_walking_ctx);
  } else {
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

      heap->bytes_freed += obj->size_bytes;
      free(obj);
    }
  }

  if (scoop_gc_verify_roots_enabled()) {
    scoop_gc_verify_roots_after_gc_unlocked(heap, self, initiator_stack_walking_ctx);
  }

  if (initiator_stack_walking_ctx != 0) {
    scoop_platform_unwind_ctx_destroy(initiator_stack_walking_ctx);
    initiator_stack_walking_ctx = 0;
  }

  scoop_gc_stop_the_world_end_unlocked();
  (void)pthread_mutex_unlock(&scoop_gc_lock);
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

  pthread_t self = pthread_self();
  (void)pthread_mutex_lock(&scoop_gc_lock);
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
  (void)pthread_mutex_unlock(&scoop_gc_lock);

  // 恢复 stackmap registry（避免影响同进程内其它测试）。
  scoop_stackmap_registry_reset();
  (void)scoop_stackmap_registry_register_current_process();

  if (section != 0) {
    free(section);
  }

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

// Test-only export（T1506c）：端到端验证 “多帧 roots + stackmap lookup”：
// - outer frame 的 stack root 保活一个 heap 对象；
// - inner frame 在 safepoint poll 处 park；main 线程触发 stop-the-world GC；
// - GC 逐帧 lookup stackmap records，并能扫描到 outer frame 的 root slot；
// - GC 后 worker 仍能访问该对象；随后 root 消失后对象可被回收（release callback 被调用一次）。
//
// 返回：
// - 1：通过
// - 0：当前平台/编译器不支持（例如非 clang/gcc）
// - <0：失败（用于测试诊断）
intptr_t scoop_test_gc_stackmap_multiframe_keepalive(void) {
  void scoop_runtime_init(void);
  void scoop_thread_register(void);
  void scoop_thread_unregister(void);

  // 仅用于测试断言（避免依赖未声明函数的隐式声明）。
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
  pthread_t self = pthread_self();
  (void)pthread_mutex_lock(&scoop_gc_lock);
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

  // 构造一个最小可解析的 stackmap section：
  // - 2 functions, 2 records；
  // - inner record：0 locations（仅用于 records_hit 计数）；
  // - outer record：2 个 Direct locations（模拟 statepoint base/derived 成对 roots），均指向 outer frame 的 root slot。
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
  // tail padding already zeroed (align to 8)

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

  // 先 reset，再 register current process（设置幂等标记），最后注册 synthetic section。
  scoop_stackmap_registry_reset();
  (void)scoop_stackmap_registry_register_current_process();
  const uint32_t added =
      scoop_stackmap_registry_register_section((const uint8_t *)section, section_size);
  if (added == 0) {
    rc = -28;
    goto done;
  }

  // 额外断言：至少命中 inner+outer 两帧 record，且能枚举到 outer 的非空 roots slot。
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
  (void)pthread_mutex_unlock(&scoop_gc_lock);

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

  if (rc != 1) {
    // 失败路径：确保 worker 已被 stop/join，避免测试进程悬挂。
    __atomic_store_n(&shared.stop, 1, __ATOMIC_SEQ_CST);
    (void)pthread_join(worker, 0);
  }

  scoop_thread_unregister();
  return rc;
}

void scoop_gc_collect_minor(void) {
  // baseline backend 无 nursery/minor 语义：minor 退化为一次 major collect，保持链接稳定。
  scoop_gc_collect();
}

uint32_t scoop_gc_try_collect_minor(uint32_t deadline_ms) {
  (void)deadline_ms;
  scoop_gc_collect_minor();
  return 1;
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
    // 防御：饱和加，避免极端情况下 u64 溢出导致观测值回卷。
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

#endif // SCOOP_GC_BACKEND == SCOOP_GC_BACKEND_BASELINE
