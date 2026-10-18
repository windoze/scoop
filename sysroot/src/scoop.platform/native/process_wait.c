#define _GNU_SOURCE 1
#include <errno.h>
#include <signal.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <sys/wait.h>

int64_t scoop_platform_process_wait(int32_t child, bool no_hang) {
    int status;
    pid_t result;
    do {
        result = waitpid((pid_t)child, &status, no_hang ? WNOHANG : 0);
    } while (result < 0 && errno == EINTR);
    if (result < 0)
        return errno == ECHILD ? -3 : -1;
    if (result == 0)
        return -2;
    return WIFEXITED(status) ? WEXITSTATUS(status) : 256 + WTERMSIG(status);
}

int scoop_platform_process_signal(int32_t child, bool force) {
    int result = kill((pid_t)child, force ? SIGKILL : SIGTERM);
    return result < 0 && errno == ESRCH ? 0 : result;
}

void scoop_platform_process_abandon(int32_t child) { (void)waitpid((pid_t)child, NULL, WNOHANG); }
