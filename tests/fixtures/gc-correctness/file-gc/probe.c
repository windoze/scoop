#define _GNU_SOURCE 1
#include <assert.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

typedef int32_t (*Callback)(int32_t, void *);
static Callback callback;
static void *context;
static _Atomic int closes, directory_closes, fail_close;
static pthread_mutex_t read_mutex = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t read_condition = PTHREAD_COND_INITIALIZER;
static int concurrent, readers;

void m45_file_callback(Callback value, void *cookie) {
    callback = value;
    context = cookie;
}
void m45_file_clear_callback(void) {
    callback = NULL;
    context = NULL;
}
int m45_file_closes(void) { return atomic_load(&closes); }
int m45_directory_closes(void) { return atomic_load(&directory_closes); }
void m45_file_fail_close(void) { atomic_store(&fail_close, 1); }

ssize_t m45_file_read(int fd, void *data, size_t count) {
    assert(pthread_mutex_lock(&read_mutex) == 0);
    if (concurrent) {
        ++readers;
        assert(pthread_cond_broadcast(&read_condition) == 0);
        while (readers != 2)
            assert(pthread_cond_wait(&read_condition, &read_mutex) == 0);
    }
    assert(pthread_mutex_unlock(&read_mutex) == 0);
    if (callback != NULL)
        assert(callback(1, context) == 0);
    assert(fcntl(fd, F_GETFD) >= 0);
    return read(fd, data, count);
}

ssize_t m45_file_write(int fd, const void *data, size_t count) {
    if (callback != NULL)
        assert(callback(2, context) == 0);
    assert(fcntl(fd, F_GETFD) >= 0);
    return write(fd, data, count);
}

int m45_file_close(int fd) {
    atomic_fetch_add(&closes, 1);
    int result = close(fd);
    if (atomic_exchange(&fail_close, 0)) {
        assert(result == 0);
        errno = EINTR;
        return -1;
    }
    return result;
}

struct dirent *m45_directory_next(DIR *directory) {
    int before = atomic_load(&directory_closes);
    if (callback != NULL)
        assert(callback(3, context) == 0);
    assert(atomic_load(&directory_closes) == before);
    return readdir(directory);
}

int m45_directory_close(DIR *directory) {
    atomic_fetch_add(&directory_closes, 1);
    return closedir(directory);
}

/* Compare Scoop path handling with the filesystem's actual raw-byte support. */
int m45_native_raw_path(const char *bytes, int64_t length) {
    assert(length >= 0);
    char *path = malloc((size_t)length + 1);
    assert(path != NULL);
    memcpy(path, bytes, (size_t)length);
    path[length] = '\0';
    int fd = open(path, O_CREAT | O_EXCL | O_WRONLY | O_CLOEXEC, 0600);
    int error = fd < 0 ? errno : 0;
    if (fd >= 0) {
        assert(close(fd) == 0);
        assert(unlink(path) == 0);
    }
    free(path);
    return error;
}

struct Worker {
    Callback function;
    void *context;
    int result;
};

static void *run_worker(void *argument) {
    struct Worker *worker = argument;
    worker->result = worker->function(0, worker->context);
    return NULL;
}

int m45_file_concurrent(Callback function, void *cookie) {
    struct Worker workers[2] = {{function, cookie, 0}, {function, cookie, 0}};
    pthread_t threads[2];
    concurrent = 1;
    readers = 0;
    for (int index = 0; index < 2; ++index)
        assert(pthread_create(&threads[index], NULL, run_worker, &workers[index]) == 0);
    for (int index = 0; index < 2; ++index)
        assert(pthread_join(threads[index], NULL) == 0);
    concurrent = 0;
    return workers[0].result + workers[1].result;
}
