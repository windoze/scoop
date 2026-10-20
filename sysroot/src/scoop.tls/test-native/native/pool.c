/* Occupy the real libuv pool while TLS transport continues on the event loop.
 */
#include <pthread.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdlib.h>

typedef struct {
  pthread_mutex_t mutex;
  pthread_cond_t changed;
  unsigned started;
  bool released;
  atomic_uint references;
} HeldPool;

unsigned scoop_test_tls_pool_size(void) {
  const char *value = getenv("UV_THREADPOOL_SIZE");
  unsigned count = value ? (unsigned)atoi(value) : 4;
  return count == 0 ? 1 : count > 1024 ? 1024 : count;
}

void *scoop_test_tls_pool_new(void) {
  HeldPool *pool = calloc(1, sizeof(*pool));
  if (!pool || pthread_mutex_init(&pool->mutex, NULL) != 0 ||
      pthread_cond_init(&pool->changed, NULL) != 0)
    abort();
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
    pthread_cond_destroy(&pool->changed);
    pthread_mutex_destroy(&pool->mutex);
    free(pool);
  }
}

static void execute(void *value) {
  HeldPool *pool = value;
  pthread_mutex_lock(&pool->mutex);
  pool->started++;
  pthread_cond_broadcast(&pool->changed);
  while (!pool->released)
    pthread_cond_wait(&pool->changed, &pool->mutex);
  pthread_mutex_unlock(&pool->mutex);
}

void scoop_test_tls_pool_wait(HeldPool *pool) {
  pthread_mutex_lock(&pool->mutex);
  while (pool->started != scoop_test_tls_pool_size())
    pthread_cond_wait(&pool->changed, &pool->mutex);
  pthread_mutex_unlock(&pool->mutex);
}

void scoop_test_tls_pool_close(HeldPool *pool) {
  pthread_mutex_lock(&pool->mutex);
  pool->released = true;
  pthread_cond_broadcast(&pool->changed);
  pthread_mutex_unlock(&pool->mutex);
  drop(pool);
}

typedef void (*WorkFunction)(void *);
WorkFunction scoop_test_tls_pool_executor(void) { return execute; }
WorkFunction scoop_test_tls_pool_releaser(void) { return drop; }
