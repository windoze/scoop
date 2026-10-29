// Validate static ELF TLS alignment and independent values in two threads.
#include <pthread.h>

_Thread_local long tls_initialized = 7;
_Thread_local long tls_zero;
_Alignas(64) _Thread_local long tls_aligned = 9;

__attribute__((noinline)) void inspect_tls(long expected) {
  __asm__ volatile("" : : "r"(expected) : "memory");
}

static void set_values(long value) {
  tls_initialized = value;
  tls_zero = value * 2;
  tls_aligned = value * 3;
}

static void *worker(void *unused) {
  (void)unused;
  set_values(84);
  inspect_tls(84);
  return 0;
}

int main(void) {
  set_values(42);
  inspect_tls(42);
  pthread_t thread;
  if (pthread_create(&thread, 0, worker, 0) || pthread_join(thread, 0))
    return 1;
  inspect_tls(42);
  return 0;
}
