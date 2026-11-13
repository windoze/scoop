#include "profiling.h"

#if defined(SCOOP_ENABLE_PROFILING) && SCOOP_ENABLE_PROFILING
#include <errno.h>
#include <inttypes.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>

#include "gc.h"
#include "platform/primitives.h"
#include "process_termination.h"
#include "profiling/gc.h"
#include "profiling/path.h"

/* Own initialization and flushing, including Scoop's _exit path. This
 * definition suppresses compiler-rt's default constructor and atexit file
 * writer. */
int __llvm_profile_runtime;
extern uint64_t __llvm_profile_get_size_for_buffer(void);
extern int __llvm_profile_write_buffer(char *buffer);

static FILE *counter_file;
static FILE *metrics_file;
static uint64_t started_ns;
static _Atomic(bool) runtime_ready;
static atomic_flag finished = ATOMIC_FLAG_INIT;

static void write_counters(void) {
  uint64_t size = __llvm_profile_get_size_for_buffer();
  if (size > SIZE_MAX) {
    fputs("scoop profiling: counter buffer is too large\n", stderr);
    return;
  }
  char *buffer = malloc((size_t)size);
  if (buffer == NULL) {
    fputs("scoop profiling: cannot allocate counter buffer\n", stderr);
    return;
  }
  if (__llvm_profile_write_buffer(buffer) != 0 ||
      fwrite(buffer, 1, (size_t)size, counter_file) != size)
    fprintf(stderr, "scoop profiling: cannot write counters: %s\n",
            strerror(errno));
  free(buffer);
}

static void finish(const char *reason, const int32_t *exit_code) {
  if (counter_file == NULL ||
      atomic_flag_test_and_set_explicit(&finished, memory_order_acq_rel))
    return;
  uint64_t ended_ns;
  bool has_time = scoop_runtime_platform_monotonic_ns(&ended_ns);
  scoop_process_prepare_termination();
  if (atomic_load_explicit(&runtime_ready, memory_order_acquire))
    scoop_gc_write_metrics(metrics_file);
  scoop_profile_gc_write(metrics_file);
  fprintf(metrics_file,
          "{\"scoop_profile\":1,\"event\":\"finish\",\"reason\":\"%s\",\"exit_"
          "code\":",
          reason);
  if (exit_code != NULL)
    fprintf(metrics_file, "%" PRId32, *exit_code);
  else
    fputs("null", metrics_file);
  if (has_time && ended_ns >= started_ns)
    fprintf(metrics_file, ",\"runtime_elapsed_ns\":%" PRIu64,
            ended_ns - started_ns);
  fputs("}\n", metrics_file);
  write_counters();
  if (fclose(counter_file) != 0)
    fprintf(stderr, "scoop profiling: cannot close counters: %s\n",
            strerror(errno));
  if (fclose(metrics_file) != 0)
    fprintf(stderr, "scoop profiling: cannot close metrics: %s\n",
            strerror(errno));
}

static void finish_at_exit(void) { finish("process-exit", NULL); }

void scoop_profile_initialize(void) {
  if (!scoop_runtime_platform_monotonic_ns(&started_ns)) {
    fputs("scoop profiling: cannot read monotonic clock\n", stderr);
    return;
  }
  if (!scoop_profile_open_files(&counter_file, &metrics_file, started_ns)) {
    fprintf(
        stderr,
        "scoop profiling: cannot create output under SCOOP_PROFILE_DIR: %s\n",
        strerror(errno));
    return;
  }
  scoop_profile_gc_initialize(started_ns);
  if (atexit(finish_at_exit) != 0)
    fputs("scoop profiling: cannot register process exit writer\n", stderr);
}

void scoop_profile_runtime_ready(void) {
  atomic_store_explicit(&runtime_ready, true, memory_order_release);
}

void scoop_profile_finish(const char *reason, int32_t exit_code) {
  finish(reason, &exit_code);
}

FILE *scoop_profile_metrics_file(void) { return metrics_file; }
#endif
