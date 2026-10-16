#define _GNU_SOURCE 1
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static void write_all(int fd, const char *bytes, size_t length) {
    while (length != 0) {
        ssize_t count = write(fd, bytes, length);
        if (count < 0 && errno == EINTR)
            continue;
        assert(count > 0);
        bytes += count;
        length -= (size_t)count;
    }
}

int main(int argc, char **argv) {
    assert(argc >= 2);
    if (strcmp(argv[1], "check") == 0) {
        assert(argc == 6 && argv[2][0] == '\0');
        assert(strcmp(argv[3], "a ' b") == 0);
        assert((unsigned char)argv[4][0] == 255 && argv[4][1] == '\0');
        assert((unsigned char)getenv("RAW")[0] == 254 && getenv("RAW")[1] == '\0');
        assert(getenv("EMPTY")[0] == '\0');
        char *directory = getcwd(NULL, 0);
        assert(directory != NULL && strcmp(directory, argv[5]) == 0);
        free(directory);
        sigset_t mask;
        assert(sigprocmask(SIG_SETMASK, NULL, &mask) == 0 && !sigismember(&mask, SIGPIPE));
        struct sigaction action;
        assert(sigaction(SIGPIPE, NULL, &action) == 0 && action.sa_handler == SIG_DFL);
        for (int fd = 0; fd < 3; ++fd)
            assert(fcntl(fd, F_GETFD) == 0);
        for (int fd = 3; fd < 64; ++fd)
            assert(fcntl(fd, F_GETFD) < 0 && errno == EBADF);
        puts("checked");
    } else if (strcmp(argv[1], "echo") == 0) {
        char bytes[4096];
        for (;;) {
            ssize_t count = read(0, bytes, sizeof(bytes));
            if (count < 0 && errno == EINTR)
                continue;
            assert(count >= 0);
            if (count == 0)
                break;
            write_all(1, bytes, (size_t)count);
            write_all(2, bytes, (size_t)count);
        }
    } else if (strcmp(argv[1], "stop") == 0) {
        assert(raise(SIGSTOP) == 0);
    } else {
        assert(strcmp(argv[1], "exit") == 0);
    }
    return 0;
}
