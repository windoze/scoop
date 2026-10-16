#include <stdio.h>

static int initialized;

__attribute__((constructor)) static void initialize(void) { initialized = 42; }
__attribute__((destructor)) static void finalize(void) { (void)fputs("cleanup\n", stdout); }

int native_initialized(void) { return initialized; }
