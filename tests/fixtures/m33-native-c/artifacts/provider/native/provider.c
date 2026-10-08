#include <errno.h>
#include <stdint.h>
int32_t m33_provider_sum(int32_t value) {
  errno = 17;
  return value + PROVIDER_OFFSET;
}
