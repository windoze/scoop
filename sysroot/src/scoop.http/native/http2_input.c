#include "http2.h"
#include <limits.h>

static int progress(http2_input *input, int32_t id, int64_t count) {
  if (!input->capture || id == 0 || count == 0)
    return 1;
  http_bytes *bytes = &input->progress;
  if (bytes->size) {
    http2_input_progress *last =
        (http2_input_progress *)(bytes->data + bytes->size) - 1;
    if (last->stream_id == id && count <= INT64_MAX - last->count) {
      last->count += count;
      return 1;
    }
  }
  http2_input_progress entry = {count, id};
  return http_bytes_append(bytes, &entry, sizeof(entry));
}

static int begin_frame(nghttp2_session *session, const nghttp2_frame_hd *hd,
                       void *user_data) {
  (void)session;
  http2 *h = user_data;
  http2_input *input = &h->input;
  input->began = 1;
  input->payload_left = hd->length;
  input->header_left = 9;
  input->stream_id = hd->type == NGHTTP2_HEADERS ||
                             hd->type == NGHTTP2_CONTINUATION ||
                             hd->type == NGHTTP2_DATA
                         ? hd->stream_id
                         : 0;
  if (h->server && hd->type == NGHTTP2_HEADERS) {
    http2_stream *stream = http2_find(h, hd->stream_id);
    if ((!stream || !stream->received_final) &&
        http2_push_simple(h, HTTP_EVENT_REQUEST_HEAD_START, hd->stream_id, 0))
      return NGHTTP2_ERR_CALLBACK_FAILURE;
  }
  return progress(input, input->stream_id, 9) ? 0
                                              : NGHTTP2_ERR_CALLBACK_FAILURE;
}

void http2_input_setup(http2 *h, nghttp2_session_callbacks *callbacks) {
  h->input.magic_left = h->server ? NGHTTP2_CLIENT_MAGIC_LEN : 0;
  h->input.header_left = 9;
  nghttp2_session_callbacks_set_on_begin_frame_callback(callbacks, begin_frame);
}

enum { INPUT_MAGIC, INPUT_PAYLOAD, INPUT_HEADER, INPUT_UNKNOWN, INPUT_ENDED };

static size_t input_prefix(http2 *h, size_t available, int *kind) {
  http2_input *input = &h->input;
  size_t count;
  if (!nghttp2_session_want_read(h->session)) {
    *kind = INPUT_ENDED;
    return available;
  }
  if (input->magic_left) {
    *kind = INPUT_MAGIC;
    count = input->magic_left;
  } else if (input->payload_left) {
    *kind = INPUT_PAYLOAD;
    count = input->payload_left;
  } else if (input->header_left) {
    *kind = INPUT_HEADER;
    count = input->header_left;
  } else {
    // Ignored frames have no begin callback; let nghttp2 find the next
    // boundary.
    *kind = INPUT_UNKNOWN;
    count = 1;
  }
  return count < available ? count : available;
}

http_step scoop_http2_receive(uint64_t handle, const uint8_t *data,
                              int64_t size, int32_t capture) {
  http2 *h = (http2 *)(uintptr_t)handle;
  if (h->ended || h->failed)
    return (http_step){0, http2_result(h, NGHTTP2_ERR_INVALID_STATE)};
  http2_input *input = &h->input;
  input->capture = capture;
  input->progress.size = 0;
  int64_t total = 0;
  while (total < size) {
    int kind;
    size_t prefix = input_prefix(h, (size_t)(size - total), &kind);
    int32_t id = input->stream_id;
    input->began = 0;
    nghttp2_ssize count =
        nghttp2_session_mem_recv2(h->session, data + total, prefix);
    if (count < 0) {
      h->failed = 1;
      return (http_step){total, http2_result(h, (int)count)};
    }
    if (!count)
      break;
    total += count;
    if (kind == INPUT_MAGIC)
      input->magic_left -= (size_t)count;
    else if (kind == INPUT_HEADER && !input->began)
      input->header_left -= (size_t)count;
    else if (kind == INPUT_PAYLOAD) {
      input->payload_left -= (size_t)count;
      if (!progress(input, id, count)) {
        h->failed = 1;
        return (http_step){total, http2_result(h, NGHTTP2_ERR_NOMEM)};
      }
    }
  }
  return (http_step){total, 0};
}

int64_t scoop_http2_input_progress_count(uint64_t handle) {
  return (int64_t)(((http2 *)(uintptr_t)handle)->input.progress.size /
                   sizeof(http2_input_progress));
}

http2_input_progress scoop_http2_input_progress(uint64_t handle,
                                                int64_t index) {
  http2_input *input = &((http2 *)(uintptr_t)handle)->input;
  return ((http2_input_progress *)input->progress.data)[index];
}
