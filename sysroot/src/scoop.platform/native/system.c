#define _GNU_SOURCE 1
#include <errno.h>
#include <stdint.h>
#include <string.h>
#include <sys/utsname.h>
#include <unistd.h>

int scoop_platform_operating_system(void) {
#ifdef __APPLE__
    return 0;
#else
    return 1;
#endif
}

int scoop_platform_architecture(void) {
#ifdef __aarch64__
    return 0;
#else
    return 1;
#endif
}

int64_t scoop_platform_processor_count(void) {
    errno = 0;
    long count = sysconf(_SC_NPROCESSORS_ONLN);
    if (count <= 0) {
        if (errno == 0)
            errno = EIO;
        return -1;
    }
    return count;
}

char *scoop_platform_system_version(void) {
    struct utsname value;
    if (uname(&value) < 0)
        return NULL;
    return strdup(value.release);
}
