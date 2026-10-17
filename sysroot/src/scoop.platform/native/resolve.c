#define _GNU_SOURCE 1
#include "socket_native.h"
#include <errno.h>
#include <netdb.h>
#include <stdio.h>

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

void *scoop_platform_resolve_next(struct addrinfo *entry) { return entry->ai_next; }
struct ScoopSocketAddress scoop_platform_resolve_address(struct addrinfo *entry) {
    return scoop_socket_address_decode(entry->ai_addr, entry->ai_addrlen);
}
void scoop_platform_resolve_free(struct addrinfo *head) { freeaddrinfo(head); }
