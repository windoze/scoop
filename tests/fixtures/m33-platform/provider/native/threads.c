#include <errno.h>
#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>

typedef int32_t (*Callback)(void *);

typedef struct {
  pthread_t thread;
  Callback callback;
  void *context;
  int32_t result;
} Job;

static void *run(void *data) {
  Job *job = data;
  job->result = job->callback(job->context);
  return NULL;
}

void *m33_platform_spawn(Callback callback, void *context) {
  Job *job = malloc(sizeof(*job));
  if (job == NULL) {
    errno = ENOMEM;
    return NULL;
  }
  job->callback = callback;
  job->context = context;
  int error = pthread_create(&job->thread, NULL, run, job);
  if (error != 0) {
    free(job);
    errno = error;
    return NULL;
  }
  return job;
}

int32_t m33_platform_join(void *data) {
  Job *job = data;
  int error = pthread_join(job->thread, NULL);
  if (error != 0) {
    errno = error;
    return -1;
  }
  int32_t result = job->result;
  free(job);
  return result;
}
