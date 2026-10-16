#define _GNU_SOURCE 1
#include "socket_native.h"
#include <arpa/inet.h>
#include <errno.h>
#include <string.h>

socklen_t scoop_socket_address_encode(struct ScoopSocketAddress address,
                                      struct sockaddr_storage *out) {
    memset(out, 0, sizeof(*out));
    if (address.family == 4) {
        struct sockaddr_in *value = (struct sockaddr_in *)out;
        value->sin_family = AF_INET;
#ifdef __APPLE__
        value->sin_len = sizeof(*value);
#endif
        value->sin_port = htons((uint16_t)address.port);
        value->sin_addr.s_addr = htonl(address.a);
        return sizeof(*value);
    }
    if (address.family == 6) {
        struct sockaddr_in6 *value = (struct sockaddr_in6 *)out;
        value->sin6_family = AF_INET6;
#ifdef __APPLE__
        value->sin6_len = sizeof(*value);
#endif
        value->sin6_port = htons((uint16_t)address.port);
        value->sin6_scope_id = address.scope;
        uint32_t words[4] = {htonl(address.a), htonl(address.b), htonl(address.c),
                             htonl(address.d)};
        memcpy(&value->sin6_addr, words, sizeof(words));
        return sizeof(*value);
    }
    errno = EAFNOSUPPORT;
    return 0;
}

struct ScoopSocketAddress scoop_socket_address_decode(const struct sockaddr *address,
                                                      socklen_t length) {
    struct ScoopSocketAddress result = {0};
    if (address->sa_family == AF_INET && length >= sizeof(struct sockaddr_in)) {
        const struct sockaddr_in *value = (const struct sockaddr_in *)address;
        result.family = 4;
        result.port = ntohs(value->sin_port);
        result.a = ntohl(value->sin_addr.s_addr);
    } else if (address->sa_family == AF_INET6 && length >= sizeof(struct sockaddr_in6)) {
        const struct sockaddr_in6 *value = (const struct sockaddr_in6 *)address;
        result.family = 6;
        result.port = ntohs(value->sin6_port);
        result.scope = value->sin6_scope_id;
        uint32_t words[4];
        memcpy(words, &value->sin6_addr, sizeof(words));
        result.a = ntohl(words[0]);
        result.b = ntohl(words[1]);
        result.c = ntohl(words[2]);
        result.d = ntohl(words[3]);
    } else {
        errno = EAFNOSUPPORT;
    }
    return result;
}

struct ScoopSocketAddress scoop_platform_ip_parse(const char *text) {
    struct sockaddr_in ipv4 = {0};
    ipv4.sin_family = AF_INET;
    if (inet_pton(AF_INET, text, &ipv4.sin_addr) == 1)
        return scoop_socket_address_decode((const struct sockaddr *)&ipv4, sizeof(ipv4));
    struct sockaddr_in6 ipv6 = {0};
    ipv6.sin6_family = AF_INET6;
    if (inet_pton(AF_INET6, text, &ipv6.sin6_addr) == 1)
        return scoop_socket_address_decode((const struct sockaddr *)&ipv6, sizeof(ipv6));
    return (struct ScoopSocketAddress){0};
}

int scoop_platform_ip_format(struct ScoopSocketAddress address, char *text) {
    struct sockaddr_storage storage;
    if (scoop_socket_address_encode(address, &storage) == 0)
        return -1;
    const void *bytes = address.family == 4
                            ? (const void *)&((struct sockaddr_in *)&storage)->sin_addr
                            : (const void *)&((struct sockaddr_in6 *)&storage)->sin6_addr;
    return inet_ntop(storage.ss_family, bytes, text, INET6_ADDRSTRLEN) == NULL ? -1 : 0;
}

struct ScoopSocketAddress scoop_platform_socket_address(int descriptor, bool peer) {
    struct sockaddr_storage storage;
    socklen_t length = sizeof(storage);
    int result = peer ? getpeername(descriptor, (struct sockaddr *)&storage, &length)
                      : getsockname(descriptor, (struct sockaddr *)&storage, &length);
    return result < 0 ? (struct ScoopSocketAddress){0}
                      : scoop_socket_address_decode((const struct sockaddr *)&storage, length);
}
