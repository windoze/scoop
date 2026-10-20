#include <arpa/inet.h>
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <unistd.h>

static int listener = -1;
static int client = -1;

void method_peer_close(void) {
  if (client >= 0)
    close(client);
  if (listener >= 0)
    close(listener);
  client = listener = -1;
}

int32_t method_peer_start(void) {
  listener = socket(AF_INET, SOCK_STREAM, 0);
  struct sockaddr_in address = {.sin_family = AF_INET,
                                .sin_addr.s_addr = htonl(INADDR_LOOPBACK)};
  socklen_t length = sizeof(address);
  if (listener < 0 || bind(listener, (struct sockaddr *)&address, length) ||
      listen(listener, 1) ||
      getsockname(listener, (struct sockaddr *)&address, &length)) {
    method_peer_close();
    return -1;
  }
  return ntohs(address.sin_port);
}

int32_t method_peer_reply(int32_t index) {
  static const char *methods[] = {"CUSTOM", "HEAD", "head", "connect"};
  if (index < 0 || index >= 4)
    return -1;
  if (client < 0) {
    client = accept(listener, NULL, NULL);
    if (client < 0)
      return -2;
    struct timeval timeout = {.tv_sec = 10};
    if (setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, &timeout, sizeof(timeout)))
      return -3;
#ifdef SO_NOSIGPIPE
    int enabled = 1;
    if (setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, &enabled, sizeof(enabled)))
      return -3;
#endif
  }
  char expected[128], received[128];
  int count = snprintf(expected, sizeof(expected),
                       "%s /resource HTTP/1.1\r\nHost: example.test\r\n\r\n",
                       methods[index]);
  if (count <= 0 || (size_t)count >= sizeof(expected))
    return -4;
  size_t offset = 0;
  while (offset < (size_t)count) {
    ssize_t got = recv(client, received + offset, (size_t)count - offset, 0);
    if (got < 0 && errno == EINTR)
      continue;
    if (got <= 0)
      return -5;
    offset += (size_t)got;
  }
  if (memcmp(expected, received, (size_t)count))
    return -6;
  const char *response = index == 1 ? "HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 "
                                      "200 OK\r\nContent-Length: 3\r\n\r\n"
                                    : "HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 "
                                      "200 OK\r\nContent-Length: 3\r\n\r\nyes";
  size_t length = strlen(response);
  offset = 0;
  while (offset < length) {
    int flags = 0;
#ifdef MSG_NOSIGNAL
    flags = MSG_NOSIGNAL;
#endif
    ssize_t sent = send(client, response + offset, length - offset, flags);
    if (sent < 0 && errno == EINTR)
      continue;
    if (sent <= 0)
      return -7;
    offset += (size_t)sent;
  }
  return 0;
}
