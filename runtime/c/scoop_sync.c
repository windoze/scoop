// Scoop C runtime: std `scoop.sync` (platform backend, early stage).
//
// TODO T1319b：
// - 为 sysroot 的 `scoop.sync`（Mutex/CondVar/Once）提供最小可执行实现；
// - 由 LLVM codegen 将 sysroot 表面直接映射到本文件导出的 C 符号；
// - 当前阶段只覆盖 host 平台（POSIX/pthread 通过 `runtime/c/platform` 收敛）。
//
// 设计约定（early stage）：
// - `Mutex/CondVar/Once` 在 sysroot 侧声明为 class（引用类型），因此这里把它们实现为
//   “GC-managed 对象”（以 `ScoopGcObjectHeader` 开头，并通过 `scoop_alloc_typed` 分配）。
// - 资源释放采用双路径：
//   - 用户可通过显式 `destroy()` 提前释放平台资源；
//   - 若对象在未显式 `destroy()` 的情况下变为不可达，则通过 type descriptor 的 release_fn
//     在 sweep 前做同一份 cleanup。
// - 这里的 release_fn 只是受限 GC cleanup，不等价于通用 finalizer：不保证顺序，且不允许复活对象。
// - `Once.run(block)` 当前只支持非捕获 lambda（由后端保证）；并提供最小的“同线程重入不死锁”
//   语义：初始化线程在 init 过程中再次 run 同一 once，会直接返回。

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

#include "platform/platform.h"
#include "scoop_gc.h"

// `scoop_alloc` / 线程注册 API 由 `scoop_runtime.c` 提供；这里仅做前置声明。
void *scoop_alloc(uint64_t size);
void *scoop_alloc_typed(const ScoopTypeDescriptor *type_desc, uint64_t size_bytes);
void scoop_thread_register(void);

// GC native transition (defined in scoop_gc.c / backend): transition to IN_NATIVE
// before blocking system calls, allowing STW GC to skip this thread.
void scoop_enter_native(void ***root_slots, uint32_t root_slots_len);
void scoop_leave_native(void);

// --- Test-only destroy counters ---

static uint64_t scoop_test_sync_mutex_destroy_calls = 0;
static uint64_t scoop_test_sync_condvar_destroy_calls = 0;
static uint64_t scoop_test_sync_once_destroy_calls = 0;

void scoop_test_sync_destroy_counts_reset(void) {
  __atomic_store_n(&scoop_test_sync_mutex_destroy_calls, 0u, __ATOMIC_SEQ_CST);
  __atomic_store_n(&scoop_test_sync_condvar_destroy_calls, 0u, __ATOMIC_SEQ_CST);
  __atomic_store_n(&scoop_test_sync_once_destroy_calls, 0u, __ATOMIC_SEQ_CST);
}

intptr_t scoop_test_sync_mutex_destroy_count(void) {
  return (intptr_t)__atomic_load_n(&scoop_test_sync_mutex_destroy_calls, __ATOMIC_SEQ_CST);
}

intptr_t scoop_test_sync_condvar_destroy_count(void) {
  return (intptr_t)__atomic_load_n(&scoop_test_sync_condvar_destroy_calls, __ATOMIC_SEQ_CST);
}

intptr_t scoop_test_sync_once_destroy_count(void) {
  return (intptr_t)__atomic_load_n(&scoop_test_sync_once_destroy_calls, __ATOMIC_SEQ_CST);
}

// --- Mutex ---

typedef struct ScoopSyncMutex {
  ScoopGcObjectHeader header;
  ScoopPlatformMutex mutex;
  uint32_t destroyed;
  uint32_t initialized;
} ScoopSyncMutex;

static void scoop_sync_mutex_destroy_impl(ScoopSyncMutex *m) {
  if (m == 0 || m->destroyed || !m->initialized) {
    return;
  }

  m->destroyed = 1;
  scoop_platform_sync_mutex_destroy(&m->mutex);
  m->initialized = 0;
  (void)__atomic_fetch_add(&scoop_test_sync_mutex_destroy_calls, 1u, __ATOMIC_SEQ_CST);
}

static void scoop_sync_mutex_release(void *object) {
  scoop_sync_mutex_destroy_impl((ScoopSyncMutex *)object);
}

