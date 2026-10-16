#define _GNU_SOURCE 1
#include <assert.h>
#include <errno.h>
#include <pthread.h>
#include <pwd.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/random.h>
#include <time.h>
#include <unistd.h>

typedef int32_t (*Callback)(int32_t, void *);
static Callback callback;
static void *context;
static int entropy_mode, entropy_calls, home_mode, home_calls, fail_clock;

void m45_system_callback(Callback function, void *cookie) {
    callback = function;
    context = cookie;
}
void m45_system_clear_callback(void) {
    callback = NULL;
    context = NULL;
}
int64_t m45_entropy_mode(int mode) {
    entropy_mode = mode;
    entropy_calls = 0;
#ifdef __APPLE__
    return 256;
#else
    return 17;
#endif
}

static int entropy_step(void) {
    if (callback != NULL)
        assert(callback(0, context) == 0);
    ++entropy_calls;
    if (!entropy_mode)
        return 0;
    if (entropy_calls == 2)
        return 1;
    errno = entropy_calls == 1 ? EINTR : EIO;
    return -1;
}

#ifdef __APPLE__
int m45_getentropy(void *data, size_t size) {
    int step = entropy_step();
    if (step < 0)
        return -1;
    if (step == 0)
        return getentropy(data, size);
    memset(data, 90, size);
    return 0;
}
#else
ssize_t m45_getrandom(void *data, size_t size, unsigned flags) {
    int step = entropy_step();
    if (step < 0)
        return -1;
    if (step == 0)
        return getrandom(data, size, flags);
    assert(size > 17);
    memset(data, 90, 17);
    return 17;
}
#endif

void m45_home_mode(int mode) {
    assert(unsetenv("HOME") == 0);
    home_mode = mode;
    home_calls = 0;
}
int m45_getpwuid_r(uid_t uid, struct passwd *entry, char *buffer, size_t size,
                   struct passwd **result) {
    if (!home_mode)
        return getpwuid_r(uid, entry, buffer, size, result);
    if (home_mode == 3)
        return EIO;
    *result = NULL;
    if (home_mode == 2)
        return 0;
    if (++home_calls == 1)
        return ERANGE;
    assert(size > sizeof("/test/fallback"));
    strcpy(buffer, "/test/fallback");
    entry->pw_dir = buffer;
    *result = entry;
    return 0;
}

void m45_clock_failure(void) { fail_clock = 1; }
int m45_clock_gettime(clockid_t clock, struct timespec *value) {
    if (fail_clock) {
        fail_clock = 0;
        errno = EIO;
        return -1;
    }
    return clock_gettime(clock, value);
}

static pthread_mutex_t mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t condition = PTHREAD_COND_INITIALIZER;
static int sleep_mode, sleeping, collected, sleep_calls;
int m45_nanosleep(const struct timespec *duration, struct timespec *remaining) {
    if (sleep_mode) {
        assert(pthread_mutex_lock(&mutex) == 0);
        sleeping = 1;
        assert(pthread_cond_broadcast(&condition) == 0);
        while (!collected)
            assert(pthread_cond_wait(&condition, &mutex) == 0);
        assert(pthread_mutex_unlock(&mutex) == 0);
        if (++sleep_calls == 1) {
            errno = EINTR;
            return -1;
        }
    }
    return nanosleep(duration, remaining);
}

struct Worker {
    Callback function;
    void *context;
    int result;
};
static void *worker(void *argument) {
    struct Worker *state = argument;
    state->result = state->function(0, state->context);
    return NULL;
}
int m45_sleep_gc(Callback function, void *cookie) {
    struct Worker state = {function, cookie, 0};
    sleep_mode = 1;
    sleeping = collected = sleep_calls = 0;
    pthread_t thread;
    assert(pthread_create(&thread, NULL, worker, &state) == 0);
    assert(pthread_mutex_lock(&mutex) == 0);
    while (!sleeping)
        assert(pthread_cond_wait(&condition, &mutex) == 0);
    assert(pthread_mutex_unlock(&mutex) == 0);
    int result = function(1, cookie);
    assert(pthread_mutex_lock(&mutex) == 0);
    collected = 1;
    assert(pthread_cond_broadcast(&condition) == 0);
    assert(pthread_mutex_unlock(&mutex) == 0);
    assert(pthread_join(thread, NULL) == 0);
    assert(sleep_calls >= 1);
    sleep_mode = 0;
    return result + state.result;
}
