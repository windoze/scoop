#include "scoop_rt.h"
#include <assert.h>
#include <config.h>
#include <errno.h>

typedef struct Pair {
  int32_t number;
  bool flag;
  double fraction;
} Pair;

static int32_t count;
static _Thread_local int32_t thread_count;

int32_t m33_native_count(void) { return ++count + ++thread_count; }
const char *m33_native_message(void) { return "native"; }

int32_t m33_native_value(int32_t mode) {
  assert(scoop_rt_thread_debug_mode() == (uint32_t)mode);
  return M33_OFFSET + M33_VALUE;
}

Pair m33_native_pair(Pair value) {
  value.number += m33_native_platform();
  value.flag = !value.flag;
  value.fraction += 0.25;
  return value;
}

int32_t m33_native_errno(int32_t value) {
  assert(errno == 0);
  errno = 29;
  return value;
}
