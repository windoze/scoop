#include "../../native/linux/backend.h"
#include <errno.h>
#include <linux/filter.h>
#include <linux/seccomp.h>
#include <poll.h>
#include <stddef.h>
#include <sys/prctl.h>
#include <sys/syscall.h>
#include <sys/wait.h>
#include <unistd.h>

static int deny_syscall(int number) {
    struct sock_filter rules[] = {
        BPF_STMT(BPF_LD | BPF_W | BPF_ABS, offsetof(struct seccomp_data, nr)),
        BPF_JUMP(BPF_JMP | BPF_JEQ | BPF_K, (unsigned)number, 0, 1),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ERRNO | EPERM),
        BPF_STMT(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),
    };
    struct sock_fprog program = {.len = sizeof(rules) / sizeof(rules[0]), .filter = rules};
    return prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) < 0                 ? -errno
           : prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, &program) < 0 ? -errno
                                                                      : 0;
}
static int wait_child(pid_t child) {
    if (child < 0)
        return -errno;
    int status;
    pid_t result;
    do {
        result = waitpid(child, &status, 0);
    } while (result < 0 && errno == EINTR);
    if (result < 0)
        return -errno;
    return WIFEXITED(status) ? WEXITSTATUS(status) : -EIO;
}
int scoop_test_ring_denied(void) {
    pid_t child = fork();
    if (!child) {
        if (deny_syscall(__NR_io_uring_setup))
            _exit(2);
        ScoopIoCreated created = scoop_io_loop_create();
        _exit(created.status.code == EPERM && !created.pointer ? 0 : 3);
    }
    return wait_child(child);
}
int scoop_test_ring_submit_denied(void) {
    pid_t child = fork();
    if (!child) {
        ScoopIoCreated created = scoop_io_loop_create();
        if (created.status.code)
            _exit(2);
        ScoopIoLoop *loop = created.pointer;
        if (scoop_io_timer_start(loop, 1, 1000, 0).code || deny_syscall(__NR_io_uring_enter))
            _exit(3);
        if (scoop_io_loop_poll(loop, false).code != EPERM)
            _exit(4);
        if (!loop->requests || loop->requests->entries[0].state != RING_PUBLISHED ||
            scoop_io_loop_idle(loop) || scoop_io_loop_batch_size(loop))
            _exit(5);
        if (scoop_io_loop_poll(loop, false).code)
            _exit(6);
        struct pollfd notification = {.fd = loop->notification, .events = POLLIN};
        if (poll(&notification, 1, 0))
            _exit(7);
        // The request cannot be retired without a CQE. Process exit owns this test's final
        // teardown.
        _exit(0);
    }
    return wait_child(child);
}

static void unexpected_work(ScoopIoWork *work) {
    (void)work;
    _exit(6);
}
static void unexpected_finish(ScoopIoWork *work, int status) {
    (void)work;
    (void)status;
    _exit(7);
}
int scoop_test_workers_denied(void) {
    pid_t child = fork();
    if (!child) {
        ScoopIoCreated created = scoop_io_loop_create();
        if (created.status.code)
            _exit(2);
        ScoopIoLoop *loop = created.pointer;
        if (deny_syscall(__NR_clone) || deny_syscall(__NR_clone3))
            _exit(3);
        ScoopIoWork work = {0};
        for (unsigned attempt = 0; attempt < 2; attempt++) {
            if (scoop_io_work_submit(&work, loop, 1, unexpected_work, unexpected_finish) >= 0 ||
                loop->workers || loop->workers_started || loop->work)
                _exit(4);
        }
        if (scoop_io_timer_start(loop, 2, 0, 1).code)
            _exit(5);
        while (!scoop_io_loop_batch_size(loop)) {
            if (scoop_io_loop_poll(loop, true).code)
                _exit(8);
        }
        if (scoop_io_loop_next(loop).id != 2 || scoop_io_loop_destroy(loop).code)
            _exit(9);
        _exit(0);
    }
    return wait_child(child);
}
