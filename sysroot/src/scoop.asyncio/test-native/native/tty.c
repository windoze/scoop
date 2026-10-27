#include "../../native/loop.h"
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <sys/ioctl.h>
#include <termios.h>
#include <unistd.h>
#ifdef __APPLE__
#include <util.h>
#else
#include <pty.h>
#endif

typedef struct {
    int master, slave, saved_input;
    struct termios original;
} TestTerminal;

ScoopIoCreated scoop_test_tty_create(void) {
    TestTerminal *terminal = calloc(1, sizeof(*terminal));
    if (!terminal)
        return (ScoopIoCreated){NULL, -ENOMEM};
    int master = -1, slave = -1, saved = -1;
    if (openpty(&master, &slave, NULL, NULL, NULL) < 0)
        goto failed;
    saved = dup(STDIN_FILENO);
    if (saved < 0 || tcgetattr(slave, &terminal->original) < 0)
        goto failed;
    if (fcntl(master, F_SETFD, FD_CLOEXEC) < 0 || fcntl(slave, F_SETFD, FD_CLOEXEC) < 0 ||
        fcntl(saved, F_SETFD, FD_CLOEXEC) < 0 || dup2(slave, STDIN_FILENO) < 0)
        goto failed;
    terminal->master = master;
    terminal->slave = slave;
    terminal->saved_input = saved;
    return (ScoopIoCreated){terminal, 0};
failed: {
    int error = errno;
    if (master >= 0)
        close(master);
    if (slave >= 0)
        close(slave);
    if (saved >= 0)
        close(saved);
    free(terminal);
    return (ScoopIoCreated){NULL, -error};
}
}

int scoop_test_tty_send(TestTerminal *terminal, int value) {
    unsigned char byte = (unsigned char)value;
    ssize_t count;
    do {
        count = write(terminal->master, &byte, 1);
    } while (count < 0 && errno == EINTR);
    return count == 1 ? 0 : -errno;
}

int scoop_test_tty_receive(TestTerminal *terminal) {
    struct pollfd wait = {.fd = terminal->master, .events = POLLIN};
    int status;
    do {
        status = poll(&wait, 1, 5000);
    } while (status < 0 && errno == EINTR);
    if (status <= 0)
        return status < 0 ? -errno : -ETIMEDOUT;
    unsigned char byte;
    ssize_t count;
    do {
        count = read(terminal->master, &byte, 1);
    } while (count < 0 && errno == EINTR);
    return count == 1 ? byte : -(count < 0 ? errno : EIO);
}

int scoop_test_tty_resize(TestTerminal *terminal, int columns, int rows) {
    struct winsize size = {.ws_col = (unsigned short)columns, .ws_row = (unsigned short)rows};
    if (ioctl(terminal->slave, TIOCSWINSZ, &size) < 0)
        return -errno;
    return kill(getpid(), SIGWINCH);
}

int scoop_test_tty_end_input(TestTerminal *terminal) {
    int fd = terminal->master;
    terminal->master = -1;
    return close(fd);
}

bool scoop_test_tty_restored(TestTerminal *terminal) {
    // musl fills the kernel control-character range; initialize unused libc slots too.
    struct termios current = {0};
    if (tcgetattr(terminal->slave, &current) < 0)
        return false;
    const struct termios *before = &terminal->original;
    tcflag_t local_mask = (tcflag_t)-1;
#ifdef __APPLE__
    /* XNU sets PENDIN when restoring canonical input; it is kernel queue state. */
    local_mask &= ~PENDIN;
#endif
    if (current.c_iflag != before->c_iflag || current.c_oflag != before->c_oflag ||
        current.c_cflag != before->c_cflag ||
        (current.c_lflag & local_mask) != (before->c_lflag & local_mask) ||
        cfgetispeed(&current) != cfgetispeed(before) ||
        cfgetospeed(&current) != cfgetospeed(before))
        return false;
    for (unsigned i = 0; i < NCCS; i++)
        if (current.c_cc[i] != before->c_cc[i])
            return false;
    return true;
}

void scoop_test_tty_close(TestTerminal *terminal) {
    if (dup2(terminal->saved_input, STDIN_FILENO) < 0)
        abort();
    close(terminal->saved_input);
    close(terminal->slave);
    if (terminal->master >= 0)
        close(terminal->master);
    free(terminal);
}
