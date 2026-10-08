#include <stdint.h>
#include <value.h>
extern "C" int32_t m33_cached(void) {
  return (M33_OFFSET + M33_VALUE + M33_BIAS) * M33_SCALE;
}
