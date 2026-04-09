// Scoop C runtime: std `scoop.thread` (platform backend, early stage).
//
// TODO T1319c：
// - 为 sysroot 的 `scoop.thread`（spawn/join/sleep/yield/currentId）提供最小可执行实现；
// - 由 LLVM codegen 将 sysroot 表面直接映射到本文件导出的 C 符号；
// - 当前阶段只覆盖 host 平台（POSIX/pthread 通过 `runtime/c/platform` 收敛）。
//
// 设计约定（early stage）：
// - `Thread` 在 sysroot 侧声明为 class（引用类型），这里实现为 “GC-managed 对象”
//   （以 `ScoopGcObjectHeader` 开头，并通过 `scoop_alloc` 分配）。
// - 为避免在 early stage 引入更多资源管理语义：不提供 detach/destroy；调用方应显式 `join()`。
// - 线程入口会调用 `scoop_thread_register/unregister`，避免 GC 的线程枚举残留已退出线程的 TLS 槽位。

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "platform/platform.h"
#include "scoop_gc.h"

// `scoop_alloc` / 线程注册 API 由 `scoop_runtime.c` 提供；这里仅做前置声明。
void *scoop_alloc(uint64_t size);
void scoop_thread_register(void);
void scoop_thread_unregister(void);

// GC native transition (defined in scoop_gc.c / backend): transition to IN_NATIVE
// before blocking system calls, allowing STW GC to skip this thread.
void scoop_enter_native(void ***root_slots, uint32_t root_slots_len);
void scoop_leave_native(void);

typedef void (*ScoopThreadStartFn)(void *env);

typedef struct ScoopThreadHandle {
  ScoopGcObjectHeader header;
  ScoopPlatformThread thread;
  uint32_t started;
  uint32_t joined;
} ScoopThreadHandle;

typedef struct ScoopThreadStartArgs {
  void *env;
  ScoopThreadStartFn fn;
} ScoopThreadStartArgs;

static void *scoop_thread_entry(void *arg) {
  if (arg == 0) {
    return 0;
  }

  ScoopThreadStartArgs *args = (ScoopThreadStartArgs *)arg;
  ScoopThreadStartFn fn = args->fn;
  void *env = args->env;
  free(args);

  scoop_thread_register();
  if (fn != 0) {
    fn(env);
  }
  scoop_thread_unregister();
  return 0;
}

void *scoop_thread_spawn(void *env_ptr, ScoopThreadStartFn fn) {
  scoop_thread_register();

  ScoopThreadHandle *t = (ScoopThreadHandle *)scoop_alloc((uint64_t)sizeof(ScoopThreadHandle));
  if (t == 0) {
    return 0;
  }

  (void)memset(&t->thread, 0, sizeof(t->thread));
  t->started = 0;
  t->joined = 0;

  ScoopThreadStartArgs *args = (ScoopThreadStartArgs *)malloc(sizeof(ScoopThreadStartArgs));
  if (args == 0) {
    return 0;
  }
  args->env = env_ptr;
  args->fn = fn;

  if (!scoop_platform_thread_spawn(&t->thread, scoop_thread_entry, (void *)args)) {
    free(args);
    return 0;
  }

  t->started = 1;
  return (void *)t;
}

void scoop_thread_join(void *thread_obj) {
  if (thread_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopThreadHandle *t = (ScoopThreadHandle *)thread_obj;
  if (!t->started) {
    return;
  }
  if (t->joined) {
    return;
  }

  t->joined = 1;

  // T0105: Transition to IN_NATIVE before blocking on thread join.
  // Without this, the calling thread stays RUNNING but cannot reach a safepoint
  // (blocked in kernel); if the child thread triggers GC, STW will deadlock.
  scoop_enter_native(0, 0);
  (void)scoop_platform_thread_join(t->thread);
  scoop_leave_native();
}

void scoop_thread_yield(void) {
  scoop_thread_register();
  scoop_platform_thread_yield();
}

void scoop_thread_sleep_millis(int64_t ms) {
  if (ms <= 0) {
    return;
  }

  scoop_thread_register();

  // T0105: Transition to IN_NATIVE before blocking on sleep.
  scoop_enter_native(0, 0);
  scoop_platform_thread_sleep_millis(ms);
  scoop_leave_native();
}

int64_t scoop_thread_current_id(void) {
  scoop_thread_register();
  return scoop_platform_thread_current_id();
}
