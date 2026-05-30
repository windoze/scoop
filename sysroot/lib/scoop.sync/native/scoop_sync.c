// Native implementation for the `scoop.sync` sysroot cone.
//
// The public Scoop API is ordinary Scoop wrapper code. This file owns the
// platform resources behind `Mutex`, `CondVar`, and the user-visible `Once`.
// New entry points operate on raw native handles; the legacy GC-object helpers
// remain until the follow-up runtime cleanup removes their descriptors.

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include <scoop_runtime.h>

#if defined(_WIN32)
typedef struct ScoopSyncNativeMutex {
  uint64_t _storage[8];
} ScoopSyncNativeMutex;

typedef struct ScoopSyncNativeCondVar {
  uint64_t _storage[8];
} ScoopSyncNativeCondVar;

typedef uint64_t ScoopSyncNativeThread;

static int scoop_sync_native_mutex_init(ScoopSyncNativeMutex *mutex) {
  (void)mutex;
  return 0;
}

static void scoop_sync_native_mutex_lock(ScoopSyncNativeMutex *mutex) { (void)mutex; }

static void scoop_sync_native_mutex_unlock(ScoopSyncNativeMutex *mutex) { (void)mutex; }

static void scoop_sync_native_mutex_destroy(ScoopSyncNativeMutex *mutex) { (void)mutex; }

static int scoop_sync_native_condvar_init(ScoopSyncNativeCondVar *condvar) {
  (void)condvar;
  return 0;
}

static void scoop_sync_native_condvar_wait(ScoopSyncNativeCondVar *condvar,
                                           ScoopSyncNativeMutex *mutex) {
  (void)condvar;
  (void)mutex;
}

static void scoop_sync_native_condvar_signal(ScoopSyncNativeCondVar *condvar) {
  (void)condvar;
}

static void scoop_sync_native_condvar_broadcast(ScoopSyncNativeCondVar *condvar) {
  (void)condvar;
}

static void scoop_sync_native_condvar_destroy(ScoopSyncNativeCondVar *condvar) {
  (void)condvar;
}

static ScoopSyncNativeThread scoop_sync_native_thread_self(void) { return 0; }

static int scoop_sync_native_thread_equal(ScoopSyncNativeThread a, ScoopSyncNativeThread b) {
  (void)a;
  (void)b;
  return 0;
}
#else
#include <pthread.h>

typedef pthread_mutex_t ScoopSyncNativeMutex;
typedef pthread_cond_t ScoopSyncNativeCondVar;
typedef pthread_t ScoopSyncNativeThread;

static int scoop_sync_native_mutex_init(ScoopSyncNativeMutex *mutex) {
  if (mutex == 0) {
    return 0;
  }
  return pthread_mutex_init(mutex, 0) == 0;
}

static void scoop_sync_native_mutex_lock(ScoopSyncNativeMutex *mutex) {
  if (mutex == 0) {
    return;
  }
  (void)pthread_mutex_lock(mutex);
}

static void scoop_sync_native_mutex_unlock(ScoopSyncNativeMutex *mutex) {
  if (mutex == 0) {
    return;
  }
  (void)pthread_mutex_unlock(mutex);
}

static void scoop_sync_native_mutex_destroy(ScoopSyncNativeMutex *mutex) {
  if (mutex == 0) {
    return;
  }
  (void)pthread_mutex_destroy(mutex);
}

static int scoop_sync_native_condvar_init(ScoopSyncNativeCondVar *condvar) {
  if (condvar == 0) {
    return 0;
  }
  return pthread_cond_init(condvar, 0) == 0;
}

static void scoop_sync_native_condvar_wait(ScoopSyncNativeCondVar *condvar,
                                           ScoopSyncNativeMutex *mutex) {
  if (condvar == 0 || mutex == 0) {
    return;
  }
  (void)pthread_cond_wait(condvar, mutex);
}

static void scoop_sync_native_condvar_signal(ScoopSyncNativeCondVar *condvar) {
  if (condvar == 0) {
    return;
  }
  (void)pthread_cond_signal(condvar);
}

static void scoop_sync_native_condvar_broadcast(ScoopSyncNativeCondVar *condvar) {
  if (condvar == 0) {
    return;
  }
  (void)pthread_cond_broadcast(condvar);
}

