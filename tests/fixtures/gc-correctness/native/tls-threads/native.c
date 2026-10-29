#include "../../../language/callback/threading.h"

#include <assert.h>
#include <stdint.h>

typedef int32_t (*Callback)(int32_t, void *);
typedef struct {
    Callback callback;
    void *context;
    int32_t first;
    int32_t result;
} Job;

static M13ThreadResult M13_THREAD_CALL worker(void *raw) {
    Job *job = raw;
    job->result = job->callback(job->first, job->context);
    job->result += job->callback(job->first + 1, job->context);
    return 0;
}

int32_t run_tls_workers(Callback callback, void *context) {
    Job jobs[2] = {{callback, context, 5, 0}, {callback, context, 9, 0}};
    M13Thread threads[2];
    for (int i = 0; i < 2; ++i) {
        assert(m13_thread_create(&threads[i], worker, &jobs[i]) == 0);
    }
    for (int i = 0; i < 2; ++i) {
        assert(m13_thread_join(threads[i]) == 0);
    }
    return jobs[0].result + jobs[1].result;
}
