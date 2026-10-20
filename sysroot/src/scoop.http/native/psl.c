#include "http.h"

static const char public_suffix_data[] =
#include "public_suffix_list.inc"
    ;

http_byte_view scoop_http_public_suffix_data(void) {
  return (http_byte_view){(const uint8_t *)public_suffix_data,
                          (int64_t)(sizeof(public_suffix_data) - 1)};
}
