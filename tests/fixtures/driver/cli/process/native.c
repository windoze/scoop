#if defined(__APPLE__)
#include <crt_externs.h>
#endif
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

static void print_argument(size_t index, const char *argument, const char *cwd) {
    if (index == 0) {
        printf("argv0=%s\ncwd=%s\nenv=%s\n", argument, cwd, getenv("CLI_PAYLOAD"));
        return;
    }
    printf("arg%zu=", index);
    for (const unsigned char *p = (const unsigned char *)argument; *p; p++)
        printf("%02x", *p);
    putchar('\n');
}

static void print_arguments(const char *cwd) {
#if defined(__APPLE__)
    char **argv = *_NSGetArgv();
    int argc = *_NSGetArgc();
    for (int i = 0; i < argc; i++) print_argument((size_t)i, argv[i], cwd);
#elif defined(__linux__)
    FILE *file = fopen("/proc/self/cmdline", "rb");
    if (!file) exit(94);
    char *argument = NULL;
    size_t capacity = 0;
    size_t index = 0;
    while (getdelim(&argument, &capacity, '\0', file) >= 0)
        print_argument(index++, argument, cwd);
    if (ferror(file)) exit(95);
    free(argument);
    fclose(file);
#else
#error Unsupported process fixture platform
#endif
}

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
    print_arguments(cwd);
    int byte;
    while ((byte = getchar()) != EOF) putchar(byte);
    fputs("program stderr\n", stderr);
}
