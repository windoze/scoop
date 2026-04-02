// Scoop C runtime: std `scoop.sync` (platform backend, early stage).
//
// TODO T1319b：
// - 为 sysroot 的 `scoop.sync`（Mutex/CondVar/Once）提供最小可执行实现；
// - 由 LLVM codegen 将 sysroot 表面直接映射到本文件导出的 C 符号；
// - 当前阶段只覆盖 host 平台（POSIX/pthread 通过 `runtime/c/platform` 收敛）。
//
// 设计约定（early stage）：
// - `Mutex/CondVar/Once` 在 sysroot 侧声明为 class（引用类型），因此这里把它们实现为
//   “GC-managed 对象”（以 `ScoopGcObjectHeader` 开头，并通过 `scoop_alloc` 分配）。
// - 资源释放采用显式 `destroy()`：调用后该对象进入“已销毁”状态，后续操作会 no-op；
//   对象内存仍由 GC 回收（TODO：未来可通过 type descriptor 的 release_fn 接入 finalizer）。
// - `Once.run(block)` 当前只支持非捕获 lambda（由后端保证）；并提供最小的“同线程重入不死锁”
//   语义：初始化线程在 init 过程中再次 run 同一 once，会直接返回。

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "platform/platform.h"
#include "scoop_gc.h"

// `scoop_alloc` / 线程注册 API 由 `scoop_runtime.c` 提供；这里仅做前置声明。
void *scoop_alloc(uint64_t size);
void scoop_thread_register(void);

// --- Mutex ---

typedef struct ScoopSyncMutex {
  ScoopGcObjectHeader header;
  ScoopPlatformMutex mutex;
  uint32_t destroyed;
  uint32_t _reserved_u32;
} ScoopSyncMutex;

void *scoop_sync_mutex_create(void) {
  scoop_thread_register();

  ScoopSyncMutex *m = (ScoopSyncMutex *)scoop_alloc((uint64_t)sizeof(ScoopSyncMutex));
  if (m == 0) {
    return 0;
  }

  if (!scoop_platform_sync_mutex_init(&m->mutex)) {
    return 0;
  }
  m->destroyed = 0;
  m->_reserved_u32 = 0;
  return (void *)m;
}

