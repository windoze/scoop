#include <pthread.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct {
  pthread_mutex_t mutex;
  pthread_cond_t changed;
  pthread_t caller;
  int started, finished, released, foreign;
  bool allowed;
} TestWorkProbe;

typedef struct {
  TestWorkProbe *probe;
  int result;
} TestWork;

void *scoop_test_native_probe_new(bool allowed) {
  TestWorkProbe *probe = calloc(1, sizeof(*probe));
  if (!probe || pthread_mutex_init(&probe->mutex, NULL) != 0 ||
      pthread_cond_init(&probe->changed, NULL) != 0)
    abort();
  probe->caller = pthread_self();
  probe->allowed = allowed;
  return probe;
}

void scoop_test_native_probe_free(TestWorkProbe *probe) {
  pthread_cond_destroy(&probe->changed);
  pthread_mutex_destroy(&probe->mutex);
  free(probe);
}

void *scoop_test_native_work_new(TestWorkProbe *probe) {
  TestWork *work = calloc(1, sizeof(*work));
  if (!work)
    abort();
  work->probe = probe;
  return work;
}

static void execute(void *value) {
  TestWork *work = value;
  TestWorkProbe *probe = work->probe;
  pthread_mutex_lock(&probe->mutex);
  probe->started++;
  probe->foreign = !pthread_equal(probe->caller, pthread_self());
  pthread_cond_broadcast(&probe->changed);
  while (!probe->allowed)
    pthread_cond_wait(&probe->changed, &probe->mutex);
  work->result = 42;
  probe->finished++;
  pthread_mutex_unlock(&probe->mutex);
}

static void release(void *value) {
  TestWork *work = value;
  pthread_mutex_lock(&work->probe->mutex);
  work->probe->released++;
  pthread_mutex_unlock(&work->probe->mutex);
  free(work);
}

typedef void (*WorkFunction)(void *);
WorkFunction scoop_test_native_execute(void) { return execute; }
WorkFunction scoop_test_native_release(void) { return release; }
int scoop_test_native_result(TestWork *work) { return work->result; }

void scoop_test_native_wait(TestWorkProbe *probe) {
  pthread_mutex_lock(&probe->mutex);
  while (!probe->started)
    pthread_cond_wait(&probe->changed, &probe->mutex);
  pthread_mutex_unlock(&probe->mutex);
}

void scoop_test_native_allow(TestWorkProbe *probe) {
  pthread_mutex_lock(&probe->mutex);
  probe->allowed = true;
  pthread_cond_broadcast(&probe->changed);
  pthread_mutex_unlock(&probe->mutex);
}

int scoop_test_native_count(TestWorkProbe *probe, int kind) {
  pthread_mutex_lock(&probe->mutex);
  int value = kind == 0   ? probe->started
              : kind == 1 ? probe->finished
              : kind == 2 ? probe->released
                          : probe->foreign;
  pthread_mutex_unlock(&probe->mutex);
  return value;
}