static const ScoopTypeDescriptor SCOOP_SYNC_MUTEX_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopSyncMutex),
    .align_bytes = (uint64_t)_Alignof(ScoopSyncMutex),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = 0,
    .release_fn = scoop_sync_mutex_release,
    .type_id = 0,
    .parent_type_desc = 0,
    .itable = 0,
    .vtable = 0,
};

void *scoop_sync_mutex_create(void) {
  scoop_thread_register();

  ScoopSyncMutex *m = (ScoopSyncMutex *)scoop_alloc_typed(
      &SCOOP_SYNC_MUTEX_TYPE_DESC, (uint64_t)sizeof(ScoopSyncMutex));
  if (m == 0) {
    return 0;
  }

  m->destroyed = 1;
  m->initialized = 0;
  if (!scoop_platform_sync_mutex_init(&m->mutex)) {
    return 0;
  }
  m->destroyed = 0;
  m->initialized = 1;
  return (void *)m;
}

void scoop_sync_mutex_lock(void *mutex_obj) {
  if (mutex_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopSyncMutex *m = (ScoopSyncMutex *)mutex_obj;
  if (m->destroyed || !m->initialized) {
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
  if (m->destroyed || !m->initialized) {
    return;
  }
  scoop_platform_sync_mutex_unlock(&m->mutex);
}

void scoop_sync_mutex_destroy(void *mutex_obj) {
  if (mutex_obj == 0) {
    return;
  }

  scoop_thread_register();

  scoop_sync_mutex_destroy_impl((ScoopSyncMutex *)mutex_obj);
}

// --- CondVar ---

typedef struct ScoopSyncCondVar {
  ScoopGcObjectHeader header;
  ScoopPlatformCondVar cond;
  uint32_t destroyed;
  uint32_t initialized;
} ScoopSyncCondVar;

static void scoop_sync_condvar_destroy_impl(ScoopSyncCondVar *cv) {
  if (cv == 0 || cv->destroyed || !cv->initialized) {
    return;
  }

  cv->destroyed = 1;
  scoop_platform_sync_condvar_destroy(&cv->cond);
  cv->initialized = 0;
  (void)__atomic_fetch_add(&scoop_test_sync_condvar_destroy_calls, 1u, __ATOMIC_SEQ_CST);
}

static void scoop_sync_condvar_release(void *object) {
  scoop_sync_condvar_destroy_impl((ScoopSyncCondVar *)object);
}

static const ScoopTypeDescriptor SCOOP_SYNC_CONDVAR_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopSyncCondVar),
    .align_bytes = (uint64_t)_Alignof(ScoopSyncCondVar),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = 0,
    .release_fn = scoop_sync_condvar_release,
    .type_id = 0,
    .parent_type_desc = 0,
    .itable = 0,
    .vtable = 0,
};

void *scoop_sync_condvar_create(void) {
  scoop_thread_register();

  ScoopSyncCondVar *cv = (ScoopSyncCondVar *)scoop_alloc_typed(
      &SCOOP_SYNC_CONDVAR_TYPE_DESC, (uint64_t)sizeof(ScoopSyncCondVar));
  if (cv == 0) {
    return 0;
  }

  cv->destroyed = 1;
  cv->initialized = 0;
  if (!scoop_platform_sync_condvar_init(&cv->cond)) {
    return 0;
  }
  cv->destroyed = 0;
  cv->initialized = 1;
  return (void *)cv;
}

