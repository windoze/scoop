#define _GNU_SOURCE 1
#include <errno.h>
#include <signal.h>
#include <stddef.h>

static volatile sig_atomic_t calls, mask_ok, signal_ok;
static int selected;

static void observed(int signal) {
    sigset_t mask;
    int saved = errno;
    calls++;
    mask_ok = sigprocmask(SIG_SETMASK, NULL, &mask) == 0 && sigismember(&mask, SIGUSR1);
    signal_ok = signal == SIGWINCH;
    errno = saved;
}

static void detailed(int signal, siginfo_t *info, void *context) {
    (void)context;
    observed(signal);
    signal_ok = signal_ok && info != NULL;
}

int m59_install_resize(int mode) {
    struct sigaction action = {0};
    selected = mode;
    calls = mask_ok = signal_ok = 0;
    sigemptyset(&action.sa_mask);
    sigaddset(&action.sa_mask, SIGUSR1);
    action.sa_flags = SA_RESTART | SA_NODEFER;
    if (mode == 1)
        action.sa_handler = observed;
    else if (mode == 3)
        action.sa_handler = SIG_IGN;
    else if (mode == 4)
        action.sa_handler = SIG_DFL;
    else {
        action.sa_sigaction = detailed;
        action.sa_flags |= SA_SIGINFO;
        if (mode == 2)
            action.sa_flags |= SA_RESETHAND;
    }
    return sigaction(SIGWINCH, &action, NULL);
}

int m59_verify_resize(void) {
    struct sigaction action;
    if (sigaction(SIGWINCH, NULL, &action) < 0)
        return -1;
    if (selected == 2 || selected == 4) {
        if (action.sa_handler != SIG_DFL)
            return -2;
    } else if (selected == 3) {
        if (action.sa_handler != SIG_IGN)
            return -3;
    } else {
        if ((selected == 0 &&
             (!(action.sa_flags & SA_SIGINFO) || action.sa_sigaction != detailed)) ||
            (selected == 1 && action.sa_handler != observed))
            return -4;
        if (!(action.sa_flags & SA_RESTART) || !(action.sa_flags & SA_NODEFER) ||
            !sigismember(&action.sa_mask, SIGUSR1))
            return -5;
    }
    if (selected <= 2 && calls < 1)
        return -6;
    if (selected <= 2 && !mask_ok)
        return -7;
    if (selected <= 2 && !signal_ok)
        return -8;
    int before = calls;
    errno = E2BIG;
    if (raise(SIGWINCH) != 0 || errno != E2BIG)
        return -9;
    return calls == before + (selected < 2 ? 1 : 0) ? 1 : -10;
}
