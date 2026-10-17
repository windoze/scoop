#define _GNU_SOURCE 1
#include <errno.h>
#include <pwd.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#ifdef __APPLE__
#include <crt_externs.h>
#else
extern char **environ;
#endif

const char *scoop_platform_environment_value(const char *name) { return getenv(name); }

char **scoop_platform_environment(void) {
    static char *empty[] = {NULL};
#ifdef __APPLE__
    char **values = *_NSGetEnviron();
#else
    char **values = environ;
#endif
    return values != NULL ? values : empty;
}

char *scoop_platform_current_directory(void) { return getcwd(NULL, 0); }

char *scoop_platform_home_directory(void) {
    long suggested = sysconf(_SC_GETPW_R_SIZE_MAX);
    size_t size = suggested > 0 ? (size_t)suggested : 1024;
    for (;;) {
        char *buffer = malloc(size);
        if (buffer == NULL)
            return NULL;
        struct passwd entry, *result;
        int error = getpwuid_r(getuid(), &entry, buffer, size, &result);
        if (error == ERANGE) {
            free(buffer);
            if (size > SIZE_MAX / 2) {
                errno = EOVERFLOW;
                return NULL;
            }
            size *= 2;
            continue;
        }
        char *home = NULL;
        if (error == 0 && result != NULL && result->pw_dir != NULL && result->pw_dir[0] != '\0') {
            home = strdup(result->pw_dir);
            if (home == NULL)
                error = errno;
        }
        free(buffer);
        errno = error;
        return home;
    }
}
