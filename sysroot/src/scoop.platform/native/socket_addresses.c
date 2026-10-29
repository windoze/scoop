#ifndef _WIN32
#define _GNU_SOURCE 1
#endif
#include "socket_native.h"
#ifndef _WIN32
#include <arpa/inet.h>
#include <errno.h>
#endif
#include <string.h>

ScoopSocketLength scoop_socket_address_encode(struct ScoopSocketAddress address,
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
#ifdef _WIN32
    WSASetLastError(WSAEAFNOSUPPORT);
#else
    errno = EAFNOSUPPORT;
#endif
    return 0;
}

struct ScoopSocketAddress scoop_socket_address_decode(const struct sockaddr *address,
                                                      ScoopSocketLength length) {
    struct ScoopSocketAddress result = {0};
    if (address->sa_family == AF_INET && length >= (ScoopSocketLength)sizeof(struct sockaddr_in)) {
        const struct sockaddr_in *value = (const struct sockaddr_in *)address;
        result.family = 4;
        result.port = ntohs(value->sin_port);
        result.a = ntohl(value->sin_addr.s_addr);
    } else if (address->sa_family == AF_INET6 &&
               length >= (ScoopSocketLength)sizeof(struct sockaddr_in6)) {
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
#ifdef _WIN32
        WSASetLastError(WSAEAFNOSUPPORT);
#else
        errno = EAFNOSUPPORT;
#endif
    }
    return result;
}

struct ScoopSocketAddress scoop_platform_ip_parse(const char *text) {
    struct sockaddr_in ipv4 = {0};
    ipv4.sin_family = AF_INET;
#ifdef _WIN32
    if (InetPtonA(AF_INET, text, &ipv4.sin_addr) == 1)
#else
    if (inet_pton(AF_INET, text, &ipv4.sin_addr) == 1)
#endif
        return scoop_socket_address_decode((const struct sockaddr *)&ipv4, sizeof(ipv4));
    struct sockaddr_in6 ipv6 = {0};
    ipv6.sin6_family = AF_INET6;
#ifdef _WIN32
    if (InetPtonA(AF_INET6, text, &ipv6.sin6_addr) == 1)
#else
    if (inet_pton(AF_INET6, text, &ipv6.sin6_addr) == 1)
#endif
        return scoop_socket_address_decode((const struct sockaddr *)&ipv6, sizeof(ipv6));
    return (struct ScoopSocketAddress){0};
}

int scoop_platform_ip_format(struct ScoopSocketAddress address, char *text) {
    struct sockaddr_storage storage;
    if (scoop_socket_address_encode(address, &storage) == 0)
        return scoop_socket_error();
    const void *bytes = address.family == 4
                            ? (const void *)&((struct sockaddr_in *)&storage)->sin_addr
                            : (const void *)&((struct sockaddr_in6 *)&storage)->sin6_addr;
#ifdef _WIN32
    return InetNtopA(storage.ss_family, (void *)bytes, text, INET6_ADDRSTRLEN) == NULL
               ? WSAGetLastError()
               : 0;
#else
    return inet_ntop(storage.ss_family, bytes, text, INET6_ADDRSTRLEN) == NULL ? errno : 0;
#endif
}

struct ScoopSocketAddressResult scoop_platform_socket_address(struct ScoopSocketHandle socket,
                                                              bool peer) {
    struct sockaddr_storage storage;
    ScoopSocketLength length = sizeof(storage);
    int status =
        peer ? getpeername((ScoopOsSocket)socket.value, (struct sockaddr *)&storage, &length)
             : getsockname((ScoopOsSocket)socket.value, (struct sockaddr *)&storage, &length);
    struct ScoopSocketAddressResult result = {0};
    if (status < 0) {
        result.error = scoop_socket_error();
        return result;
    }
    result.address = scoop_socket_address_decode((const struct sockaddr *)&storage, length);
    if (result.address.family == 0)
        result.error = scoop_socket_error();
    return result;
}
