#include <stddef.h>
#include <stdint.h>
#include <zlib-ng.h>

uint32_t scoop_http_eventstream_crc32(uint32_t crc, const uint8_t *data,
                                    int64_t size) {
  return zng_crc32_z(crc, data, (size_t)size);
}
