#ifndef _WIN32
#define _GNU_SOURCE 1
#endif
#include "socket_native.h"
#include <errno.h>
#ifndef _WIN32
#include <netdb.h>
#endif
#include <stdio.h>

#ifdef _WIN32
#ifndef AI_DISABLE_IDN_ENCODING
/* Windows 8+ SDK flag omitted by the selected mingw-w64 headers. */
#define AI_DISABLE_IDN_ENCODING 0x00080000
#endif
struct ScoopResolution scoop_platform_resolve(const wchar_t *host, int port, int family) {
    int error = scoop_socket_startup();
    if (error != 0)
        return (struct ScoopResolution){NULL, error, 0, false};
    wchar_t service[6];
    (void)swprintf(service, 6, L"%d", port);
    ADDRINFOW hints = {0};
    hints.ai_family = family == 4 ? AF_INET : family == 6 ? AF_INET6 : AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_flags = AI_NUMERICSERV | AI_DISABLE_IDN_ENCODING;
    ADDRINFOW *head = NULL;
    int code = GetAddrInfoW(host, service, &hints, &head);
    return (struct ScoopResolution){head, code, 0, false};
}
#else
struct ScoopResolution scoop_platform_resolve(const char *host, int port, int family) {
    char service[6];
    (void)snprintf(service, sizeof(service), "%d", port);
    struct addrinfo hints = {0};
    hints.ai_family = family == 4 ? AF_INET : family == 6 ? AF_INET6 : AF_UNSPEC;
    hints.ai_socktype = SOCK_STREAM;
    hints.ai_flags = AI_NUMERICSERV;
    struct addrinfo *head = NULL;
    int code = getaddrinfo(host, service, &hints, &head);
    int system_code = errno;
    return (struct ScoopResolution){head, code, code == EAI_SYSTEM ? system_code : 0,
                                    code == EAI_SYSTEM};
}
#endif

void *scoop_platform_resolve_next(ScoopAddressInfo *entry) { return entry->ai_next; }
struct ScoopSocketAddress scoop_platform_resolve_address(ScoopAddressInfo *entry) {
    return scoop_socket_address_decode(entry->ai_addr, (ScoopSocketLength)entry->ai_addrlen);
}
struct ScoopSocketAddressResult scoop_platform_resolve_endpoint(ScoopAddressInfo *entry) {
    struct ScoopSocketAddress address = scoop_platform_resolve_address(entry);
    return (struct ScoopSocketAddressResult){address,
                                             address.family == 0 ? scoop_socket_error() : 0};
}
void scoop_platform_resolve_free(ScoopAddressInfo *head) {
#ifdef _WIN32
    FreeAddrInfoW(head);
#else
    freeaddrinfo(head);
#endif
}
