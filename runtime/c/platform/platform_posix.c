// POSIX backend for Scoop C runtime platform layer (v0).
//
// 注意：
// - 本文件通过 `platform.h` 以 `#include` 方式被包含到 runtime 的编译单元中；
// - 所有 API 必须是 `static`，避免成为对外导出符号（见 T1401）。

#include <errno.h>
#include <pthread.h>
#include <sched.h>
#include <stdlib.h>
#include <time.h>
#include <sys/time.h>
#include <unistd.h>

#if defined(__linux__)
#include <sys/syscall.h>
#endif

static SCOOP_PLATFORM_UNUSED const char *scoop_platform_env_getenv(const char *key_cstr) {
  if (key_cstr == 0) {
    return 0;
  }
  return getenv(key_cstr);
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_time_now_unix_millis(int64_t *out_unix_millis) {
  if (out_unix_millis == 0) {
    return 0;
  }

  struct timeval tv;
  int ok = gettimeofday(&tv, 0);
  if (ok != 0) {
    return 0;
  }

  int64_t sec = (int64_t)tv.tv_sec;
  int64_t usec = (int64_t)tv.tv_usec;
  *out_unix_millis = (sec * 1000) + (usec / 1000);
  return 1;
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_io_write_all_fd(int fd,
                                                               const uint8_t *buf,
                                                               size_t len) {
  if (len == 0) {
    return 1;
  }
  if (buf == 0) {
    return 0;
  }

  const uint8_t *p = buf;
  size_t remaining = len;
  while (remaining > 0) {
    ssize_t n = write(fd, p, remaining);
    if (n < 0) {
      if (errno == EINTR) {
        continue;
      }
      return 0;
    }
    if (n == 0) {
      // `write` 返回 0 通常意味着不可恢复的错误（例如管道已关闭）。
      return 0;
    }

    p += (size_t)n;
    remaining -= (size_t)n;
  }

  return 1;
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_io_write_stdout_all(const uint8_t *buf,
                                                                    size_t len) {
  return scoop_platform_io_write_all_fd(1, buf, len);
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_io_write_stderr_all(const uint8_t *buf,
                                                                    size_t len) {
  return scoop_platform_io_write_all_fd(2, buf, len);
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_io_write_stdout_byte(uint8_t byte) {
  return scoop_platform_io_write_all_fd(1, &byte, 1);
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_io_write_stderr_byte(uint8_t byte) {
  return scoop_platform_io_write_all_fd(2, &byte, 1);
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_io_read_stdin(uint8_t *buf,
                                                             size_t len,
                                                             size_t *out_nread) {
  if (out_nread == 0) {
    return 0;
  }
  *out_nread = 0;

  if (len == 0) {
    return 1;
  }
  if (buf == 0) {
    return 0;
  }

  for (;;) {
    ssize_t n = read(0, buf, len);
    if (n < 0) {
      if (errno == EINTR) {
        continue;
      }
      return 0;
    }

    *out_nread = (size_t)n;
    return 1;
  }
}

// --- sync/thread (pthread) ---

static SCOOP_PLATFORM_UNUSED int scoop_platform_sync_mutex_init(ScoopPlatformMutex *mutex) {
  if (mutex == 0) {
    return 0;
  }
  return pthread_mutex_init(&mutex->inner, 0) == 0;
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_mutex_lock(ScoopPlatformMutex *mutex) {
  if (mutex == 0) {
    return;
  }
  (void)pthread_mutex_lock(&mutex->inner);
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_mutex_unlock(ScoopPlatformMutex *mutex) {
  if (mutex == 0) {
    return;
  }
  (void)pthread_mutex_unlock(&mutex->inner);
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_mutex_destroy(ScoopPlatformMutex *mutex) {
  if (mutex == 0) {
    return;
  }
  (void)pthread_mutex_destroy(&mutex->inner);
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_sync_condvar_init(ScoopPlatformCondVar *condvar) {
  if (condvar == 0) {
    return 0;
  }
  return pthread_cond_init(&condvar->inner, 0) == 0;
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_condvar_wait(ScoopPlatformCondVar *condvar,
                                                                  ScoopPlatformMutex *mutex) {
  if (condvar == 0 || mutex == 0) {
    return;
  }
  (void)pthread_cond_wait(&condvar->inner, &mutex->inner);
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_condvar_signal(ScoopPlatformCondVar *condvar) {
  if (condvar == 0) {
    return;
  }
  (void)pthread_cond_signal(&condvar->inner);
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_condvar_broadcast(
    ScoopPlatformCondVar *condvar) {
  if (condvar == 0) {
    return;
  }
  (void)pthread_cond_broadcast(&condvar->inner);
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_sync_condvar_destroy(ScoopPlatformCondVar *condvar) {
  if (condvar == 0) {
    return;
  }
  (void)pthread_cond_destroy(&condvar->inner);
}

static SCOOP_PLATFORM_UNUSED ScoopPlatformThread scoop_platform_thread_self(void) {
  return pthread_self();
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_thread_equal(ScoopPlatformThread a,
                                                            ScoopPlatformThread b) {
  return pthread_equal(a, b) ? 1 : 0;
}

// --- thread primitives (pthread) ---

static SCOOP_PLATFORM_UNUSED int scoop_platform_thread_spawn(ScoopPlatformThread *out_thread,
                                                            ScoopPlatformThreadEntryFn entry,
                                                            void *arg) {
  if (out_thread == 0 || entry == 0) {
    return 0;
  }
  return pthread_create(out_thread, 0, entry, arg) == 0;
}

static SCOOP_PLATFORM_UNUSED int scoop_platform_thread_join(ScoopPlatformThread thread) {
  return pthread_join(thread, 0) == 0 ? 1 : 0;
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_thread_yield(void) {
  (void)sched_yield();
}

static SCOOP_PLATFORM_UNUSED void scoop_platform_thread_sleep_millis(int64_t ms) {
  if (ms <= 0) {
    return;
  }

  int64_t sec = ms / 1000;
  int64_t nsec = (ms % 1000) * 1000000;

  // `nanosleep` 使用 `time_t/long`，这里做最小的宽度适配与容错。
  struct timespec ts;
  ts.tv_sec = (time_t)sec;
  ts.tv_nsec = (long)nsec;

  // 若被信号打断，则继续 sleep 剩余时间。
  while (nanosleep(&ts, &ts) != 0) {
    if (errno != EINTR) {
      break;
    }
  }
}

static SCOOP_PLATFORM_UNUSED int64_t scoop_platform_thread_current_id(void) {
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
