#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <netdb.h>
#include <stdint.h>

/* Private category numbers shared with system/errors.scoop. */
static int32_t errno_kind(int32_t code) {
    switch (code) {
    case ENOENT:
        return 1;
    case EEXIST:
        return 2;
    case EACCES:
    case EPERM:
        return 3;
    case ENOTDIR:
        return 4;
    case EISDIR:
        return 5;
    case EINVAL:
    case ENAMETOOLONG:
        return 6;
    case ENOMEM:
        return 7;
    case ENOSPC:
    case EDQUOT:
        return 8;
    case EPIPE:
        return 9;
    case ECONNREFUSED:
        return 10;
    case ECONNRESET:
    case ECONNABORTED:
        return 11;
    case EADDRINUSE:
        return 12;
    case EADDRNOTAVAIL:
        return 13;
    case ETIMEDOUT:
        return 14;
    case EINTR:
        return 15;
    case EAGAIN:
        return 16;
#if EWOULDBLOCK != EAGAIN
    case EWOULDBLOCK:
        return 16;
#endif
    case ENOSYS:
    case ENOTSUP:
        return 17;
#if EOPNOTSUPP != ENOTSUP
    case EOPNOTSUPP:
        return 17;
#endif
    default:
        return 0;
    }
}

int32_t scoop_platform_error_kind(int32_t domain, int32_t code, int32_t system_code) {
    if (domain == 0)
        return errno_kind(code);
    /* libuv's supported Unix targets encode system errors as negative errno. */
    if (domain == 2)
        return code < 0 && code != INT32_MIN ? errno_kind(-code) : 0;
    switch (code) {
    case EAI_SYSTEM:
        return errno_kind(system_code);
    case EAI_NONAME:
        return 1;
    case EAI_BADFLAGS:
        return 6;
    case EAI_MEMORY:
        return 7;
    case EAI_AGAIN:
        return 16;
    case EAI_FAMILY:
    case EAI_SOCKTYPE:
    case EAI_SERVICE:
        return 17;
    default:
        return 0;
    }
}
