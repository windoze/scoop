#include "platform/platform.h"
#include <assert.h>
#include <errno.h>
#include <poll.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/event.h>
#include <sys/ioctl.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
#include <util.h>

extern const ScoopThreadVmOps scoop_darwin_thread_vm_ops;
static const ScoopThreadVmOps *const vm = &scoop_darwin_thread_vm_ops;
static volatile sig_atomic_t calls, signal_ok, mask_ok;

static void resize_handler(int signal, siginfo_t *info, void *context) {
    (void)context;
    int saved = errno;
    sigset_t mask;
    mask_ok = sigprocmask(SIG_SETMASK, NULL, &mask) == 0 && sigismember(&mask, SIGUSR1) == 1;
    signal_ok = signal == SIGWINCH && info != NULL;
    ++calls;
    errno = saved;
}

static void *worker(void *unused) {
    (void)unused;
    const struct timespec delay = {1, 0};
    for (;;)
        nanosleep(&delay, NULL);
    return NULL;
}

static void discard_while_resizing(int ready) {
    alarm(20);
    pthread_t threads[8];
    for (size_t index = 0; index < sizeof threads / sizeof *threads; ++index)
        assert(pthread_create(&threads[index], NULL, worker, NULL) == 0);
    size_t page = vm->page_size(), size = 4 * 1024 * 1024;
    ScoopPlatformError error = {0};
    void *mapping = NULL;
    assert(vm->reserve_read_write(size, page, &mapping, &error));
    unsigned char *bytes = mapping;
    assert(!vm->discard_pages(NULL, page, &error));
    assert(!vm->discard_pages(bytes, 0, &error));
    assert(!vm->discard_pages(bytes + 1, page, &error));
    memset(mapping, 93, size);
    for (unsigned repeat = 0; repeat < 30; ++repeat) {
        calls = signal_ok = mask_ok = 0;
        struct sigaction action = {0};
        action.sa_sigaction = resize_handler;
        action.sa_flags = SA_SIGINFO | SA_RESTART | SA_NODEFER | SA_RESETHAND;
        assert(sigemptyset(&action.sa_mask) == 0);
        assert(sigaddset(&action.sa_mask, SIGUSR1) == 0);
        assert(sigaction(SIGWINCH, &action, NULL) == 0);
        int queue = kqueue();
        assert(queue >= 0);
        struct kevent change, event;
        EV_SET(&change, SIGWINCH, EVFILT_SIGNAL, EV_ADD | EV_CLEAR, 0, 0, NULL);
        assert(kevent(queue, &change, 1, NULL, 0, NULL) == 0);
        assert(write(ready, "r", 1) == 1);
        // Overlap asynchronous delivery with the same VM operation used by full GC.
        for (unsigned sample = 0; sample < 100; ++sample)
            assert(vm->discard_pages(bytes + page, size - 2 * page, &error));
        struct pollfd watched = {.fd = queue, .events = POLLIN};
        int result;
        do {
            result = poll(&watched, 1, 5000);
        } while (result < 0 && errno == EINTR);
        assert(result == 1);
        const struct timespec immediate = {0, 0};
        do {
            result = kevent(queue, NULL, 0, &event, 1, &immediate);
        } while (result < 0 && errno == EINTR);
        assert(result == 1 && event.filter == EVFILT_SIGNAL && event.ident == SIGWINCH);
        assert(close(queue) == 0);
        const struct timespec delay = {0, 1000000};
        for (unsigned attempt = 0; attempt < 1000 && calls == 0; ++attempt)
            nanosleep(&delay, NULL);
        assert(calls == 1 && signal_ok && mask_ok);
        assert(raise(SIGWINCH) == 0);
        assert(calls == 1);
        for (size_t offset = 0; offset < page; ++offset)
            assert(bytes[offset] == 93 && bytes[size - page + offset] == 93);
        // Discard is advisory; initialize the reused pages before reading them.
        memset(bytes + page, 42, size - 2 * page);
        assert(bytes[page] == 42 && bytes[size - page - 1] == 42);
    }
    assert(vm->release_mapping(mapping, size, &error));
}

int main(void) {
    alarm(25);
    int ready[2], master;
    assert(pipe(ready) == 0);
    int diagnostic = dup(STDERR_FILENO);
    assert(diagnostic >= 0);
    struct winsize initial = {.ws_row = 24, .ws_col = 80};
    pid_t child = forkpty(&master, NULL, NULL, &initial);
    assert(child >= 0);
    if (child == 0) {
        assert(dup2(diagnostic, STDERR_FILENO) >= 0);
        close(diagnostic);
        close(ready[0]);
        discard_while_resizing(ready[1]);
        _exit(0);
    }
    close(diagnostic);
    close(ready[1]);
    char byte;
    unsigned short step = 0;
    ssize_t received;
    while ((received = read(ready[0], &byte, 1)) == 1) {
        struct winsize next = {.ws_row = 12, .ws_col = (unsigned short)(30 + step++)};
        assert(ioctl(master, TIOCSWINSZ, &next) == 0);
    }
    assert(received == 0);
    close(ready[0]);
    int status;
    assert(waitpid(child, &status, 0) == child);
    close(master);
    assert(WIFEXITED(status) && WEXITSTATUS(status) == 0);
    puts("page discard, reuse and one-shot signal delivery passed");
    return 0;
}