static void scoop_sync_native_condvar_destroy(ScoopSyncNativeCondVar *condvar) {
  if (condvar == 0) {
    return;
  }
  (void)pthread_cond_destroy(condvar);
}

static ScoopSyncNativeThread scoop_sync_native_thread_self(void) { return pthread_self(); }

static int scoop_sync_native_thread_equal(ScoopSyncNativeThread a, ScoopSyncNativeThread b) {
  return pthread_equal(a, b) ? 1 : 0;
}
#endif

// Optional test-cone hooks. Hidden weak no-op definitions keep ordinary
// `scoop.sync` users free of test exports, while `scoop.runtime.test` can
// override them when that cone is explicitly linked.
#if defined(__clang__) || defined(__GNUC__)
#define SCOOP_SYNC_TEST_HOOK __attribute__((weak, visibility("hidden")))
SCOOP_SYNC_TEST_HOOK void scoop_runtime_test_sync_mutex_destroyed(void) {}
SCOOP_SYNC_TEST_HOOK void scoop_runtime_test_sync_condvar_destroyed(void) {}
SCOOP_SYNC_TEST_HOOK void scoop_runtime_test_sync_once_destroyed(void) {}
#else
static void scoop_runtime_test_sync_mutex_destroyed(void) {}
static void scoop_runtime_test_sync_condvar_destroyed(void) {}
static void scoop_runtime_test_sync_once_destroyed(void) {}
#endif

static void scoop_sync_test_mutex_destroyed(void) {
  scoop_runtime_test_sync_mutex_destroyed();
}

static void scoop_sync_test_condvar_destroyed(void) {
  scoop_runtime_test_sync_condvar_destroyed();
}

static void scoop_sync_test_once_destroyed(void) {
  scoop_runtime_test_sync_once_destroyed();
}

// --- Mutex ---

typedef struct ScoopSyncMutex {
  ScoopObjectHeader header;
  void *native;
} ScoopSyncMutex;

typedef struct ScoopSyncMutexNative {
  ScoopSyncNativeMutex mutex;
  uint32_t destroyed;
  uint32_t initialized;
} ScoopSyncMutexNative;

static ScoopSyncMutexNative *scoop_sync_mutex_native_from_handle(void *handle) {
  if (handle == 0) {
    return 0;
  }
  return (ScoopSyncMutexNative *)handle;
}

static void scoop_sync_mutex_native_destroy_impl(ScoopSyncMutexNative *native) {
  if (native == 0 || native->destroyed || !native->initialized) {
    return;
  }

  native->destroyed = 1;
  scoop_sync_native_mutex_destroy(&native->mutex);
  native->initialized = 0;
  scoop_sync_test_mutex_destroyed();
  free(native);
}

void *scoop_sync_mutex_native_create(void) {
  ScoopSyncMutexNative *native =
      (ScoopSyncMutexNative *)malloc(sizeof(ScoopSyncMutexNative));
  if (native == 0) {
    return 0;
  }

  native->destroyed = 1;
  native->initialized = 0;
  if (!scoop_sync_native_mutex_init(&native->mutex)) {
    free(native);
    return 0;
  }
  native->destroyed = 0;
  native->initialized = 1;
  return (void *)native;
}

void scoop_sync_mutex_native_lock(void *handle) {
  ScoopSyncMutexNative *native = scoop_sync_mutex_native_from_handle(handle);
  if (native == 0 || native->destroyed || !native->initialized) {
    return;
  }
  scoop_sync_native_mutex_lock(&native->mutex);
}

void scoop_sync_mutex_native_unlock(void *handle) {
  ScoopSyncMutexNative *native = scoop_sync_mutex_native_from_handle(handle);
  if (native == 0 || native->destroyed || !native->initialized) {
    return;
  }
  scoop_sync_native_mutex_unlock(&native->mutex);
}

void scoop_sync_mutex_native_destroy(void *handle) {
  scoop_sync_mutex_native_destroy_impl(scoop_sync_mutex_native_from_handle(handle));
}

void *scoop_sync_mutex_native_null(void) { return 0; }

static ScoopSyncMutexNative *scoop_sync_mutex_native(ScoopSyncMutex *m) {
  if (m == 0 || m->native == 0) {
    return 0;
  }
  return (ScoopSyncMutexNative *)m->native;
}

