#define _POSIX_C_SOURCE 200809L
#include <assert.h>
#include <errno.h>
#include <netdb.h>
#include <stdint.h>

int32_t scoop_platform_error_kind(int32_t domain, int32_t code, int32_t system_code);

int main(void) {
    errno = EPIPE;
    assert(scoop_platform_error_kind(0, ENOENT, 0) == 1);
    assert(scoop_platform_error_kind(0, EACCES, 0) == 3);
    assert(scoop_platform_error_kind(0, ENOMEM, 0) == 7);
    assert(scoop_platform_error_kind(0, EPIPE, 0) == 9);
    assert(scoop_platform_error_kind(0, EWOULDBLOCK, 0) == 16);
    assert(scoop_platform_error_kind(0, ENOTSUP, 0) == 17);
    assert(scoop_platform_error_kind(0, INT32_MAX, 0) == 0);
    assert(scoop_platform_error_kind(1, EAI_SYSTEM, EACCES) == 3);
    assert(scoop_platform_error_kind(1, EAI_NONAME, 0) == 1);
    assert(scoop_platform_error_kind(1, EAI_MEMORY, 0) == 7);
    assert(scoop_platform_error_kind(1, INT32_MAX, 0) == 0);
    assert(errno == EPIPE);
}
