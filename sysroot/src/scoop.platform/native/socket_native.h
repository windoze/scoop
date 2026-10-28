#ifndef SCOOP_PLATFORM_SOCKET_NATIVE_H
#define SCOOP_PLATFORM_SOCKET_NATIVE_H

#include <stdbool.h>
#include <stdint.h>
#ifdef _WIN32
#ifdef _MSC_VER
#include <WinSock2.h>
#include <WS2tcpip.h>
#else
#include <winsock2.h>
#include <ws2tcpip.h>
#endif
#include "windows_api.h"
typedef SOCKET ScoopOsSocket;
typedef int ScoopSocketLength;
typedef ADDRINFOW ScoopAddressInfo;
#define SCOOP_INVALID_SOCKET INVALID_SOCKET
static inline int scoop_socket_error(void) { return WSAGetLastError(); }
int scoop_socket_startup(void);
#else
#include <errno.h>
#include <sys/socket.h>
typedef int ScoopOsSocket;
typedef socklen_t ScoopSocketLength;
typedef struct addrinfo ScoopAddressInfo;
#define SCOOP_INVALID_SOCKET (-1)
static inline int scoop_socket_error(void) { return errno; }
#endif

struct ScoopSocketHandle {
    uint64_t value;
};
struct ScoopSocketOpened {
    struct ScoopSocketHandle handle;
    int32_t error;
};
struct ScoopSocketResult {
    int64_t value;
    int32_t error;
};

struct ScoopSocketAddress {
    int32_t family, port;
    uint32_t scope, a, b, c, d;
};
struct ScoopDatagram {
    struct ScoopSocketAddress source;
    int64_t count;
    bool truncated;
};
struct ScoopSocketAddressResult {
    struct ScoopSocketAddress address;
    int32_t error;
};
struct ScoopSocketReceived {
    struct ScoopDatagram packet;
    int32_t error;
};
struct ScoopResolution {
    void *head;
    int32_t code, system_code;
    bool has_system_code;
};

ScoopSocketLength scoop_socket_address_encode(struct ScoopSocketAddress address,
                                              struct sockaddr_storage *out);
struct ScoopSocketAddress scoop_socket_address_decode(const struct sockaddr *address,
                                                      ScoopSocketLength length);
#ifndef _WIN32
int scoop_socket_prepare(int descriptor);
int scoop_socket_failed(int descriptor);
int scoop_platform_socket_bind(struct ScoopSocketAddress address, bool datagram, bool reuse,
                               int backlog);
#endif

#ifdef _WIN32
struct ScoopResolution scoop_platform_resolve(const wchar_t *host, int port, int family);
#else
struct ScoopResolution scoop_platform_resolve(const char *host, int port, int family);
#endif
void *scoop_platform_resolve_next(ScoopAddressInfo *entry);
struct ScoopSocketAddress scoop_platform_resolve_address(ScoopAddressInfo *entry);
void scoop_platform_resolve_free(ScoopAddressInfo *head);

#endif