void scoop_sync_mutex_lock(void *mutex_obj) {
  if (mutex_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopSyncMutex *m = (ScoopSyncMutex *)mutex_obj;
  if (m->destroyed) {
    return;
  }
  scoop_platform_sync_mutex_lock(&m->mutex);
}

void scoop_sync_mutex_unlock(void *mutex_obj) {
  if (mutex_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopSyncMutex *m = (ScoopSyncMutex *)mutex_obj;
  if (m->destroyed) {
    return;
  }
  scoop_platform_sync_mutex_unlock(&m->mutex);
}

void scoop_sync_mutex_destroy(void *mutex_obj) {
  if (mutex_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopSyncMutex *m = (ScoopSyncMutex *)mutex_obj;
  if (m->destroyed) {
    return;
  }
  m->destroyed = 1;
  scoop_platform_sync_mutex_destroy(&m->mutex);
}

// --- CondVar ---

typedef struct ScoopSyncCondVar {
  ScoopGcObjectHeader header;
  ScoopPlatformCondVar cond;
  uint32_t destroyed;
  uint32_t _reserved_u32;
} ScoopSyncCondVar;

void *scoop_sync_condvar_create(void) {
  scoop_thread_register();

  ScoopSyncCondVar *cv = (ScoopSyncCondVar *)scoop_alloc((uint64_t)sizeof(ScoopSyncCondVar));
  if (cv == 0) {
    return 0;
  }

  if (!scoop_platform_sync_condvar_init(&cv->cond)) {
    return 0;
  }
  cv->destroyed = 0;
  cv->_reserved_u32 = 0;
  return (void *)cv;
}

void scoop_sync_condvar_wait(void *condvar_obj, void *mutex_obj) {
  if (condvar_obj == 0 || mutex_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopSyncCondVar *cv = (ScoopSyncCondVar *)condvar_obj;
  ScoopSyncMutex *m = (ScoopSyncMutex *)mutex_obj;
  if (cv->destroyed || m->destroyed) {
    return;
  }

  // 约定：调用方必须在进入 wait 前持有 mutex；pthread_cond_wait 会原子地解锁并等待，
  // 被唤醒后在返回前重新加锁。
  scoop_platform_sync_condvar_wait(&cv->cond, &m->mutex);
}

void scoop_sync_condvar_notify_one(void *condvar_obj) {
  if (condvar_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopSyncCondVar *cv = (ScoopSyncCondVar *)condvar_obj;
  if (cv->destroyed) {
    return;
  }
  scoop_platform_sync_condvar_signal(&cv->cond);
}

void scoop_sync_condvar_notify_all(void *condvar_obj) {
  if (condvar_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopSyncCondVar *cv = (ScoopSyncCondVar *)condvar_obj;
  if (cv->destroyed) {
    return;
  }
  scoop_platform_sync_condvar_broadcast(&cv->cond);
}

void scoop_sync_condvar_destroy(void *condvar_obj) {
  if (condvar_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopSyncCondVar *cv = (ScoopSyncCondVar *)condvar_obj;
  if (cv->destroyed) {
    return;
  }
  cv->destroyed = 1;
  scoop_platform_sync_condvar_destroy(&cv->cond);
}

// --- Once ---

typedef enum ScoopSyncOnceStateU32 {
  SCOOP_SYNC_ONCE_STATE_UNINITIALIZED = 0u,
  SCOOP_SYNC_ONCE_STATE_INITIALIZING = 1u,
  SCOOP_SYNC_ONCE_STATE_INITIALIZED = 2u,
} ScoopSyncOnceStateU32;

typedef void (*ScoopSyncOnceInitFn)(void *env);

typedef struct ScoopSyncOnce {
  ScoopGcObjectHeader header;
  ScoopPlatformMutex lock;
  ScoopPlatformCondVar cond;
  uint32_t state;
  uint32_t _reserved_u32;
  ScoopPlatformThread owner;
} ScoopSyncOnce;

void *scoop_sync_once_create(void) {
  scoop_thread_register();

  ScoopSyncOnce *o = (ScoopSyncOnce *)scoop_alloc((uint64_t)sizeof(ScoopSyncOnce));
  if (o == 0) {
    return 0;
  }

  if (!scoop_platform_sync_mutex_init(&o->lock)) {
    return 0;
  }
  if (!scoop_platform_sync_condvar_init(&o->cond)) {
    scoop_platform_sync_mutex_destroy(&o->lock);
    return 0;
  }
  o->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_UNINITIALIZED;
  o->_reserved_u32 = 0;
  (void)memset(&o->owner, 0, sizeof(o->owner));
  return (void *)o;
}

bool scoop_sync_once_is_done(void *once_obj) {
  if (once_obj == 0) {
    return false;
  }

  scoop_thread_register();

  ScoopSyncOnce *o = (ScoopSyncOnce *)once_obj;
  scoop_platform_sync_mutex_lock(&o->lock);
  bool done = (o->state == (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZED);
  scoop_platform_sync_mutex_unlock(&o->lock);
  return done;
}

void scoop_sync_once_run(void *once_obj, void *env_ptr, ScoopSyncOnceInitFn fn) {
  if (once_obj == 0 || fn == 0) {
    return;
  }

  scoop_thread_register();

  ScoopSyncOnce *o = (ScoopSyncOnce *)once_obj;
  ScoopPlatformThread self = scoop_platform_thread_self();

  scoop_platform_sync_mutex_lock(&o->lock);

  uint32_t state = o->state;
  if (state == (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZED) {
    scoop_platform_sync_mutex_unlock(&o->lock);
    return;
  }

  if (state == (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZING) {
    // 同线程重入：直接返回，避免自旋/死锁。
    if (scoop_platform_thread_equal(o->owner, self)) {
      scoop_platform_sync_mutex_unlock(&o->lock);
      return;
    }

    // 其它线程正在初始化：等待其完成。
    while (o->state == (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZING) {
      scoop_platform_sync_condvar_wait(&o->cond, &o->lock);
    }
    scoop_platform_sync_mutex_unlock(&o->lock);
    return;
  }

  // 当前线程获得初始化权：释放锁后执行 block，避免在 block 内持有 once 锁导致反向依赖死锁。
  o->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZING;
  o->owner = self;
  scoop_platform_sync_mutex_unlock(&o->lock);

  fn(env_ptr);

  scoop_platform_sync_mutex_lock(&o->lock);
  o->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZED;
  scoop_platform_sync_condvar_broadcast(&o->cond);
  scoop_platform_sync_mutex_unlock(&o->lock);
}
