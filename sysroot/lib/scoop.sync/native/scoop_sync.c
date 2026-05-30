// Native implementation for the `scoop.sync` sysroot cone.
//
// The public Scoop API is ordinary Scoop wrapper code. This file owns only raw
// platform resources behind `Mutex`, `CondVar`, and the user-visible `Once`.

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

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

// --- CondVar ---

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

// --- Once ---

typedef enum ScoopSyncOnceStateU32 {
  SCOOP_SYNC_ONCE_STATE_UNINITIALIZED = 0u,
  SCOOP_SYNC_ONCE_STATE_INITIALIZING = 1u,
  SCOOP_SYNC_ONCE_STATE_INITIALIZED = 2u,
} ScoopSyncOnceStateU32;

typedef struct ScoopSyncOnceNative {
  uint32_t state;
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
