#include <assert.h>
#include <pthread.h>
#include <stdint.h>

#include "scoop_rt.h"

/* No runtime initialization or attachment: linking only the two pressure
 * sources also verifies that the production hint needs no runtime state. */
static void *notify(void *unused) {
    (void)unused;
    for (int i = 0; i < 10000; ++i) {
        scoop_rt_gc_adjust_pressure(INT64_MAX);
        scoop_rt_gc_adjust_pressure(INT64_MIN);
        scoop_rt_gc_adjust_pressure(0);
    }
    return NULL;
}

int main(void) {
    pthread_t workers[4];
    notify(NULL);
    for (int i = 0; i < 4; ++i)
        assert(pthread_create(&workers[i], NULL, notify, NULL) == 0);
    for (int i = 0; i < 4; ++i)
        assert(pthread_join(workers[i], NULL) == 0);
    return 0;
}
