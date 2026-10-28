#define _GNU_SOURCE 1
#define _DARWIN_C_SOURCE 1
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/ioctl.h>
#include <sys/wait.h>
#include <termios.h>
#include <time.h>
#include <unistd.h>

extern char **environ;

static struct termios child_modes;

int scoop_readline_test_capture_modes(void) { return tcgetattr(STDIN_FILENO, &child_modes); }

bool scoop_readline_test_current_modes_restored(void) {
    struct termios actual;
    return tcgetattr(STDIN_FILENO, &actual) == 0 && actual.c_iflag == child_modes.c_iflag &&
           actual.c_oflag == child_modes.c_oflag && actual.c_cflag == child_modes.c_cflag &&
           (actual.c_lflag & ~PENDIN) == (child_modes.c_lflag & ~PENDIN) &&
           memcmp(actual.c_cc, child_modes.c_cc, NCCS) == 0;
}

typedef struct {
    int master, slave;
    pid_t child;
    struct termios original;
} test_pty;

void scoop_readline_test_pty_close(test_pty *pty) {
    close(pty->master);
    close(pty->slave);
    if (pty->child > 0) {
        kill(pty->child, SIGKILL);
        while (waitpid(pty->child, NULL, 0) < 0 && errno == EINTR) {
        }
    }
    free(pty);
}

test_pty *scoop_readline_test_pty_open(char *executable, char *selector) {
    test_pty *pty = calloc(1, sizeof(*pty));
    if (!pty)
        return NULL;
    pty->slave = -1;
    pty->master = posix_openpt(O_RDWR | O_NOCTTY | O_CLOEXEC);
    char **environment = NULL;
    if (pty->master < 0 || grantpt(pty->master) < 0 || unlockpt(pty->master) < 0)
        goto fail;
    const char *name = ptsname(pty->master);
    if (!name)
        goto fail;
    pty->slave = open(name, O_RDWR | O_NOCTTY | O_CLOEXEC);
    if (pty->slave < 0 || tcgetattr(pty->slave, &pty->original) < 0)
        goto fail;
    struct winsize size = {.ws_row = 8, .ws_col = 24};
    if (ioctl(pty->slave, TIOCSWINSZ, &size) < 0)
        goto fail;
    size_t count = 0;
    while (environ[count])
        ++count;
    environment = calloc(count + 2, sizeof(char *));
    if (!environment)
        goto fail;
    size_t used = 0;
    for (size_t i = 0; i < count; ++i) {
        if (strncmp(environ[i], "SCOOP_TEST_", 11) && strncmp(environ[i], "TERM=", 5))
            environment[used++] = environ[i];
    }
    environment[used] = "TERM=xterm-256color";
    char *arguments[] = {executable, "--scoop-test-filter-ignored", selector, NULL};
    pty->child = fork();
    if (pty->child < 0)
        goto fail;
    if (pty->child == 0) {
        if (setsid() < 0 || ioctl(pty->slave, TIOCSCTTY, 0) < 0 || dup2(pty->slave, 0) < 0 ||
            dup2(pty->slave, 1) < 0 || dup2(pty->slave, 2) < 0)
            _exit(125);
        close(pty->master);
        close(pty->slave);
        execve(executable, arguments, environment);
        _exit(126);
    }
    free(environment);
    return pty;
fail: {
    int error = errno;
    free(environment);
    scoop_readline_test_pty_close(pty);
    errno = error;
    return NULL;
}
}

int64_t scoop_readline_test_pty_read(test_pty *pty, void *bytes, int64_t count) {
    struct pollfd pollfd = {.fd = pty->master, .events = POLLIN};
    int status;
    do {
        status = poll(&pollfd, 1, 25);
    } while (status < 0 && errno == EINTR);
    if (status <= 0)
        return status;
    ssize_t result;
    do {
        result = read(pty->master, bytes, (size_t)count);
    } while (result < 0 && errno == EINTR);
    return result;
}

int64_t scoop_readline_test_pty_write(test_pty *pty, const void *bytes, int64_t count) {
    ssize_t result;
    do {
        result = write(pty->master, bytes, (size_t)count);
    } while (result < 0 && errno == EINTR);
    return result;
}

int scoop_readline_test_pty_wait(test_pty *pty) {
    close(pty->slave);
    pty->slave = -1;
    for (int i = 0; i < 200; ++i) {
        int status;
        pid_t result = waitpid(pty->child, &status, WNOHANG);
        if (result == pty->child) {
            pty->child = 0;
            return WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status);
        }
        if (result < 0 && errno != EINTR)
            return -1;
        struct timespec pause = {.tv_nsec = 25000000};
        nanosleep(&pause, NULL);
    }
    errno = ETIMEDOUT;
    return -1;
}

bool scoop_readline_test_pty_restored(test_pty *pty) {
    struct termios actual = {0};
    if (tcgetattr(pty->slave, &actual) < 0) {
        fprintf(stderr, "PTY tcgetattr: %s\n", strerror(errno));
        return false;
    }
    bool restored = actual.c_iflag == pty->original.c_iflag &&
                    actual.c_oflag == pty->original.c_oflag &&
                    actual.c_cflag == pty->original.c_cflag &&
                    (actual.c_lflag & ~PENDIN) == (pty->original.c_lflag & ~PENDIN) &&
                    !memcmp(actual.c_cc, pty->original.c_cc, NCCS);
    if (!restored) {
        fprintf(stderr, "PTY flags before/after: %lx/%lx %lx/%lx %lx/%lx %lx/%lx\n",
                (unsigned long)pty->original.c_iflag, (unsigned long)actual.c_iflag,
                (unsigned long)pty->original.c_oflag, (unsigned long)actual.c_oflag,
                (unsigned long)pty->original.c_cflag, (unsigned long)actual.c_cflag,
                (unsigned long)pty->original.c_lflag, (unsigned long)actual.c_lflag);
        for (int i = 0; i < NCCS; ++i)
            if (actual.c_cc[i] != pty->original.c_cc[i])
                fprintf(stderr, "PTY cc[%d] before/after: %d/%d\n", i, pty->original.c_cc[i],
                        actual.c_cc[i]);
    }
    return restored;
}

int scoop_readline_test_pty_resize(test_pty *pty, int rows, int columns) {
    struct winsize size = {.ws_row = (unsigned short)rows, .ws_col = (unsigned short)columns};
    return ioctl(pty->master, TIOCSWINSZ, &size);
}

static void suspend_probe(int signal_number) {
    (void)signal_number;
    kill(getpid(), SIGSTOP);
}

int scoop_readline_test_suspend_probe(void) {
    struct sigaction action = {0};
    action.sa_handler = suspend_probe;
    sigemptyset(&action.sa_mask);
    return sigaction(SIGTSTP, &action, NULL);
}

int scoop_readline_test_pty_stopped(test_pty *pty) {
    for (int i = 0; i < 200; ++i) {
        int status;
        pid_t result = waitpid(pty->child, &status, WNOHANG | WUNTRACED);
        if (result == pty->child) {
            if (WIFSTOPPED(status))
                return 0;
            pty->child = 0;
            errno = ECHILD;
            return -1;
        }
        if (result < 0 && errno != EINTR)
            return -1;
        struct timespec pause = {.tv_nsec = 25000000};
        nanosleep(&pause, NULL);
    }
    errno = ETIMEDOUT;
    return -1;
}

int scoop_readline_test_pty_resume(test_pty *pty) { return kill(pty->child, SIGCONT); }
