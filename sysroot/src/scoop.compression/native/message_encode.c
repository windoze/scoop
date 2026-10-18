#include "compression.h"

void comp_message_resume_encoding(comp_codec *codec) {
  // Z_NO_FLUSH permits a new empty sync block after the preceding sync point.
  comp_progress step = comp_deflate_step(codec, NULL, 0, codec->output,
                                         sizeof(codec->output), 0);
  if (step.produced)
    comp_fail(codec, COMP_BACKEND, 0, "unfinished DEFLATE message output");
}

comp_progress comp_message_encode(comp_codec *codec, const unsigned char *input,
                                  size_t size, size_t capacity, int action) {
  if (action == 2 && codec->flush_done) {
    comp_message_resume_encoding(codec);
    codec->flush_done = 0;
    if (codec->kind)
      return (comp_progress){0};
  }
  size_t prefix = codec->message.tail_size;
  memcpy(codec->output, codec->message.tail, prefix);
  comp_progress step =
      comp_deflate_step(codec, input, size, codec->output + prefix,
                        capacity - prefix, action == 0 ? 0 : 1);
  size_t total = prefix + step.produced;
  if (action == 1 && step.status == 0 && !codec->kind) {
    codec->message.tail_size = 0;
    step.produced = total;
    return step;
  }
  size_t kept = total < 4 ? total : 4;
  codec->message.tail_size = kept;
  memcpy(codec->message.tail, codec->output + total - kept, kept);
  step.produced = total - kept;
  if (action == 2 && step.status == 0 && !codec->kind) {
    static const unsigned char sync_tail[4] = {0, 0, 255, 255};
    if (kept != 4 || memcmp(codec->message.tail, sync_tail, 4))
      comp_fail(codec, COMP_BACKEND, 0,
                "DEFLATE sync flush has no message tail");
    else {
      codec->message.tail_size = 0;
      step.status = 2;
    }
  }
  return step;
}
