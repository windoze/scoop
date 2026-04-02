// Scoop GC stop-the-world helpers (internal, header-only).
//
// 目的（TODO T1408b）：
// - 把当前基于 shadow stack 的协作式 STW（T1408a）与未来基于 stackmap/statepoint 的
//   STW（T1505）在“线程记录结构 / 线程状态机 / park 语义 / 超时诊断”层面对齐；
// - 通过共享结构与 helper，避免两套协议在实现上长期分叉。
//
// 注意：
// - 该头文件是 **internal**：仅供 `runtime/c/*` 的 GC backends include。
// - 为避免引入新的导出符号（见 `runtime/c/scoop_runtime_api.h` 的 ABI allowlist），
//   本文件保持 header-only（全部为 `static inline`）。

#ifndef SCOOP_GC_STW_INTERNAL_H
#define SCOOP_GC_STW_INTERNAL_H

#include <errno.h>
#include <inttypes.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

typedef struct ScoopGcFrame ScoopGcFrame;

// 线程状态机（对齐 PLAN §9.1 / TODO T1505）：
// - Running：正常执行（可被 STW 请求打断）；
// - Parked：已进入 safepoint park（GC 可枚举 roots）；
// - InNative：处于 native 过渡态（roots 从 TLS native_roots 来；T1505/T1510）。
typedef enum ScoopGcThreadState {
  SCOOP_GC_THREAD_RUNNING = 0,
  SCOOP_GC_THREAD_PARKED = 1,
  SCOOP_GC_THREAD_IN_NATIVE = 2,
} ScoopGcThreadState;

// GC 线程记录（shadow stack v0 + 为 stackmap 预留字段）。
typedef struct ScoopGcThreadRecord {
  struct ScoopGcThreadRecord *next;
  pthread_t thread;

  // roots 来源（shadow stack v0）：指向该线程 TLS 内的 `current_frame` 槽位。
  // stackmap 时代该字段可为 NULL（roots 将来自 stack walking / native_roots）。
  ScoopGcFrame **current_frame_slot;

  ScoopGcThreadState state;

  // 诊断字段：该线程“最后一次观察到/更新到的 STW epoch”。
  uint64_t last_safepoint_epoch;

  // 该线程进入 Parked 的 epoch（用于避免重复计数）。
  uint64_t parked_epoch;

  // 预留：未来 stack walking 上下文 / native roots（T1505/T1506/T1510）。
  void *stack_walking_ctx;
  void *native_roots;
  uint32_t native_roots_len;
} ScoopGcThreadRecord;

typedef struct ScoopGcStwState {
  uint32_t requested;   // 0/1（受 lock 保护；T1505 会升级为原子 fast path）
  pthread_t initiator;  // 当前 STW 发起方（只有发起方不 park）
  uint64_t epoch;       // 每次 STW begin 递增（用于诊断与配对）
  uint32_t parked_count;
} ScoopGcStwState;

// 超时诊断间隔：在等待所有线程 park 的过程中，每隔该时间打印一次状态快照（仅在超时触发）。
#ifndef SCOOP_GC_STW_DIAG_INTERVAL_MS
#define SCOOP_GC_STW_DIAG_INTERVAL_MS 1000u
#endif

static inline const char *scoop_gc_thread_state_name(ScoopGcThreadState s) {
  switch (s) {
  case SCOOP_GC_THREAD_RUNNING:
    return "Running";
  case SCOOP_GC_THREAD_PARKED:
    return "Parked";
  case SCOOP_GC_THREAD_IN_NATIVE:
    return "InNative";
  default:
    return "Unknown";
  }
}

// 为诊断生成一个“尽力而为”的线程标识：把 pthread_t 的低位字节拷贝进 uintptr_t。
// 说明：
// - pthread_t 的表示在不同平台可能是 pointer/整数/结构体；
// - 这里不追求可逆或稳定 ABI，只用于卡死/超时日志定位问题线程。
static inline uintptr_t scoop_gc_thread_id_for_diag(pthread_t t) {
  uintptr_t out = 0;
  size_t n = sizeof(t);
  if (n > sizeof(out)) {
    n = sizeof(out);
  }
  (void)memcpy(&out, &t, n);
  return out;
}

static inline void scoop_gc_stw_diag_dump_threads_unlocked(const ScoopGcStwState *stw,
                                                           const ScoopGcThreadRecord *threads,
                                                           uint32_t need_to_park) {
  if (stw == 0) {
    return;
  }

  (void)fprintf(stderr,
                "[scooprt][gc][stw] waiting for park: epoch=%" PRIu64
                " parked=%u need=%u\n",
                (uint64_t)stw->epoch,
                (unsigned)stw->parked_count,
                (unsigned)need_to_park);

  for (const ScoopGcThreadRecord *it = threads; it != 0; it = it->next) {
    uintptr_t tid = scoop_gc_thread_id_for_diag(it->thread);
    uint32_t is_initiator = 0;
    if (pthread_equal(it->thread, stw->initiator)) {
      is_initiator = 1;
    }

    (void)fprintf(stderr,
                  "  - thread=0x%" PRIxPTR " state=%s last_epoch=%" PRIu64
                  " parked_epoch=%" PRIu64 "%s\n",
                  tid,
                  scoop_gc_thread_state_name(it->state),
                  (uint64_t)it->last_safepoint_epoch,
                  (uint64_t)it->parked_epoch,
                  is_initiator ? " (initiator)" : "");
  }

  (void)fflush(stderr);
}

static inline void scoop_gc_stw_timespec_after_ms(uint32_t ms, struct timespec *out) {
  if (out == 0) {
    return;
  }

  struct timespec now;
#if defined(CLOCK_REALTIME)
  (void)clock_gettime(CLOCK_REALTIME, &now);
#else
  (void)timespec_get(&now, TIME_UTC);
#endif

  uint64_t add_ns = (uint64_t)ms * 1000000ull;
  uint64_t ns = (uint64_t)now.tv_nsec + add_ns;
  out->tv_sec = now.tv_sec + (time_t)(ns / 1000000000ull);
  out->tv_nsec = (long)(ns % 1000000000ull);
}

#endif

