#define _GNU_SOURCE 1
#include <arpa/inet.h>
#include <assert.h>
#include <pthread.h>
#include <sys/socket.h>
#include <unistd.h>

static struct sockaddr_storage endpoint;
static socklen_t endpoint_length;
static pthread_t peer;

static void *connect_peer(void *unused) {
    (void)unused;
    int descriptor = socket(endpoint.ss_family, SOCK_STREAM, 0);
    assert(descriptor >= 0);
    assert(connect(descriptor, (struct sockaddr *)&endpoint, endpoint_length) == 0);
    const char *text = "peer";
    size_t remaining = 4;
    while (remaining > 0) {
        ssize_t count = write(descriptor, text, remaining);
        assert(count > 0);
        text += count;
        remaining -= (size_t)count;
    }
    assert(close(descriptor) == 0);
    return NULL;
}

void m45_network_start_peer(int listener) {
    endpoint_length = sizeof(endpoint);
    assert(getsockname(listener, (struct sockaddr *)&endpoint, &endpoint_length) == 0);
    assert(pthread_create(&peer, NULL, connect_peer, NULL) == 0);
}
void m45_network_finish_peer(void) { assert(pthread_join(peer, NULL) == 0); }

void m45_network_send_packet(int receiver) {
    struct sockaddr_storage address;
    socklen_t length = sizeof(address);
    assert(getsockname(receiver, (struct sockaddr *)&address, &length) == 0);
    int descriptor = socket(address.ss_family, SOCK_DGRAM, 0);
    assert(descriptor >= 0);
    assert(sendto(descriptor, "abcd", 4, 0, (struct sockaddr *)&address, length) == 4);
    assert(close(descriptor) == 0);
}