static void scoop_sync_mutex_destroy_impl(ScoopSyncMutex *m) {
  ScoopSyncMutexNative *native = scoop_sync_mutex_native(m);
  scoop_sync_mutex_native_destroy_impl(native);
  if (m != 0) {
    m->native = 0;
  }
}

static void scoop_sync_mutex_release(void *object) {
  scoop_sync_mutex_destroy_impl((ScoopSyncMutex *)object);
}

static const ScoopTypeDescriptor SCOOP_SYNC_MUTEX_TYPE_DESC = {
    .abi_version = SCOOP_RUNTIME_ABI_VERSION,
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
  scoop_gc_thread_attach_current();

  ScoopSyncMutex *m = (ScoopSyncMutex *)scoop_alloc_typed(
      &SCOOP_SYNC_MUTEX_TYPE_DESC, (uint64_t)sizeof(ScoopSyncMutex));
  if (m == 0) {
    return 0;
  }

  m->native = 0;

  ScoopSyncMutexNative *native =
      (ScoopSyncMutexNative *)malloc(sizeof(ScoopSyncMutexNative));
  if (native == 0) {
    return 0;
  }

  native->destroyed = 1;
  native->initialized = 0;
  if (!scoop_sync_native_mutex_init(&native->mutex)) {
    free(native);
    return 0;
  }
  native->destroyed = 0;
  native->initialized = 1;
  m->native = native;
  return (void *)m;
}

void scoop_sync_mutex_lock(void *mutex_obj) {
  if (mutex_obj == 0) {
    return;
  }

  scoop_gc_thread_attach_current();

  ScoopSyncMutexNative *native = scoop_sync_mutex_native((ScoopSyncMutex *)mutex_obj);
  if (native == 0 || native->destroyed || !native->initialized) {
    return;
  }

  scoop_enter_native(0, 0);
  scoop_sync_native_mutex_lock(&native->mutex);
  scoop_leave_native();
}

void scoop_sync_mutex_unlock(void *mutex_obj) {
  if (mutex_obj == 0) {
    return;
  }

  scoop_gc_thread_attach_current();

  ScoopSyncMutexNative *native = scoop_sync_mutex_native((ScoopSyncMutex *)mutex_obj);
  if (native == 0 || native->destroyed || !native->initialized) {
    return;
  }
  scoop_sync_native_mutex_unlock(&native->mutex);
}

void scoop_sync_mutex_destroy(void *mutex_obj) {
  if (mutex_obj == 0) {
    return;
  }

  scoop_gc_thread_attach_current();

  scoop_sync_mutex_destroy_impl((ScoopSyncMutex *)mutex_obj);
}

// --- CondVar ---

typedef struct ScoopSyncCondVar {
  ScoopObjectHeader header;
  void *native;
} ScoopSyncCondVar;

typedef struct ScoopSyncCondVarNative {
  ScoopSyncNativeCondVar cond;
  uint32_t destroyed;
  uint32_t initialized;
} ScoopSyncCondVarNative;

static ScoopSyncCondVarNative *scoop_sync_condvar_native_from_handle(void *handle) {
  if (handle == 0) {
    return 0;
  }
  return (ScoopSyncCondVarNative *)handle;
}

static void scoop_sync_condvar_native_destroy_impl(ScoopSyncCondVarNative *native) {
  if (native == 0 || native->destroyed || !native->initialized) {
    return;
  }

  native->destroyed = 1;
  scoop_sync_native_condvar_destroy(&native->cond);
  native->initialized = 0;
  scoop_sync_test_condvar_destroyed();
  free(native);
}

void *scoop_sync_condvar_native_create(void) {
  ScoopSyncCondVarNative *native =
      (ScoopSyncCondVarNative *)malloc(sizeof(ScoopSyncCondVarNative));
  if (native == 0) {
    return 0;
  }

  native->destroyed = 1;
  native->initialized = 0;
  if (!scoop_sync_native_condvar_init(&native->cond)) {
    free(native);
    return 0;
  }
  native->destroyed = 0;
  native->initialized = 1;
  return (void *)native;
}

