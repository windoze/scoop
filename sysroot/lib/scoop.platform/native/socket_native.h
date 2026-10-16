#ifndef SCOOP_PLATFORM_SOCKET_NATIVE_H
#define SCOOP_PLATFORM_SOCKET_NATIVE_H

#include <stdbool.h>
#include <stdint.h>
#include <sys/socket.h>

struct ScoopSocketAddress {
    int32_t family, port;
    uint32_t scope, a, b, c, d;
};
struct ScoopDatagram {
    struct ScoopSocketAddress source;
    int64_t count;
    bool truncated;
};
struct ScoopResolution {
    void *head;
    int32_t code, system_code;
    bool has_system_code;
};

socklen_t scoop_socket_address_encode(struct ScoopSocketAddress address,
                                      struct sockaddr_storage *out);
struct ScoopSocketAddress scoop_socket_address_decode(const struct sockaddr *address,
                                                      socklen_t length);
int scoop_socket_prepare(int descriptor);
int scoop_socket_failed(int descriptor);

#endif
