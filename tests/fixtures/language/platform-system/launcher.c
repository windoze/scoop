#define _GNU_SOURCE 1
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

int main(int argc, char **argv) {
    assert(argc == 2);
    char argument[] = {'-', (char)0xff, 0};
    char raw[] = {'K', (char)0xff, '=', 'V', (char)0xfe, 0};
    char home[] = {'H', 'O', 'M', 'E', '=', '/', 'h', (char)0xff, 0};
    char *arguments[] = {argv[1], "", "two words", argument, "中", NULL};
    char *environment[] = {"EMPTY=", "PAIR=a=b", raw, home, "TMPDIR=/tmp", NULL, NULL};
    if (getenv("SCOOP_GC_STRESS_MOVE") != NULL)
        environment[5] = "SCOOP_GC_STRESS_MOVE=1";
    execve(argv[1], arguments, environment);
    perror("execve");
    return 1;
}