void scoop_sync_condvar_native_wait(void *condvar_handle, void *mutex_handle) {
  ScoopSyncCondVarNative *cv = scoop_sync_condvar_native_from_handle(condvar_handle);
  ScoopSyncMutexNative *m = scoop_sync_mutex_native_from_handle(mutex_handle);
  if (cv == 0 || m == 0 || cv->destroyed || !cv->initialized || m->destroyed || !m->initialized) {
    return;
  }
  scoop_sync_native_condvar_wait(&cv->cond, &m->mutex);
}

void scoop_sync_condvar_native_notify_one(void *handle) {
  ScoopSyncCondVarNative *cv = scoop_sync_condvar_native_from_handle(handle);
  if (cv == 0 || cv->destroyed || !cv->initialized) {
    return;
  }
  scoop_sync_native_condvar_signal(&cv->cond);
}

void scoop_sync_condvar_native_notify_all(void *handle) {
  ScoopSyncCondVarNative *cv = scoop_sync_condvar_native_from_handle(handle);
  if (cv == 0 || cv->destroyed || !cv->initialized) {
    return;
  }
  scoop_sync_native_condvar_broadcast(&cv->cond);
}

void scoop_sync_condvar_native_destroy(void *handle) {
  scoop_sync_condvar_native_destroy_impl(scoop_sync_condvar_native_from_handle(handle));
}

void *scoop_sync_condvar_native_null(void) { return 0; }

static ScoopSyncCondVarNative *scoop_sync_condvar_native(ScoopSyncCondVar *cv) {
  if (cv == 0 || cv->native == 0) {
    return 0;
  }
  return (ScoopSyncCondVarNative *)cv->native;
}

static void scoop_sync_condvar_destroy_impl(ScoopSyncCondVar *cv) {
  ScoopSyncCondVarNative *native = scoop_sync_condvar_native(cv);
  scoop_sync_condvar_native_destroy_impl(native);
  if (cv != 0) {
    cv->native = 0;
  }
}

static void scoop_sync_condvar_release(void *object) {
  scoop_sync_condvar_destroy_impl((ScoopSyncCondVar *)object);
}

static const ScoopTypeDescriptor SCOOP_SYNC_CONDVAR_TYPE_DESC = {
    .abi_version = SCOOP_RUNTIME_ABI_VERSION,
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
  scoop_gc_thread_attach_current();

  ScoopSyncCondVar *cv = (ScoopSyncCondVar *)scoop_alloc_typed(
      &SCOOP_SYNC_CONDVAR_TYPE_DESC, (uint64_t)sizeof(ScoopSyncCondVar));
  if (cv == 0) {
    return 0;
  }

  cv->native = 0;

  ScoopSyncCondVarNative *native =
      (ScoopSyncCondVarNative *)malloc(sizeof(ScoopSyncCondVarNative));
  if (native == 0) {
    return 0;
  }

  native->destroyed = 1;
  native->initialized = 0;
  if (!scoop_sync_native_condvar_init(&native->cond)) {
    free(native);
    return 0;
  }
  native->destroyed = 0;
  native->initialized = 1;
  cv->native = native;
  return (void *)cv;
}

void scoop_sync_condvar_wait(void *condvar_obj, void *mutex_obj) {
  if (condvar_obj == 0 || mutex_obj == 0) {
    return;
  }

  scoop_gc_thread_attach_current();

  ScoopSyncCondVarNative *cv = scoop_sync_condvar_native((ScoopSyncCondVar *)condvar_obj);
  ScoopSyncMutexNative *m = scoop_sync_mutex_native((ScoopSyncMutex *)mutex_obj);
  if (cv == 0 || m == 0 || cv->destroyed || !cv->initialized || m->destroyed || !m->initialized) {
    return;
  }

  scoop_enter_native(0, 0);
  scoop_sync_native_condvar_wait(&cv->cond, &m->mutex);
  scoop_leave_native();
}

void scoop_sync_condvar_notify_one(void *condvar_obj) {
  if (condvar_obj == 0) {
    return;
  }

  scoop_gc_thread_attach_current();

  ScoopSyncCondVarNative *cv = scoop_sync_condvar_native((ScoopSyncCondVar *)condvar_obj);
  if (cv == 0 || cv->destroyed || !cv->initialized) {
    return;
  }
  scoop_sync_native_condvar_signal(&cv->cond);
}

