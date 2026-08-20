#include <assert.h>
#include <errno.h>
#include <stdint.h>

typedef int32_t (*Callback)(int32_t, void *);
extern int32_t m13_run_many(Callback callback, void *context, int32_t base,
                            int32_t count);

int32_t m33_errno_callback(Callback callback, void *context, int32_t value) {
  assert(errno == 0);
  errno = 93;
  int32_t result = callback(value, context);
  errno = 61;
  return result + 1;
}

int32_t m33_errno_parallel(Callback callback, void *context) {
  assert(errno == 0);
  int32_t result = m13_run_many(callback, context, 10, 4);
  errno = 79;
  return result;
}
