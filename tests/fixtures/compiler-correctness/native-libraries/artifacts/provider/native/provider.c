#include <errno.h>
#include <math.h>
#include <stdint.h>
int32_t m33_provider_sum(int32_t value) {
  volatile double argument = 0.0;
  int32_t extra = cos(argument) == 1.0 ? 0 : 1000;
  errno = 17;
  return value + PROVIDER_OFFSET + extra;
}