void scoop_sync_condvar_notify_all(void *condvar_obj) {
  if (condvar_obj == 0) {
    return;
  }

  scoop_gc_thread_attach_current();

  ScoopSyncCondVarNative *cv = scoop_sync_condvar_native((ScoopSyncCondVar *)condvar_obj);
  if (cv == 0 || cv->destroyed || !cv->initialized) {
    return;
  }
  scoop_sync_native_condvar_broadcast(&cv->cond);
}

void scoop_sync_condvar_destroy(void *condvar_obj) {
  if (condvar_obj == 0) {
    return;
  }

  scoop_gc_thread_attach_current();

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
  ScoopObjectHeader header;
  void *native;
} ScoopSyncOnce;

typedef struct ScoopSyncOnceNative {
  ScoopSyncNativeMutex lock;
  ScoopSyncNativeCondVar cond;
  uint32_t state;
  uint32_t init_flags;
  ScoopSyncNativeThread owner;
} ScoopSyncOnceNative;

enum {
  SCOOP_SYNC_ONCE_BEGIN_SKIP = 0,
  SCOOP_SYNC_ONCE_BEGIN_WAIT = 1,
  SCOOP_SYNC_ONCE_BEGIN_RUN = 2,
};

static ScoopSyncOnceNative *scoop_sync_once_native_from_handle(void *handle) {
  if (handle == 0) {
    return 0;
  }
  return (ScoopSyncOnceNative *)handle;
}

void *scoop_sync_once_native_create(void) {
  ScoopSyncOnceNative *native =
      (ScoopSyncOnceNative *)malloc(sizeof(ScoopSyncOnceNative));
  if (native == 0) {
    return 0;
  }
  native->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_UNINITIALIZED;
  native->init_flags = 0u;
  (void)memset(&native->owner, 0, sizeof(native->owner));
  return (void *)native;
}