void scoop_sync_condvar_wait(void *condvar_obj, void *mutex_obj) {
  if (condvar_obj == 0 || mutex_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopSyncCondVar *cv = (ScoopSyncCondVar *)condvar_obj;
  ScoopSyncMutex *m = (ScoopSyncMutex *)mutex_obj;
  if (cv->destroyed || !cv->initialized || m->destroyed || !m->initialized) {
    return;
  }

  // 约定：调用方必须在进入 wait 前持有 mutex；pthread_cond_wait 会原子地解锁并等待，
  // 被唤醒后在返回前重新加锁。
  //
  // T0105: Transition to IN_NATIVE before blocking on condvar_wait.
  // Without this, the thread stays RUNNING but cannot reach a safepoint (blocked
  // in kernel); if another thread triggers GC, STW will deadlock waiting for this
  // thread to park.
  scoop_enter_native(0, 0);
  scoop_platform_sync_condvar_wait(&cv->cond, &m->mutex);
  scoop_leave_native();
}

void scoop_sync_condvar_notify_one(void *condvar_obj) {
  if (condvar_obj == 0) {
    return;
  }

  scoop_thread_register();

  ScoopSyncCondVar *cv = (ScoopSyncCondVar *)condvar_obj;
  if (cv->destroyed || !cv->initialized) {
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
  if (cv->destroyed || !cv->initialized) {
    return;
  }
  scoop_platform_sync_condvar_broadcast(&cv->cond);
}

void scoop_sync_condvar_destroy(void *condvar_obj) {
  if (condvar_obj == 0) {
    return;
  }

  scoop_thread_register();

  scoop_sync_condvar_destroy_impl((ScoopSyncCondVar *)condvar_obj);
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
  uint32_t init_flags;
  ScoopPlatformThread owner;
} ScoopSyncOnce;

enum {
  SCOOP_SYNC_ONCE_INIT_FLAG_LOCK = 1u << 0,
  SCOOP_SYNC_ONCE_INIT_FLAG_COND = 1u << 1,
};

static void scoop_sync_once_destroy_impl(ScoopSyncOnce *o) {
  if (o == 0 || o->init_flags == 0u) {
    return;
  }

  if ((o->init_flags & (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_COND) != 0u) {
    scoop_platform_sync_condvar_destroy(&o->cond);
    o->init_flags &= ~(uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_COND;
  }
  if ((o->init_flags & (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_LOCK) != 0u) {
    scoop_platform_sync_mutex_destroy(&o->lock);
    o->init_flags &= ~(uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_LOCK;
  }
  o->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_UNINITIALIZED;
  (void)memset(&o->owner, 0, sizeof(o->owner));
  (void)__atomic_fetch_add(&scoop_test_sync_once_destroy_calls, 1u, __ATOMIC_SEQ_CST);
}

static void scoop_sync_once_release(void *object) {
  scoop_sync_once_destroy_impl((ScoopSyncOnce *)object);
}

static const ScoopTypeDescriptor SCOOP_SYNC_ONCE_TYPE_DESC = {
    .abi_version = 0,
    .flags = 0,
    .size_bytes = sizeof(ScoopSyncOnce),
    .align_bytes = (uint64_t)_Alignof(ScoopSyncOnce),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = 0,
    .release_fn = scoop_sync_once_release,
    .type_id = 0,
    .parent_type_desc = 0,
    .itable = 0,
    .vtable = 0,
};

void *scoop_sync_once_create(void) {
  scoop_thread_register();

  ScoopSyncOnce *o =
      (ScoopSyncOnce *)scoop_alloc_typed(&SCOOP_SYNC_ONCE_TYPE_DESC, (uint64_t)sizeof(ScoopSyncOnce));
  if (o == 0) {
    return 0;
  }

  o->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_UNINITIALIZED;
  o->init_flags = 0u;
  (void)memset(&o->owner, 0, sizeof(o->owner));
  if (!scoop_platform_sync_mutex_init(&o->lock)) {
    return 0;
  }
  o->init_flags |= (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_LOCK;
  if (!scoop_platform_sync_condvar_init(&o->cond)) {
    scoop_platform_sync_mutex_destroy(&o->lock);
    o->init_flags = 0u;
    return 0;
  }
  o->init_flags |= (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_COND;
  return (void *)o;
}

bool scoop_sync_once_is_done(void *once_obj) {
  if (once_obj == 0) {
    return false;
  }

  scoop_thread_register();

  ScoopSyncOnce *o = (ScoopSyncOnce *)once_obj;
  if ((o->init_flags & (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_LOCK) == 0u) {
    return false;
  }
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
  if ((o->init_flags & ((uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_LOCK |
                        (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_COND)) !=
      ((uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_LOCK | (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_COND)) {
    return;
  }
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
    // T0105: Transition to IN_NATIVE while blocking on condvar_wait.
    scoop_enter_native(0, 0);
    while (o->state == (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZING) {
      scoop_platform_sync_condvar_wait(&o->cond, &o->lock);
    }
    scoop_leave_native();
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
