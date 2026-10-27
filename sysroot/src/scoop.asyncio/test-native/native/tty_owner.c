#include "../../../scoop.platform/native/tty/terminal.h"
#include <errno.h>
#include <fcntl.h>
#include <sys/wait.h>
#include <unistd.h>
#ifdef __APPLE__
#include <util.h>
#else
#include <pty.h>
#endif

int scoop_test_tty_controlling_owner(void) {
    pid_t child = fork();
    if (child < 0)
        return -errno;
    if (!child) {
        if (setsid() < 0)
            _exit(2);
        int master, slave;
        if (openpty(&master, &slave, NULL, NULL, NULL) < 0 || dup2(slave, STDIN_FILENO) < 0)
            _exit(3);
        scoop_terminal *terminal = scoop_terminal_open();
        if (!terminal)
            _exit(4);
        int controlling = open("/dev/tty", O_RDWR | O_NOCTTY);
        if (controlling >= 0)
            _exit(5);
        if (errno != ENXIO)
            _exit(6);
        if (scoop_terminal_close(terminal) < 0)
            _exit(7);
        close(slave);
        close(master);
        _exit(0);
    }
    int status;
    pid_t result;
    do {
        result = waitpid(child, &status, 0);
    } while (result < 0 && errno == EINTR);
    if (result < 0)
        return -errno;
    return WIFEXITED(status) ? WEXITSTATUS(status) : -EINTR;
}
