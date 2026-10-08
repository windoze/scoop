#include "scoop_rt.h"
#include <assert.h>
#include <errno.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>

typedef struct Pair {
  int32_t number;
  bool flag;
  double fraction;
} Pair;

static _Atomic(int32_t) released;

int32_t m33_errno_int(int32_t value, int32_t code, int32_t mode) {
  if (mode != -2) {
    assert(errno == 0);
  }
  if (mode >= 0) {
    assert(scoop_rt_thread_debug_mode() == (uint32_t)mode);
  }
  errno = code;
  return value;
}

void m33_errno_void(int32_t code) {
  assert(errno == 0);
  errno = code;
}

Pair m33_errno_pair(Pair value, int32_t capture) {
  if (capture) {
    assert(errno == 0);
  }
  value.number += 10;
  value.flag = !value.flag;
  value.fraction += 0.5;
  errno = 31;
  return value;
}

bool m33_errno_bool(bool value) {
  assert(errno == 0);
  errno = 29;
  return !value;
}

int8_t *m33_errno_pointer(int32_t present) {
  static int8_t text[] = "errno";
  assert(errno == 0);
  errno = present ? 0 : ENOENT;
  return present ? text : NULL;
}

int32_t m33_errno_missing(void) { return ENOENT; }

void m33_errno_disturb(void) { errno = 777; }

void m33_errno_record(int32_t value, int32_t error) {
  assert(value == 5 && error == 55);
  atomic_fetch_add_explicit(&released, 1, memory_order_relaxed);
}

int32_t m33_errno_released(void) {
  return atomic_load_explicit(&released, memory_order_relaxed);
}
