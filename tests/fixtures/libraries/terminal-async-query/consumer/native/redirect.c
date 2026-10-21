#include <fcntl.h>
#include <unistd.h>

static int saved_input = -1, saved_output = -1;

int m59_query_redirect(void) {
    saved_input = dup(STDIN_FILENO);
    saved_output = dup(STDOUT_FILENO);
    int empty = open("/dev/null", O_RDWR);
    if (saved_input < 0 || saved_output < 0 || empty < 0)
        return -1;
    int result = dup2(empty, STDIN_FILENO);
    if (result >= 0)
        result = dup2(empty, STDOUT_FILENO);
    close(empty);
    return result < 0 ? -1 : 0;
}

int m59_query_restore(void) {
    int result = dup2(saved_input, STDIN_FILENO);
    if (result >= 0)
        result = dup2(saved_output, STDOUT_FILENO);
    close(saved_input);
    close(saved_output);
    return result < 0 ? -1 : 0;
}
