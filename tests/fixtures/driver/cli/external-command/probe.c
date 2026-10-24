#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "exit") == 0) {
        return 37;
    }
    if (argc > 1 && strcmp(argv[1], "signal") == 0) {
        raise(SIGTERM);
        return 1;
    }
    char cwd[4096];
    if (getcwd(cwd, sizeof(cwd)) == NULL) {
        return 1;
    }
    printf("argv0=%s\ncwd=%s\nenv=%s\n", argv[0], cwd, getenv("CLI_PAYLOAD"));
    for (int index = 1; index < argc; ++index) {
        printf("arg%d=", index);
        for (const unsigned char *byte = (unsigned char *)argv[index]; *byte; ++byte) {
            printf("%02x", *byte);
        }
        putchar('\n');
    }
    int byte;
    while ((byte = getchar()) != EOF) {
        putchar(byte);
    }
    fputs("command stderr\n", stderr);
    return 0;
}
