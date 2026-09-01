#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>

typedef int64_t (*M13Callback)(int64_t value, void *context);
typedef void (*M13ContextFirstCallback)(void *context, int64_t value);

typedef struct M13Job {
    M13Callback callback;
    void *context;
    int64_t value;
    int64_t result;
    int go;
    pthread_t thread;
    pthread_mutex_t mutex;
    pthread_cond_t condition;
} M13Job;

typedef struct M13Group {
    int64_t count;
    M13Job **jobs;
} M13Group;

static void *m13_worker(void *raw_job) {
    M13Job *job = (M13Job *)raw_job;
    pthread_mutex_lock(&job->mutex);
    while (!job->go) {
        pthread_cond_wait(&job->condition, &job->mutex);
    }
    pthread_mutex_unlock(&job->mutex);
    job->result = job->callback(job->value, job->context);
    return NULL;
}

void *m13_start_one(M13Callback callback, void *context, int64_t value) {
    M13Job *job = (M13Job *)calloc(1, sizeof(M13Job));
    if (job == NULL) {
        abort();
    }
    job->callback = callback;
    job->context = context;
    job->value = value;
    pthread_mutex_init(&job->mutex, NULL);
    pthread_cond_init(&job->condition, NULL);
    if (pthread_create(&job->thread, NULL, m13_worker, job) != 0) {
        abort();
    }
    return job;
}

int64_t m13_join_one(void *raw_job) {
    M13Job *job = (M13Job *)raw_job;
    pthread_mutex_lock(&job->mutex);
    job->go = 1;
    pthread_cond_signal(&job->condition);
    pthread_mutex_unlock(&job->mutex);
    pthread_join(job->thread, NULL);
    int64_t result = job->result;
    pthread_cond_destroy(&job->condition);
    pthread_mutex_destroy(&job->mutex);
    free(job);
    return result;
}

int64_t m13_run_many(
    M13Callback callback,
    void *context,
    int64_t base,
    int64_t count
) {
    M13Job **jobs = (M13Job **)calloc((size_t)count, sizeof(M13Job *));
    if (jobs == NULL) {
        abort();
    }
    for (int64_t index = 0; index < count; index += 1) {
        jobs[index] = (M13Job *)m13_start_one(callback, context, base + index);
    }
    for (int64_t index = 0; index < count; index += 1) {
        pthread_mutex_lock(&jobs[index]->mutex);
        jobs[index]->go = 1;
        pthread_cond_signal(&jobs[index]->condition);
        pthread_mutex_unlock(&jobs[index]->mutex);
    }
    int64_t result = 0;
    for (int64_t index = 0; index < count; index += 1) {
        pthread_join(jobs[index]->thread, NULL);
        result += jobs[index]->result;
        pthread_cond_destroy(&jobs[index]->condition);
        pthread_mutex_destroy(&jobs[index]->mutex);
        free(jobs[index]);
    }
    free(jobs);
    return result;
}

int64_t m13_fail_start(M13Callback callback, void *context) {
    (void)callback;
    (void)context;
    return -1;
}

void *m13_start_many(
    M13Callback callback,
    void *context,
    int64_t base,
    int64_t count
) {
    if (count <= 0) {
        abort();
    }
    M13Group *group = (M13Group *)calloc(1, sizeof(M13Group));
    if (group == NULL) {
        abort();
    }
    group->count = count;
    group->jobs = (M13Job **)calloc((size_t)count, sizeof(M13Job *));
    if (group->jobs == NULL) {
        abort();
    }
    for (int64_t index = 0; index < count; index += 1) {
        group->jobs[index] =
            (M13Job *)m13_start_one(callback, context, base + index);
    }
    return group;
}

int64_t m13_join_many(void *raw_group) {
    M13Group *group = (M13Group *)raw_group;
    for (int64_t index = 0; index < group->count; index += 1) {
        pthread_mutex_lock(&group->jobs[index]->mutex);
        group->jobs[index]->go = 1;
        pthread_cond_signal(&group->jobs[index]->condition);
        pthread_mutex_unlock(&group->jobs[index]->mutex);
    }
    int64_t result = 0;
    for (int64_t index = 0; index < group->count; index += 1) {
        M13Job *job = group->jobs[index];
        pthread_join(job->thread, NULL);
        result += job->result;
        pthread_cond_destroy(&job->condition);
        pthread_mutex_destroy(&job->mutex);
        free(job);
    }
    free(group->jobs);
    free(group);
    return result;
}

void m13_call_context_first(
    M13ContextFirstCallback callback,
    void *context,
    int64_t value
) {
    callback(context, value);
}
