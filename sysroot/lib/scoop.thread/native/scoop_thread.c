// Native implementation for the `scoop.thread` sysroot cone.
//
// User-level thread APIs live in this cone-local native source. Runtime core is
// only used for GC thread attachment, native transitions, allocation, and stable
// GC handle tokens.

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include <scoop_runtime.h>

#if defined(_WIN32)
typedef uint64_t ScoopThreadNativeThread;

static int scoop_thread_native_spawn(ScoopThreadNativeThread *out_thread,
                                     void *(*entry)(void *),
                                     void *arg) {
  (void)out_thread;
  (void)entry;
  (void)arg;
  return 0;
}

static int scoop_thread_native_join(ScoopThreadNativeThread thread) {
  (void)thread;
  return 0;
}

static void scoop_thread_native_yield(void) {}

static void scoop_thread_native_sleep_millis(int64_t ms) { (void)ms; }

static int64_t scoop_thread_native_current_id(void) { return 0; }
#else
#include <errno.h>
#include <pthread.h>
#include <sched.h>
#include <time.h>
#include <unistd.h>

#if defined(__linux__)
#include <sys/syscall.h>
#endif

typedef pthread_t ScoopThreadNativeThread;

static int scoop_thread_native_spawn(ScoopThreadNativeThread *out_thread,
                                     void *(*entry)(void *),
                                     void *arg) {
  if (out_thread == 0 || entry == 0) {
    return 0;
  }
  return pthread_create(out_thread, 0, entry, arg) == 0;
}

static int scoop_thread_native_join(ScoopThreadNativeThread thread) {
  return pthread_join(thread, 0) == 0 ? 1 : 0;
}

static void scoop_thread_native_yield(void) { (void)sched_yield(); }

static void scoop_thread_native_sleep_millis(int64_t ms) {
  if (ms <= 0) {
    return;
  }

  int64_t sec = ms / 1000;
  int64_t nsec = (ms % 1000) * 1000000;

  struct timespec ts;
  ts.tv_sec = (time_t)sec;
  ts.tv_nsec = (long)nsec;

  while (nanosleep(&ts, &ts) != 0) {
    if (errno != EINTR) {
      break;
    }
  }
}

static int64_t scoop_thread_native_current_id(void) {
#if defined(__APPLE__)
  uint64_t tid = 0;
  int rc = pthread_threadid_np(0, &tid);
  if (rc != 0 || tid == 0) {
    return 0;
  }
  return (int64_t)tid;
#elif defined(__linux__)
  long tid = syscall(SYS_gettid);
  if (tid <= 0) {
    return 0;
  }
  return (int64_t)tid;
#else
  return 0;
#endif
}
#endif

typedef struct ScoopThreadHandle {
  ScoopObjectHeader header;
  ScoopThreadNativeThread thread;
  uint32_t started;
  uint32_t joined;
} ScoopThreadHandle;

typedef struct ScoopThreadStartArgs {
  uintptr_t entry_handle_raw;
} ScoopThreadStartArgs;

static const ScoopTypeDescriptor SCOOP_THREAD_TYPE_DESC = {
    .abi_version = SCOOP_RUNTIME_ABI_VERSION,
    .flags = 0,
    .size_bytes = sizeof(ScoopThreadHandle),
    .align_bytes = (uint64_t)_Alignof(ScoopThreadHandle),
    .trace_start_offset_bytes = 0,
    .trace_bitmap_u64_len = 0,
    ._reserved_u32 = 0,
    .trace_bitmap = 0,
    .trace_fn = 0,
    .release_fn = 0,
    .type_id = 0,
    .parent_type_desc = 0,
    .itable = 0,
    .vtable = 0,
};

extern void scoop_thread_entry_trampoline(uintptr_t entry_handle_raw);

#if defined(_WIN32)
static void scoop_thread_init_current_if_present(void) {}
#elif defined(__APPLE__)
extern void scoop_thread_init_current(void) __attribute__((weak_import));
static void scoop_thread_init_current_if_present(void) {
  if (scoop_thread_init_current != 0) {
    scoop_thread_init_current();
  }
}
#elif defined(__GNUC__)
extern void scoop_thread_init_current(void) __attribute__((weak));
static void scoop_thread_init_current_if_present(void) {
  if (scoop_thread_init_current != 0) {
    scoop_thread_init_current();
  }
}
#else
extern void scoop_thread_init_current(void);
static void scoop_thread_init_current_if_present(void) { scoop_thread_init_current(); }
#endif

static void *scoop_thread_entry(void *arg) {
  if (arg == 0) {
    return 0;
  }

  ScoopThreadStartArgs *args = (ScoopThreadStartArgs *)arg;
  uintptr_t entry_handle_raw = args->entry_handle_raw;
  free(args);

  scoop_gc_thread_attach_current();
  // A GC may already be requested before this OS thread reaches managed code.
  scoop_gc_safepoint_poll();
  scoop_thread_init_current_if_present();
  scoop_gc_safepoint_poll();
  if (entry_handle_raw != 0) {
    scoop_thread_entry_trampoline(entry_handle_raw);
    (void)scoop_handle_drop((uint64_t)entry_handle_raw);
  }
  scoop_gc_thread_detach_current();
  return 0;
}

void *scoop_thread_spawn(uintptr_t entry_handle_raw) {
  scoop_gc_thread_attach_current();

  if (entry_handle_raw == 0) {
    return 0;
  }

  ScoopThreadHandle *t = (ScoopThreadHandle *)scoop_alloc_typed(
      &SCOOP_THREAD_TYPE_DESC, (uint64_t)sizeof(ScoopThreadHandle));
  if (t == 0) {
    (void)scoop_handle_drop((uint64_t)entry_handle_raw);
    return 0;
  }

  (void)memset(&t->thread, 0, sizeof(t->thread));
  t->started = 0;
  t->joined = 0;

  ScoopThreadStartArgs *args = (ScoopThreadStartArgs *)malloc(sizeof(ScoopThreadStartArgs));
  if (args == 0) {
    (void)scoop_handle_drop((uint64_t)entry_handle_raw);
    return 0;
  }
  args->entry_handle_raw = entry_handle_raw;

  // Keep the managed Thread handle rooted while pthread_create overlaps GC.
  void **native_root_slots[] = {(void **)&t};
  scoop_enter_native((void ***)native_root_slots, 1);
  int spawned = scoop_thread_native_spawn(&t->thread, scoop_thread_entry, (void *)args);
  scoop_leave_native();

  if (!spawned) {
    (void)scoop_handle_drop((uint64_t)entry_handle_raw);
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

  scoop_gc_thread_attach_current();

  ScoopThreadHandle *t = (ScoopThreadHandle *)thread_obj;
  if (!t->started || t->joined) {
    return;
  }

  t->joined = 1;
  scoop_enter_native(0, 0);
  (void)scoop_thread_native_join(t->thread);
  scoop_leave_native();
}

void scoop_thread_yield(void) {
  scoop_gc_thread_attach_current();
  scoop_gc_safepoint_poll();
  scoop_thread_native_yield();
}

void scoop_thread_sleep_millis(int64_t ms) {
  if (ms <= 0) {
    return;
  }

  scoop_gc_thread_attach_current();
  scoop_enter_native(0, 0);
  scoop_thread_native_sleep_millis(ms);
  scoop_leave_native();
}

int64_t scoop_thread_current_id(void) {
  scoop_gc_thread_attach_current();
  return scoop_thread_native_current_id();
}
