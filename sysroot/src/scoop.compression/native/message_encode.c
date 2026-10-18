#include "compression.h"

comp_progress comp_message_encode(comp_codec *codec, const unsigned char *input,
                                  size_t size, size_t capacity, int action) {
  size_t prefix = codec->message.tail_size;
  memcpy(codec->output, codec->message.tail, prefix);
  comp_progress step =
      comp_deflate_step(codec, input, size, codec->output + prefix,
                        capacity - prefix, action == 2 ? 1 : 0);
  size_t total = prefix + step.produced;
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
