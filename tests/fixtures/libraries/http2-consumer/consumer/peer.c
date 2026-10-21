#include <nghttp2/nghttp2.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
  nghttp2_session *session;
  int server, closed, status, binary;
  size_t sent, received, pending, offset;
  uint8_t body[4], output[32768];
} peer;

static nghttp2_ssize body_read(nghttp2_session *session, int32_t id,
                               uint8_t *buffer, size_t length, uint32_t *flags,
                               nghttp2_data_source *source, void *user_data) {
  (void)session;
  (void)id;
  (void)user_data;
  peer *p = source->ptr;
  const char *body = p->server ? "42" : "ping";
  size_t count = strlen(body) - p->sent;
  if (count > length)
    count = length;
  memcpy(buffer, body + p->sent, count);
  p->sent += count;
  if (p->sent == strlen(body))
    *flags |= NGHTTP2_DATA_FLAG_EOF;
  return (nghttp2_ssize)count;
}

#define NV(name, value)                                                        \
  {(uint8_t *)name, (uint8_t *)value, sizeof(name) - 1, sizeof(value) - 1, 0}

static int frame_recv(nghttp2_session *session, const nghttp2_frame *frame,
                      void *user_data) {
  peer *p = user_data;
  if (p->server && frame->hd.type == NGHTTP2_HEADERS &&
      frame->headers.cat == NGHTTP2_HCAT_REQUEST) {
    nghttp2_nv fields[] = {NV(":status", "200"), NV("x-value", "\xff"),
                           NV("set-cookie", "one"), NV("set-cookie", "two")};
    nghttp2_data_provider2 provider = {{.ptr = p}, body_read};
    return nghttp2_submit_response2(session, frame->hd.stream_id, fields, 4,
                                    &provider);
  }
  return 0;
}

static int header(nghttp2_session *session, const nghttp2_frame *frame,
                  const uint8_t *name, size_t namelen, const uint8_t *value,
                  size_t valuelen, uint8_t flags, void *user_data) {
  (void)session;
  (void)frame;
  (void)flags;
  peer *p = user_data;
  if (namelen == 7 && !memcmp(name, ":status", 7))
    p->status = valuelen == 3 && !memcmp(value, "200", 3);
  if (namelen == 7 && !memcmp(name, "x-value", 7))
    p->binary = valuelen == 1 && value[0] == 255;
  return 0;
}

static int data(nghttp2_session *session, uint8_t flags, int32_t id,
                const uint8_t *bytes, size_t size, void *user_data) {
  (void)session;
  (void)flags;
  (void)id;
  peer *p = user_data;
  if (size > sizeof(p->body) - p->received)
    return NGHTTP2_ERR_CALLBACK_FAILURE;
  memcpy(p->body + p->received, bytes, size);
  p->received += size;
  return 0;
}

static int closed(nghttp2_session *session, int32_t id, uint32_t code,
                  void *user_data) {
  (void)session;
  (void)id;
  ((peer *)user_data)->closed = code == 0 ? 1 : -1;
  return 0;
}

uint64_t http_peer_new(int32_t server) {
  peer *p = calloc(1, sizeof(*p));
  nghttp2_session_callbacks *callbacks = NULL;
  if (!p)
    return 0;
  p->server = server;
  if (nghttp2_session_callbacks_new(&callbacks))
    goto fail;
  nghttp2_session_callbacks_set_on_header_callback(callbacks, header);
  nghttp2_session_callbacks_set_on_frame_recv_callback(callbacks, frame_recv);
  nghttp2_session_callbacks_set_on_data_chunk_recv_callback(callbacks, data);
  nghttp2_session_callbacks_set_on_stream_close_callback(callbacks, closed);
  int code = server ? nghttp2_session_server_new(&p->session, callbacks, p)
                    : nghttp2_session_client_new(&p->session, callbacks, p);
  if (code || nghttp2_submit_settings(p->session, 0, NULL, 0))
    goto fail;
  if (!server) {
    nghttp2_nv fields[] = {NV(":method", "POST"), NV(":scheme", "https"),
                           NV(":authority", "example.test"),
                           NV(":path", "/peer"), NV("x-value", "\xff")};
    nghttp2_data_provider2 provider = {{.ptr = p}, body_read};
    if (nghttp2_submit_request2(p->session, NULL, fields, 5, &provider, NULL) <
        0)
      goto fail;
  }
  nghttp2_session_callbacks_del(callbacks);
  return (uint64_t)(uintptr_t)p;
fail:
  nghttp2_session_callbacks_del(callbacks);
  nghttp2_session_del(p->session);
  free(p);
  return 0;
}

int64_t http_peer_receive(uint64_t handle, const uint8_t *data, int64_t size) {
  peer *p = (peer *)(uintptr_t)handle;
  return nghttp2_session_mem_recv2(p->session, data, (size_t)size);
}

int64_t http_peer_output(uint64_t handle, uint8_t *output, int64_t capacity) {
  peer *p = (peer *)(uintptr_t)handle;
  if (p->offset == p->pending) {
    const uint8_t *bytes;
    nghttp2_ssize size = nghttp2_session_mem_send2(p->session, &bytes);
    if (size < 0 || size > (nghttp2_ssize)sizeof(p->output))
      return -1;
    if (size)
      memcpy(p->output, bytes, (size_t)size);
    p->offset = 0;
    p->pending = (size_t)size;
  }
  size_t count = p->pending - p->offset;
  if (count > (size_t)capacity)
    count = (size_t)capacity;
  if (count)
    memcpy(output, p->output + p->offset, count);
  p->offset += count;
  return (int64_t)count;
}

int32_t http_peer_result(uint64_t handle) {
  peer *p = (peer *)(uintptr_t)handle;
  const char *expected = p->server ? "ping" : "42";
  return p->closed == 1 && p->binary && (p->server || p->status) &&
         p->received == strlen(expected) &&
         !memcmp(p->body, expected, p->received);
}

void http_peer_free(uint64_t handle) {
  peer *p = (peer *)(uintptr_t)handle;
  nghttp2_session_del(p->session);
  free(p);
}
