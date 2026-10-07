#include "no_core.h"
#include <assert.h>
#include <pthread.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include "../src/gc/gc_internal.h"
#include "../src/managed_entries.h"
#include "../src/startup/internal.h"
#include "../src/thread.h"
#include "platform/image_fixture.h"

typedef struct Failure {
    ScoopObjectHeader header;
    uint64_t value;
} Failure;

static const ScoopTypeDescriptor failure_type = {
    .type_id = 1,
    .instance_shape = {.instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
                       .minimum_size = sizeof(Failure),
                       .instance_alignment = 8},
    .diagnostic_name = {(const uint8_t *)"GatewayFailure", sizeof("GatewayFailure") - 1},
};
static Failure *failure;
static const uint64_t scan[] = {1, 0};
static const ScoopStaticStorageDescriptorV1 failure_storage = {
    .writable_base = &failure,
    .scan_program = scan,
};
static uint32_t result;

static void initialize_frame(uintptr_t frame[4]) {
    memset(frame, 0, 4 * sizeof *frame);
    frame[2] = (uintptr_t)scoop_thread_current_required()->managed_stack_boundary;
}

static uint32_t gateway(void) {
    _Alignas(16) uintptr_t frame[4];
    initialize_frame(frame);
    ScoopManagedAnchor anchor;
    scoop_thread_push_safepoint_anchor(&anchor, 0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    scoop_thread_poll();
    if (result == 1) {
        failure = scoop_gc_alloc_internal(&failure_type, sizeof *failure);
        failure->value = 42;
    }
    scoop_thread_pop_managed_anchor(&anchor);
    return result;
}

static void *move_failure(void *unused) {
    (void)unused;
    assert(scoop_rt_attach_foreign_thread());
    scoop_thread_enter_managed(__builtin_frame_address(0));
    _Alignas(16) uintptr_t frame[4];
    initialize_frame(frame);
    scoop_rt_gc_collect_impl(0x1010, (uintptr_t)frame, (uintptr_t)&frame[2]);
    scoop_thread_leave_managed();
    scoop_rt_detach_foreign_thread();
    return NULL;
}

static void run_failure(unsigned test) {
    if (test == 0) {
        result = 2;
        scoop_startup_call_gateway(gateway);
        abort();
    }
    if (test == 1) {
        failure = NULL;
        scoop_startup_report_failure(&failure_storage, NULL);
    }
    result = 1;
    assert(scoop_startup_call_gateway(gateway) == 1);
    assert(scoop_rt_thread_debug_mode() == SCOOP_THREAD_NATIVE_SAFE);
    uintptr_t before = (uintptr_t)failure;
    pthread_t collector;
    assert(pthread_create(&collector, NULL, move_failure, NULL) == 0);
    assert(pthread_join(collector, NULL) == 0);
    assert((uintptr_t)failure != before && failure->value == 42);
    ScoopInitializationCell cell = {.state = test == 2 ? 2 : 3};
    const ScoopInitializationUnitDescriptorV1 unit = {
        .cell = &cell,
        .failure_root = &failure_storage,
        .diagnostic_path = {(const uint8_t *)"fixture.unit", sizeof("fixture.unit") - 1},
    };
    scoop_startup_report_failure(&failure_storage, test == 3 ? NULL : &unit);
}

static void expect_failure(unsigned test, const char *message) {
    int diagnostics[2];
    assert(pipe(diagnostics) == 0);
    pid_t child = fork();
    assert(child >= 0);
    if (child == 0) {
        close(diagnostics[0]);
        assert(dup2(diagnostics[1], STDERR_FILENO) == STDERR_FILENO);
        close(diagnostics[1]);
        run_failure(test);
        _exit(1);
    }
    close(diagnostics[1]);
    char output[1024] = {0};
    size_t count = 0;
    ssize_t read_count;
    while ((read_count = read(diagnostics[0], output + count, sizeof output - count - 1)) > 0) {
        count += (size_t)read_count;
        assert(count < sizeof output - 1);
    }
    close(diagnostics[0]);
    int status;
    assert(waitpid(child, &status, 0) == child);
    if (test < 3) {
        assert(WIFSIGNALED(status) && WTERMSIG(status) == SIGABRT);
    } else {
        assert(WIFEXITED(status) && WEXITSTATUS(status) == 1);
    }
    assert(strstr(output, message) != NULL);
}

int main(void) {
    scoop_test_disable_core_dumps();
    assert(setenv("SCOOP_GC_STRESS_MOVE", "1", 1) == 0);
    const ScoopTypeDescriptor *types[] = {&failure_type};
    const ScoopStaticStorageDescriptorV1 *roots[] = {&failure_storage};
    scoop_thread_runtime_init();
    scoop_test_image_init(types, 1, roots, 1, NULL, 0);
    scoop_thread_attach_main();
    assert(scoop_startup_call_gateway(gateway) == 0);
    assert(scoop_rt_thread_debug_mode() == SCOOP_THREAD_NATIVE_SAFE);
    expect_failure(0, "gateway returned an invalid status");
    expect_failure(1, "gateway failure has no published exception");
    expect_failure(2, "gateway failure has no published exception");
    expect_failure(3, "uncaught exception: GatewayFailure\n");
    expect_failure(4, "uncaught exception: GatewayFailure during initialization of fixture.unit\n");
    scoop_thread_prepare_shutdown();
    scoop_thread_detach_main();
    scoop_thread_runtime_finish_shutdown();
    puts("startup gateway invariants passed");
    return 0;
}
