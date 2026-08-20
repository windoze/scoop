#include "startup.h"
#include <stdlib.h>

static void restart(void) {
  (void)scoop_rt_run_program(NULL, 0, NULL, 0, NULL);
}

void m23_register_restart(void) {
  if (atexit(restart) != 0) {
    abort();
  }
}

void m23_reenter(void) { restart(); }
