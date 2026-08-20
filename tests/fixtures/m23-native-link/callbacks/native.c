#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>

typedef int32_t (*Callback)(int32_t, void *);

int32_t m23_callback_invoke(Callback callback, void *context, int32_t value) {
    return callback(value, context);
}

typedef struct {
    Callback callback;
    void *context;
    int32_t value;
    int32_t result;
} Job;

static void *worker(void *opaque) {
    Job *job = opaque;
    job->result = job->callback(job->value, job->context);
    return NULL;
}

int32_t m23_callback_join(Callback callback, void *context, int32_t value) {
    Job job = {callback, context, value, 0};
    pthread_t thread;
    if (pthread_create(&thread, NULL, worker, &job) != 0 ||
        pthread_join(thread, NULL) != 0) {
        abort();
    }
    return job.result;
}
