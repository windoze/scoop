#include <errno.h>

#include "parallel_mark_fixture.h"

#undef pthread_create
#undef pthread_join
extern int pthread_create(pthread_t *, const pthread_attr_t *, void *(*)(void *), void *);
extern int pthread_join(pthread_t, void **);

static size_t attempted, created, joined;

int scoop_test_gc_pthread_create(pthread_t *thread, const pthread_attr_t *attributes,
                                 void *(*entry)(void *), void *context) {
    if (++attempted == 3) {
        return EAGAIN;
    }
    int result = pthread_create(thread, attributes, entry, context);
    created += result == 0;
    return result;
}

int scoop_test_gc_pthread_join(pthread_t thread, void **result) {
    int status = pthread_join(thread, result);
    joined += status == 0;
    return status;
}

int main(void) {
    ParallelFixture fixture = {0};
    parallel_start(&fixture, 8);
    parallel_check_full(1, PARALLEL_NODE_COUNT + 4, PARALLEL_NODE_COUNT / 1024);
    assert(attempted == 3 && created == 2 && joined == 2);
    assert(nursery_metrics().worker_creation_failures == 1);
    parallel_check_full(1, PARALLEL_NODE_COUNT + 4, PARALLEL_NODE_COUNT / 1024);
    assert(attempted == 3 && joined == 2);
    parallel_finish(&fixture);
    puts("worker creation failure joins partial startup and preserves serial marking");
}