intptr_t scoop_sync_once_native_is_done(void *handle) {
  ScoopSyncOnceNative *native = scoop_sync_once_native_from_handle(handle);
  if (native == 0) {
    return 0;
  }
  return native->state == (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZED ? 1 : 0;
}

intptr_t scoop_sync_once_native_begin(void *handle) {
  ScoopSyncOnceNative *native = scoop_sync_once_native_from_handle(handle);
  if (native == 0) {
    return SCOOP_SYNC_ONCE_BEGIN_SKIP;
  }

  uint32_t state = native->state;
  if (state == (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZED) {
    return SCOOP_SYNC_ONCE_BEGIN_SKIP;
  }

  ScoopSyncNativeThread self = scoop_sync_native_thread_self();
  if (state == (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZING) {
    if (scoop_sync_native_thread_equal(native->owner, self)) {
      return SCOOP_SYNC_ONCE_BEGIN_SKIP;
    }
    return SCOOP_SYNC_ONCE_BEGIN_WAIT;
  }

  native->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZING;
  native->owner = self;
  return SCOOP_SYNC_ONCE_BEGIN_RUN;
}

void scoop_sync_once_native_complete(void *handle) {
  ScoopSyncOnceNative *native = scoop_sync_once_native_from_handle(handle);
  if (native == 0) {
    return;
  }
  native->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZED;
  (void)memset(&native->owner, 0, sizeof(native->owner));
}

void scoop_sync_once_native_destroy(void *handle) {
  ScoopSyncOnceNative *native = scoop_sync_once_native_from_handle(handle);
  if (native == 0) {
    return;
  }
  native->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_UNINITIALIZED;
  (void)memset(&native->owner, 0, sizeof(native->owner));
  scoop_sync_test_once_destroyed();
  free(native);
}

static ScoopSyncOnceNative *scoop_sync_once_native(ScoopSyncOnce *o) {
  if (o == 0 || o->native == 0) {
    return 0;
  }
  return (ScoopSyncOnceNative *)o->native;
}

enum {
  SCOOP_SYNC_ONCE_INIT_FLAG_LOCK = 1u << 0,
  SCOOP_SYNC_ONCE_INIT_FLAG_COND = 1u << 1,
};

static void scoop_sync_once_destroy_impl(ScoopSyncOnce *o) {
  ScoopSyncOnceNative *native = scoop_sync_once_native(o);
  if (native == 0 || native->init_flags == 0u) {
    return;
  }

  if ((native->init_flags & (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_COND) != 0u) {
    scoop_sync_native_condvar_destroy(&native->cond);
    native->init_flags &= ~(uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_COND;
  }
  if ((native->init_flags & (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_LOCK) != 0u) {
    scoop_sync_native_mutex_destroy(&native->lock);
    native->init_flags &= ~(uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_LOCK;
  }
  native->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_UNINITIALIZED;
  (void)memset(&native->owner, 0, sizeof(native->owner));
  free(native);
  o->native = 0;
  scoop_sync_test_once_destroyed();
}

static void scoop_sync_once_release(void *object) {
  scoop_sync_once_destroy_impl((ScoopSyncOnce *)object);
}

static const ScoopTypeDescriptor SCOOP_SYNC_ONCE_TYPE_DESC = {
    .abi_version = SCOOP_RUNTIME_ABI_VERSION,
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
  scoop_gc_thread_attach_current();

  ScoopSyncOnce *o =
      (ScoopSyncOnce *)scoop_alloc_typed(&SCOOP_SYNC_ONCE_TYPE_DESC, (uint64_t)sizeof(ScoopSyncOnce));
  if (o == 0) {
    return 0;
  }

  o->native = 0;

  ScoopSyncOnceNative *native =
      (ScoopSyncOnceNative *)malloc(sizeof(ScoopSyncOnceNative));
  if (native == 0) {
    return 0;
  }

  native->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_UNINITIALIZED;
  native->init_flags = 0u;
  (void)memset(&native->owner, 0, sizeof(native->owner));
  if (!scoop_sync_native_mutex_init(&native->lock)) {
    free(native);
    return 0;
  }
  native->init_flags |= (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_LOCK;
  if (!scoop_sync_native_condvar_init(&native->cond)) {
    scoop_sync_native_mutex_destroy(&native->lock);
    native->init_flags = 0u;
    free(native);
    return 0;
  }
  native->init_flags |= (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_COND;
  o->native = native;
  return (void *)o;
}

bool scoop_sync_once_is_done(void *once_obj) {
  if (once_obj == 0) {
    return false;
  }

  scoop_gc_thread_attach_current();

  ScoopSyncOnceNative *o = scoop_sync_once_native((ScoopSyncOnce *)once_obj);
  if (o == 0 || (o->init_flags & (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_LOCK) == 0u) {
    return false;
  }
  scoop_sync_native_mutex_lock(&o->lock);
  bool done = (o->state == (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZED);
  scoop_sync_native_mutex_unlock(&o->lock);
  return done;
}

void scoop_sync_once_run(void *once_obj, void *env_ptr, ScoopSyncOnceInitFn fn) {
  if (once_obj == 0 || fn == 0) {
    return;
  }

  scoop_gc_thread_attach_current();

  ScoopSyncOnceNative *o = scoop_sync_once_native((ScoopSyncOnce *)once_obj);
  if (o == 0 || (o->init_flags & ((uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_LOCK |
                         (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_COND)) !=
      ((uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_LOCK | (uint32_t)SCOOP_SYNC_ONCE_INIT_FLAG_COND)) {
    return;
  }
  ScoopSyncNativeThread self = scoop_sync_native_thread_self();

  scoop_sync_native_mutex_lock(&o->lock);

  uint32_t state = o->state;
  if (state == (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZED) {
    scoop_sync_native_mutex_unlock(&o->lock);
    return;
  }

  if (state == (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZING) {
    if (scoop_sync_native_thread_equal(o->owner, self)) {
      scoop_sync_native_mutex_unlock(&o->lock);
      return;
    }

    scoop_enter_native(0, 0);
    while (o->state == (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZING) {
      scoop_sync_native_condvar_wait(&o->cond, &o->lock);
    }
    scoop_leave_native();
    scoop_sync_native_mutex_unlock(&o->lock);
    return;
  }

  o->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZING;
  o->owner = self;
  scoop_sync_native_mutex_unlock(&o->lock);

  fn(env_ptr);

  scoop_sync_native_mutex_lock(&o->lock);
  o->state = (uint32_t)SCOOP_SYNC_ONCE_STATE_INITIALIZED;
  scoop_sync_native_condvar_broadcast(&o->cond);
  scoop_sync_native_mutex_unlock(&o->lock);
}
