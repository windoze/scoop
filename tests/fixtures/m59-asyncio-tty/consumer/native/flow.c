#include <termios.h>
#include <unistd.h>

int m59_tty_flow(int stopped) { return tcflow(STDOUT_FILENO, stopped ? TCOOFF : TCOON); }
