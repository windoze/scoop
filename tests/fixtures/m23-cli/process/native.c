#include <crt_externs.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

void m23_cli_process(void) {
    const char *mode = getenv("CLI_MODE");
    if (mode && strcmp(mode, "exit") == 0) exit(37);
    if (mode && strcmp(mode, "abort") == 0) raise(SIGABRT);
    if (mode && strcmp(mode, "term") == 0) raise(SIGTERM);
    if (mode && (strcmp(mode, "hold") == 0 || strcmp(mode, "interrupt") == 0)) {
        const char *ready = getenv("CLI_READY");
        char pending[4096];
        snprintf(pending, sizeof(pending), "%s.pending", ready);
        FILE *file = fopen(pending, "w");
        if (!file) exit(91);
        fprintf(file, "%d", getpid());
        fclose(file);
        if (rename(pending, ready) != 0) exit(92);
        if (strcmp(mode, "interrupt") == 0) {
            for (;;) pause();
        }
        while (access(getenv("CLI_RELEASE"), F_OK) != 0) usleep(10000);
        puts("original");
        return;
    }
    char cwd[4096];
    if (!getcwd(cwd, sizeof(cwd))) exit(93);
    char **argv = *_NSGetArgv();
    int argc = *_NSGetArgc();
    printf("argv0=%s\ncwd=%s\nenv=%s\n", argv[0], cwd, getenv("CLI_PAYLOAD"));
    for (int i = 1; i < argc; i++) {
        printf("arg%d=", i);
        for (const unsigned char *p = (const unsigned char *)argv[i]; *p; p++)
            printf("%02x", *p);
        putchar('\n');
    }
    int byte;
    while ((byte = getchar()) != EOF) putchar(byte);
    fputs("program stderr\n", stderr);
}
