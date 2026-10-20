#include "compression.h"

static int final_block(comp_codec *codec, int flags) {
  if ((flags & 192) != 192)
    return 0;
  unsigned bits = (unsigned)flags & 63;
  if (bits > 7 || (bits && (!codec->message.has_last ||
                            (codec->message.last_byte >> (8 - bits)) != 0))) {
    comp_fail(codec, COMP_INVALID, 0, "nonzero DEFLATE final-block padding");
    return 1;
  }
  int code = zng_inflateResetKeep(&codec->backend.deflate.stream);
  if (code != Z_OK)
    comp_fail(codec, COMP_BACKEND, code,
              "cannot continue after a DEFLATE final block");
  return 1;
}

comp_progress comp_message_decode(comp_codec *codec, const unsigned char *input,
                                  size_t size, size_t capacity, int action) {
  zng_stream *stream = &codec->backend.deflate.stream;
  static const unsigned char sync_tail[4] = {0, 0, 255, 255};
  if (action == 2 && !codec->message.feeding_tail &&
      zng_inflateSyncPoint(stream) == 1)
    codec->message.feeding_tail = 1;
  int tail = codec->message.feeding_tail;
  const unsigned char *data =
      tail ? sync_tail + codec->message.tail_read : input;
  size_t available = tail ? 4 - codec->message.tail_read : size;
  stream->next_in = data;
  stream->avail_in = (uint32_t)available;
  stream->next_out = codec->output;
  stream->avail_out = (uint32_t)capacity;
  int code = zng_inflate(stream, Z_BLOCK);
  size_t consumed = available - stream->avail_in;
  comp_progress step = {tail ? 0 : consumed, capacity - stream->avail_out, 0};
  int flags = stream->data_type;
  stream->next_in = NULL;
  stream->next_out = NULL;
  if (!tail && consumed) {
    codec->message.last_byte = data[consumed - 1];
    codec->message.has_last = 1;
  }
  if (code != Z_OK && code != Z_BUF_ERROR) {
    comp_fail(codec,
              code == Z_DATA_ERROR || code == Z_STREAM_END ? COMP_INVALID
                                                           : COMP_BACKEND,
              code,
              stream->msg ? stream->msg : "invalid DEFLATE message boundary");
    return step;
  }
  if (tail) {
    codec->message.tail_read += consumed;
    if (codec->message.tail_read == 4) {
      if (!(flags & 128) || (flags & 127) || step.produced)
        comp_fail(codec, COMP_INVALID, 0, "invalid DEFLATE sync boundary");
      else
        step.status = 2;
    } else
      step.status = 1;
    return step;
  }
  int ended_block = final_block(codec, flags);
  if (codec->kind)
    return step;
  if (action == 2) {
    if (step.produced || ended_block || (flags & 128) ||
        zng_inflateSyncPoint(stream) == 1)
      step.status = 1;
    else
      comp_fail(codec, COMP_TRUNCATED, 0,
                "DEFLATE message has no complete sync boundary");
  } else if (consumed < size || step.produced == capacity)
    step.status = 1;
  return step;
}
