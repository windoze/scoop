#include <llhttp.h>
#include <nghttp2/nghttp2.h>
#include <stdint.h>
#include <string.h>

struct received {
  int headers;
  size_t body;
};

static int h1_body(llhttp_t *parser, const char *data, size_t length) {
  (void)data;
  struct received *received = parser->data;
  received->body += length;
  return 0;
}

static int h1_complete(llhttp_t *parser) {
  struct received *received = parser->data;
  received->headers++;
  return 0;
}

static int h1_probe(void) {
  llhttp_t parser;
  llhttp_settings_t settings;
  llhttp_settings_init(&settings);
  settings.on_body = h1_body;
  settings.on_message_complete = h1_complete;
  llhttp_init(&parser, HTTP_REQUEST, &settings);
  struct received received = {0};
  parser.data = &received;
  const char request[] = "POST /raw%2fpath HTTP/1.1\r\nHost: example.test\r\n"
                         "Transfer-Encoding: chunked\r\n\r\n"
                         "4\r\nbody\r\n0\r\nX-Trailer: yes\r\n\r\n";
  for (size_t i = 0; i < sizeof(request) - 1; ++i) {
    if (llhttp_execute(&parser, request + i, 1) != HPE_OK)
      return 1;
  }
  if (llhttp_finish(&parser) != HPE_OK || received.headers != 1 ||
      received.body != 4)
    return 2;
  llhttp_reset(&parser);
  const char unsupported[] = "CUSTOM / HTTP/1.1\r\n\r\n";
  return llhttp_execute(&parser, unsupported, sizeof(unsupported) - 1) ==
                 HPE_INVALID_METHOD
             ? 0
             : 3;
}

static int h2_frame(nghttp2_session *session, const nghttp2_frame *frame,
                    void *data) {
  (void)session;
  struct received *received = data;
  if (frame->hd.type == NGHTTP2_HEADERS)
    received->headers++;
  return 0;
}

static int transfer(nghttp2_session *source, nghttp2_session *destination) {
  while (nghttp2_session_want_write(source)) {
    const uint8_t *data;
    nghttp2_ssize size = nghttp2_session_mem_send2(source, &data);
    if (size <= 0)
      return size < 0 ? 1 : 0;
    /* Exercise the peer's incremental frame and HPACK input. */
    for (nghttp2_ssize i = 0; i < size; ++i) {
      if (nghttp2_session_mem_recv2(destination, data + i, 1) != 1)
        return 2;
    }
  }
  return 0;
}

#define FIELD(name, value)                                                     \
  {(uint8_t *)name, (uint8_t *)value, sizeof(name) - 1, sizeof(value) - 1,     \
   NGHTTP2_NV_FLAG_NONE}

int32_t scoop_http_native_probe(void) {
  if (h1_probe() || strcmp(nghttp2_version(0)->version_str, "1.70.0") != 0)
    return 1;
  nghttp2_session_callbacks *callbacks;
  if (nghttp2_session_callbacks_new(&callbacks) != 0)
    return 2;
  nghttp2_session_callbacks_set_on_frame_recv_callback(callbacks, h2_frame);
  nghttp2_session *client = NULL, *server = NULL;
  struct received client_received = {0}, server_received = {0};
  int result = 3;
  if (nghttp2_session_client_new(&client, callbacks, &client_received) ||
      nghttp2_session_server_new(&server, callbacks, &server_received))
    goto done;
  const nghttp2_settings_entry setting = {NGHTTP2_SETTINGS_ENABLE_PUSH, 0};
  if (nghttp2_submit_settings(client, NGHTTP2_FLAG_NONE, &setting, 1) ||
      nghttp2_submit_settings(server, NGHTTP2_FLAG_NONE, NULL, 0))
    goto done;
  const nghttp2_nv request[] = {
      FIELD(":method", "GET"), FIELD(":scheme", "http"),
      FIELD(":authority", "example.test"), FIELD(":path", "/raw%2fpath")};
  int32_t stream =
      nghttp2_submit_request2(client, NULL, request, 4, NULL, NULL);
  if (stream != 1 || transfer(client, server) || transfer(server, client))
    goto done;
  const nghttp2_nv response[] = {FIELD(":status", "200"),
                                 FIELD("x-repeated", "one"),
                                 FIELD("x-repeated", "two")};
  if (nghttp2_submit_response2(server, stream, response, 3, NULL) ||
      transfer(server, client))
    goto done;
  if (server_received.headers == 1 && client_received.headers == 1)
    result = 42;
done:
  nghttp2_session_del(client);
  nghttp2_session_del(server);
  nghttp2_session_callbacks_del(callbacks);
  return result;
}
