/* Occupy the real asyncio worker pool while TLS transport continues on the
 * event loop.
 */
#include <stdatomic.h>
#include <stdbool.h>
#include <stdlib.h>
#ifdef _WIN32
#include "../../../scoop.platform/native/windows_api.h"
#define pool_lock(pool) AcquireSRWLockExclusive(&(pool)->mutex)
#define pool_unlock(pool) ReleaseSRWLockExclusive(&(pool)->mutex)
#define pool_wait(pool)                                                        \
  SleepConditionVariableSRW(&(pool)->changed, &(pool)->mutex, INFINITE, 0)
#define pool_notify(pool) WakeAllConditionVariable(&(pool)->changed)
#else
#include <pthread.h>
#define pool_lock(pool) pthread_mutex_lock(&(pool)->mutex)
#define pool_unlock(pool) pthread_mutex_unlock(&(pool)->mutex)
#define pool_wait(pool) pthread_cond_wait(&(pool)->changed, &(pool)->mutex)
#define pool_notify(pool) pthread_cond_broadcast(&(pool)->changed)
#endif

typedef struct {
#ifdef _WIN32
  SRWLOCK mutex;
  CONDITION_VARIABLE changed;
#else
  pthread_mutex_t mutex;
  pthread_cond_t changed;
#endif
  unsigned started;
  bool released;
  atomic_uint references;
} HeldPool;

extern unsigned scoop_io_work_width(void);

unsigned scoop_test_tls_pool_size(void) { return scoop_io_work_width(); }

void *scoop_test_tls_pool_new(void) {
  HeldPool *pool = calloc(1, sizeof(*pool));
  if (!pool)
    abort();
#ifdef _WIN32
  InitializeSRWLock(&pool->mutex);
  InitializeConditionVariable(&pool->changed);
#else
  if (pthread_mutex_init(&pool->mutex, NULL) != 0 ||
      pthread_cond_init(&pool->changed, NULL) != 0)
    abort();
#endif
  atomic_init(&pool->references, 1);
  return pool;
}

void *scoop_test_tls_pool_retain(HeldPool *pool) {
  atomic_fetch_add_explicit(&pool->references, 1, memory_order_relaxed);
  return pool;
}

static void drop(void *value) {
  HeldPool *pool = value;
  if (atomic_fetch_sub_explicit(&pool->references, 1, memory_order_acq_rel) ==
      1) {
#ifndef _WIN32
    pthread_cond_destroy(&pool->changed);
    pthread_mutex_destroy(&pool->mutex);
#endif
    free(pool);
  }
}

static void execute(void *value) {
  HeldPool *pool = value;
  pool_lock(pool);
  pool->started++;
  pool_notify(pool);
  while (!pool->released)
    pool_wait(pool);
  pool_unlock(pool);
}

void scoop_test_tls_pool_wait(HeldPool *pool) {
  pool_lock(pool);
  while (pool->started != scoop_test_tls_pool_size())
    pool_wait(pool);
  pool_unlock(pool);
}

void scoop_test_tls_pool_close(HeldPool *pool) {
  pool_lock(pool);
  pool->released = true;
  pool_notify(pool);
  pool_unlock(pool);
  drop(pool);
}

typedef void (*WorkFunction)(void *);
WorkFunction scoop_test_tls_pool_executor(void) { return execute; }
WorkFunction scoop_test_tls_pool_releaser(void) { return drop; }
