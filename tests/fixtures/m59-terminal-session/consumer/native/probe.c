#define _GNU_SOURCE 1
#include <errno.h>
#include <fcntl.h>
#include <unistd.h>

static int original_input = -1, original_output = -1;

int m59_stdin_nonblocking(void) {
    int flags = fcntl(STDIN_FILENO, F_GETFL);
    return flags < 0 ? -1 : (flags & O_NONBLOCK);
}

int m59_wait_ack(void) {
    char byte;
    int fd = original_input >= 0 ? original_input : STDIN_FILENO;
    ssize_t count;
    do {
        count = read(fd, &byte, 1);
    } while (count < 0 && errno == EINTR);
    return count == 1 && byte == '\n' ? 0 : -1;
}

int m59_redirect(void) {
    original_input = fcntl(STDIN_FILENO, F_DUPFD_CLOEXEC, 3);
    original_output = fcntl(STDOUT_FILENO, F_DUPFD_CLOEXEC, 3);
    int sink = open("/dev/null", O_RDWR | O_CLOEXEC);
    if (original_input < 0 || original_output < 0 || sink < 0)
        return -1;
    int input = dup2(sink, STDIN_FILENO);
    int output = dup2(sink, STDOUT_FILENO);
    int closed = close(sink);
    return input >= 0 && output >= 0 && closed == 0 ? 0 : -1;
}

int m59_restore(void) {
    int input = dup2(original_input, STDIN_FILENO);
    int output = dup2(original_output, STDOUT_FILENO);
    int first = close(original_input), second = close(original_output);
    original_input = original_output = -1;
    return input >= 0 && output >= 0 && first == 0 && second == 0 ? 0 : -1;
}
