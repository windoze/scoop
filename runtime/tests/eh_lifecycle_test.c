#include <errno.h>
#include <pthread.h>
#include <setjmp.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <unistd.h>

#include "eh_internal.h"
#include "generated_entries.h"
#include "scoop_rt.h"
#include "thread.h"

typedef struct TestException {
    ScoopObjectHeader header;
    ScoopArray *snapshot;
    uint64_t code;
    void *reference;
} TestException;

static const ScoopTypeDescriptor test_exception_td = {
    .type_id = UINT64_C(0x2501),
    .instance_shape =
        {
            .instance_kind = SCOOP_TYPE_INSTANCE_FIXED_OBJECT_V1,
            .inline_storage_kind = SCOOP_INLINE_STORAGE_NONE_V1,
            .minimum_size = sizeof(TestException),
            .instance_alignment = _Alignof(TestException),
        },
    .object_scan = NULL,
    .parent = NULL,
    .vtable = NULL,
    .itables = NULL,
    .itable_count = 0,
    .diagnostic_name = {(const uint8_t *)"test.M25Exception", sizeof("test.M25Exception") - 1}};

enum { TEST_CAPACITY = 8, CONCURRENT_THREAD_COUNT = 4 };

static _Thread_local ScoopThreadState test_thread;
static _Thread_local jmp_buf raise_jump;
static _Thread_local bool raise_jump_armed;
static _Thread_local struct _Unwind_Exception *raise_history[TEST_CAPACITY];
static _Thread_local size_t raise_count;
static _Thread_local uint64_t delete_count;

static _Noreturn void test_fail(const char *condition, int line) {
    fprintf(stderr, "EH lifecycle test failed at line %d: %s\n", line, condition);
    exit(1);
}

#define CHECK(condition)                                                                           \
    do {                                                                                           \
        if (!(condition)) {                                                                        \
            test_fail(#condition, __LINE__);                                                       \
        }                                                                                          \
    } while (0)

static uint64_t active_record_count(void) {
    uint64_t count = 0;
    for (ScoopExceptionRecord *record = test_thread.active_exceptions; record != NULL;
         record = record->active_next) {
        count++;
    }
    return count;
}

static uint64_t caught_frame_count(void) {
    uint64_t count = 0;
    for (ScoopExceptionRecord *record = test_thread.caught_exception_top; record != NULL;
         record = record->caught_previous) {
        count++;
    }
    return count;
}

static bool active_roots_contain(const void *object) {
    for (ScoopExceptionRecord *record = test_thread.active_exceptions; record != NULL;
         record = record->active_next) {
        if (record->object == object) {
            return true;
        }
    }
    return false;
}

static void initialize_current_test_thread(void) {
    memset(&test_thread, 0, sizeof test_thread);
    delete_count = 0;
    atomic_init(&test_thread.poll.mode, SCOOP_THREAD_MANAGED);
    test_thread.managed_depth = 1;
    raise_jump_armed = false;
    memset(raise_history, 0, sizeof raise_history);
    raise_count = 0;
}

static void reset_lifecycle(void) {
    CHECK(test_thread.active_exceptions == NULL);
    CHECK(test_thread.caught_exception_top == NULL);
    delete_count = 0;
    raise_jump_armed = false;
    memset(raise_history, 0, sizeof raise_history);
    raise_count = 0;
}

static TestException make_exception(uint64_t code, void *reference) {
    return (TestException){
        .header = {.td = &test_exception_td, .gc_word = 0},
        .code = code,
        .reference = reference,
    };
}

static struct _Unwind_Exception *capture_throw(const void *object) {
    size_t previous_raise_count = raise_count;
    if (setjmp(raise_jump) == 0) {
        raise_jump_armed = true;
        scoop_rt_throw(object);
    }
    raise_jump_armed = false;
    CHECK(raise_count == previous_raise_count + 1);
    return raise_history[raise_count - 1];
}

static struct _Unwind_Exception *capture_rethrow(void) {
    size_t previous_raise_count = raise_count;
    if (setjmp(raise_jump) == 0) {
        raise_jump_armed = true;
        scoop_rt_rethrow();
    }
    raise_jump_armed = false;
    CHECK(raise_count == previous_raise_count + 1);
    return raise_history[raise_count - 1];
}

static ScoopExceptionRecord *record_from_unwind(struct _Unwind_Exception *unwind) {
    return (ScoopExceptionRecord *)((unsigned char *)unwind -
                                    offsetof(ScoopExceptionRecord, unwind));
}

static void assert_runtime_empty(void) {
    CHECK(active_record_count() == 0);
    CHECK(caught_frame_count() == 0);
    CHECK(test_thread.caught_exception_top == NULL);

    CHECK(test_thread.active_exceptions == NULL);
    scoop_eh_prepare_shutdown();
}

static void test_throw_begin_end_preserves_identity_and_releases(void) {
    reset_lifecycle();
    int referenced_value = 17;
    _Alignas(32) TestException source =
        make_exception(UINT64_C(0x1122334455667788), &referenced_value);

    struct _Unwind_Exception *unwind = capture_throw(&source);
    CHECK(unwind != NULL);
    CHECK((uint64_t)unwind->exception_class == SCOOP_EXCEPTION_CLASS);
    CHECK(active_record_count() == 1);
    CHECK(caught_frame_count() == 0);

    TestException *payload = scoop_rt_begin_catch(unwind);
    CHECK(payload == &source);
    CHECK((uintptr_t)payload % test_exception_td.instance_shape.instance_alignment == 0);
    CHECK(payload->header.td == &test_exception_td);
    CHECK(payload->header.gc_word == source.header.gc_word);
    CHECK(payload->code == source.code);
    CHECK(payload->reference == source.reference);
    payload->code++;
    CHECK(source.code == UINT64_C(0x1122334455667789));
    CHECK(active_roots_contain(payload));
    CHECK(test_thread.caught_exception_top == record_from_unwind(unwind));
    CHECK(active_record_count() == 1);
    CHECK(caught_frame_count() == 1);

    scoop_rt_end_catch();
    CHECK(delete_count == 1);
    CHECK(!active_roots_contain(payload));
    assert_runtime_empty();
}

static void test_rethrow_preserves_record_and_object_identity(void) {
    reset_lifecycle();
    int referenced_value = 23;
    _Alignas(32) TestException source = make_exception(200, &referenced_value);

    struct _Unwind_Exception *unwind = capture_throw(&source);
    void *inner_payload = scoop_rt_begin_catch(unwind);
    CHECK(caught_frame_count() == 1);

    struct _Unwind_Exception *rethrown = capture_rethrow();
    CHECK(rethrown == unwind);
    CHECK(raise_count == 2);
    CHECK(raise_history[0] == raise_history[1]);
    CHECK(record_from_unwind(unwind)->state == SCOOP_EXCEPTION_RETHROWING);
    CHECK(active_record_count() == 1);
    CHECK(caught_frame_count() == 1);

    /* The cleanup edge of the inner handler releases caught ownership, but
     * the rethrown record and its root slot remain alive. */
    scoop_rt_end_catch();
    CHECK(record_from_unwind(unwind)->state == SCOOP_EXCEPTION_IN_FLIGHT);
    CHECK(delete_count == 0);
    CHECK(active_record_count() == 1);
    CHECK(caught_frame_count() == 0);

    void *outer_payload = scoop_rt_begin_catch(rethrown);
    CHECK(outer_payload == inner_payload);
    scoop_rt_end_catch();
    CHECK(delete_count == 1);
    assert_runtime_empty();
}

static void test_new_throw_releases_old_handler_record_first(void) {
    reset_lifecycle();
    int first_reference = 31;
    int second_reference = 37;
    _Alignas(32) TestException first = make_exception(300, &first_reference);
    _Alignas(32) TestException second = make_exception(400, &second_reference);

    struct _Unwind_Exception *first_unwind = capture_throw(&first);
    void *first_payload = scoop_rt_begin_catch(first_unwind);
    struct _Unwind_Exception *second_unwind = capture_throw(&second);
    CHECK(second_unwind != first_unwind);
    CHECK(active_record_count() == 2);
    CHECK(caught_frame_count() == 1);

    /* Unwinding the first handler runs its normal cleanup before the outer
     * handler begins catching the replacement exception. */
    scoop_rt_end_catch();
    CHECK(delete_count == 1);
    CHECK(!active_roots_contain(first_payload));
    CHECK(active_record_count() == 1);
    CHECK(caught_frame_count() == 0);

    TestException *second_payload = scoop_rt_begin_catch(second_unwind);
    CHECK(second_payload->code == second.code);
    CHECK(second_payload->reference == second.reference);
    CHECK(active_roots_contain(second_payload));
    scoop_rt_end_catch();
    CHECK(delete_count == 2);
    assert_runtime_empty();
}

static void test_catches_resolve_the_relocated_original_object(void) {
    reset_lifecycle();
    int reference = 41;
    _Alignas(32) TestException source = make_exception(500, &reference);
    _Alignas(32) TestException moved = source;
    struct _Unwind_Exception *unwind = capture_throw(&source);
    ScoopExceptionRecord *record = record_from_unwind(unwind);

    record->object = &moved;
    TestException *caught = scoop_rt_begin_catch(unwind);
    CHECK(caught == &moved);
    caught->code = 501;
    CHECK(moved.code == 501);

    struct _Unwind_Exception *rethrown = capture_rethrow();
    scoop_rt_end_catch();
    _Alignas(32) TestException moved_again = moved;
    record->object = &moved_again;
    caught = scoop_rt_begin_catch(rethrown);
    CHECK(caught == &moved_again);
    CHECK(caught->code == 501);
    CHECK(caught->reference == &reference);
    scoop_rt_end_catch();
    CHECK(!active_roots_contain(&moved_again));
    assert_runtime_empty();
}

static void test_new_throw_of_caught_object_has_independent_ownership(void) {
    reset_lifecycle();
    _Alignas(32) TestException source = make_exception(550, NULL);
    struct _Unwind_Exception *first = capture_throw(&source);
    TestException *caught = scoop_rt_begin_catch(first);
    struct _Unwind_Exception *second = capture_throw(caught);
    CHECK(first != second);
    CHECK(active_record_count() == 2);
    scoop_rt_end_catch();
    CHECK(active_record_count() == 1);
    CHECK(active_roots_contain(&source));
    CHECK(scoop_rt_begin_catch(second) == &source);
    scoop_rt_end_catch();
    assert_runtime_empty();
}

typedef struct ConcurrentGate {
    pthread_mutex_t mutex;
    pthread_cond_t changed;
    size_t thrown_ready;
    size_t caught_ready;
    unsigned phase;
} ConcurrentGate;

typedef struct ConcurrentWorker {
    size_t index;
    ConcurrentGate *gate;
    int referenced_value;
    _Alignas(32) TestException source;
    ScoopThreadState *thread_state;
    ScoopExceptionRecord *record;
    struct _Unwind_Exception *unwind;
    void *payload;
    size_t observed_raise_count;
    uint64_t observed_delete_count;
    bool stacks_empty;
} ConcurrentWorker;

static void concurrent_gate_init(ConcurrentGate *gate) {
    memset(gate, 0, sizeof *gate);
    CHECK(pthread_mutex_init(&gate->mutex, NULL) == 0);
    CHECK(pthread_cond_init(&gate->changed, NULL) == 0);
}

static void concurrent_gate_destroy(ConcurrentGate *gate) {
    CHECK(pthread_cond_destroy(&gate->changed) == 0);
    CHECK(pthread_mutex_destroy(&gate->mutex) == 0);
}

static void *run_concurrent_exception_lifecycle(void *raw_worker) {
    ConcurrentWorker *worker = raw_worker;
    ConcurrentGate *gate = worker->gate;
    initialize_current_test_thread();
    worker->thread_state = &test_thread;

    worker->unwind = capture_throw(&worker->source);
    worker->record = record_from_unwind(worker->unwind);
    CHECK(worker->record->owner == worker->thread_state);
    CHECK(worker->record->state == SCOOP_EXCEPTION_IN_FLIGHT);
    CHECK(worker->thread_state->active_exceptions == worker->record);
    CHECK(worker->record->active_previous == NULL);
    CHECK(worker->record->active_next == NULL);
    CHECK(active_record_count() == 1);
    CHECK(caught_frame_count() == 0);
    CHECK(worker->thread_state->caught_exception_top == NULL);

    CHECK(pthread_mutex_lock(&gate->mutex) == 0);
    gate->thrown_ready++;
    CHECK(gate->thrown_ready <= CONCURRENT_THREAD_COUNT);
    CHECK(pthread_cond_broadcast(&gate->changed) == 0);
    while (gate->phase < 1) {
        CHECK(pthread_cond_wait(&gate->changed, &gate->mutex) == 0);
    }
    CHECK(pthread_mutex_unlock(&gate->mutex) == 0);

    worker->payload = scoop_rt_begin_catch(worker->unwind);
    CHECK(worker->payload == &worker->source);
    CHECK(((TestException *)worker->payload)->code == worker->source.code);
    CHECK(((TestException *)worker->payload)->reference == worker->source.reference);
    CHECK(worker->thread_state->caught_exception_top == worker->record);

    CHECK(pthread_mutex_lock(&gate->mutex) == 0);
    gate->caught_ready++;
    CHECK(gate->caught_ready <= CONCURRENT_THREAD_COUNT);
    CHECK(pthread_cond_broadcast(&gate->changed) == 0);
    while (gate->phase < 2) {
        CHECK(pthread_cond_wait(&gate->changed, &gate->mutex) == 0);
    }
    CHECK(pthread_mutex_unlock(&gate->mutex) == 0);

    if ((worker->index & 1) != 0) {
        struct _Unwind_Exception *rethrown = capture_rethrow();
        CHECK(rethrown == worker->unwind);
        CHECK(worker->record->state == SCOOP_EXCEPTION_RETHROWING);
        CHECK(worker->thread_state->caught_exception_top == worker->record);
        scoop_rt_end_catch();
        CHECK(worker->record->state == SCOOP_EXCEPTION_IN_FLIGHT);
        CHECK(worker->thread_state->caught_exception_top == NULL);
        void *outer_payload = scoop_rt_begin_catch(rethrown);
        CHECK(outer_payload == worker->payload);
        CHECK(worker->thread_state->caught_exception_top == worker->record);
    }

    scoop_rt_end_catch();
    worker->stacks_empty = worker->thread_state->caught_exception_top == NULL &&
                           worker->thread_state->active_exceptions == NULL;
    worker->observed_delete_count = delete_count;
    worker->observed_raise_count = raise_count;
    return NULL;
}

static void test_concurrent_threads_have_isolated_caught_stacks(void) {
    reset_lifecycle();
    ConcurrentGate gate;
    ConcurrentWorker workers[CONCURRENT_THREAD_COUNT] = {0};
    pthread_t threads[CONCURRENT_THREAD_COUNT];
    concurrent_gate_init(&gate);

    for (size_t index = 0; index < CONCURRENT_THREAD_COUNT; index++) {
        workers[index].index = index;
        workers[index].gate = &gate;
        workers[index].referenced_value = (int)(1000 + index);
        workers[index].source =
            make_exception(UINT64_C(800) + index, &workers[index].referenced_value);
    }
    for (size_t index = 0; index < CONCURRENT_THREAD_COUNT; index++) {
        CHECK(pthread_create(&threads[index], NULL, run_concurrent_exception_lifecycle,
                             &workers[index]) == 0);
    }

    CHECK(pthread_mutex_lock(&gate.mutex) == 0);
    while (gate.thrown_ready < CONCURRENT_THREAD_COUNT) {
        CHECK(pthread_cond_wait(&gate.changed, &gate.mutex) == 0);
    }
    CHECK(active_record_count() == 0);
    CHECK(caught_frame_count() == 0);
    for (size_t index = 0; index < CONCURRENT_THREAD_COUNT; index++) {
        CHECK(workers[index].thread_state != NULL);
        CHECK(workers[index].record != NULL);
        CHECK(workers[index].unwind != NULL);
        CHECK(workers[index].record->owner == workers[index].thread_state);
        CHECK(workers[index].record->state == SCOOP_EXCEPTION_IN_FLIGHT);
        CHECK(workers[index].thread_state->caught_exception_top == NULL);
        for (size_t previous = 0; previous < index; previous++) {
            CHECK(workers[index].thread_state != workers[previous].thread_state);
            CHECK(workers[index].record != workers[previous].record);
        }
    }

    gate.phase = 1;
    CHECK(pthread_cond_broadcast(&gate.changed) == 0);
    while (gate.caught_ready < CONCURRENT_THREAD_COUNT) {
        CHECK(pthread_cond_wait(&gate.changed, &gate.mutex) == 0);
    }
    CHECK(active_record_count() == 0);
    CHECK(caught_frame_count() == 0);
    for (size_t index = 0; index < CONCURRENT_THREAD_COUNT; index++) {
        CHECK(workers[index].record->state == SCOOP_EXCEPTION_CAUGHT);
        CHECK(workers[index].thread_state->caught_exception_top == workers[index].record);
        CHECK(workers[index].payload == &workers[index].source);
        CHECK(workers[index].thread_state->active_exceptions == workers[index].record);
        CHECK(workers[index].record->object == workers[index].payload);
    }

    gate.phase = 2;
    CHECK(pthread_cond_broadcast(&gate.changed) == 0);
    CHECK(pthread_mutex_unlock(&gate.mutex) == 0);

    for (size_t index = 0; index < CONCURRENT_THREAD_COUNT; index++) {
        CHECK(pthread_join(threads[index], NULL) == 0);
        CHECK(workers[index].stacks_empty);
        CHECK(workers[index].observed_delete_count == 1);
        CHECK(workers[index].observed_raise_count == (((index & 1) != 0) ? 2 : 1));
    }
    concurrent_gate_destroy(&gate);

    assert_runtime_empty();
}

typedef void (*FatalStateCase)(void);

static void expect_abort_with_stderr(FatalStateCase test_case, const char *expected_stderr) {
    int stderr_pipe[2];
    CHECK(pipe(stderr_pipe) == 0);
    pid_t child = fork();
    CHECK(child >= 0);
    if (child == 0) {
        close(stderr_pipe[0]);
        if (dup2(stderr_pipe[1], STDERR_FILENO) < 0) {
            _exit(120);
        }
        close(stderr_pipe[1]);
        test_case();
        _exit(121);
    }

    close(stderr_pipe[1]);
    char actual_stderr[512];
    size_t actual_length = 0;
    for (;;) {
        CHECK(actual_length < sizeof actual_stderr - 1);
        ssize_t length = read(stderr_pipe[0], actual_stderr + actual_length,
                              sizeof actual_stderr - 1 - actual_length);
        if (length > 0) {
            actual_length += (size_t)length;
            continue;
        }
        if (length < 0 && errno == EINTR) {
            continue;
        }
        CHECK(length == 0);
        break;
    }
    close(stderr_pipe[0]);
    actual_stderr[actual_length] = '\0';

    int status = 0;
    pid_t waited;
    do {
        waited = waitpid(child, &status, 0);
    } while (waited < 0 && errno == EINTR);
    CHECK(waited == child);
    CHECK(WIFSIGNALED(status));
    CHECK(WTERMSIG(status) == SIGABRT);
    CHECK(strcmp(actual_stderr, expected_stderr) == 0);

    /* Every fatal case runs in a copy-on-write child. Its deliberately live
     * records must not mutate the parent's lifecycle registries. */
    assert_runtime_empty();
}

static void end_catch_with_empty_stack(void) { scoop_rt_end_catch(); }

static void rethrow_with_empty_stack(void) { scoop_rt_rethrow(); }

static void begin_same_record_twice(void) {
    int referenced_value = 47;
    _Alignas(32) TestException source = make_exception(600, &referenced_value);
    struct _Unwind_Exception *unwind = capture_throw(&source);
    (void)scoop_rt_begin_catch(unwind);
    (void)scoop_rt_begin_catch(unwind);
}

static void test_invalid_catch_states_abort_with_stable_diagnostics(void) {
    reset_lifecycle();
    expect_abort_with_stderr(end_catch_with_empty_stack,
                             "scoop: fatal exception state error: end-catch has no active catch "
                             "frame\n");
    expect_abort_with_stderr(rethrow_with_empty_stack,
                             "scoop: fatal exception state error: rethrow has no current caught "
                             "exception\n");
    expect_abort_with_stderr(begin_same_record_twice,
                             "scoop: fatal exception state error: begin-catch observed an invalid "
                             "record state\n");
}

_Noreturn void scoop_thread_fatal(const char *message) { test_fail(message, __LINE__); }

ScoopThreadState *scoop_thread_current(void) { return &test_thread; }

ScoopThreadState *scoop_thread_current_required(void) { return &test_thread; }

ScoopThreadState *scoop_thread_require_managed(void) {
    CHECK(atomic_load_explicit(&test_thread.poll.mode, memory_order_relaxed) ==
          SCOOP_THREAD_MANAGED);
    CHECK(test_thread.managed_depth == 1);
    return &test_thread;
}

_Unwind_Reason_Code _Unwind_RaiseException(struct _Unwind_Exception *exception) {
    CHECK(exception != NULL);
    CHECK(raise_jump_armed);
    CHECK(raise_count < TEST_CAPACITY);
    raise_history[raise_count++] = exception;
    raise_jump_armed = false;
    longjmp(raise_jump, 1);
}

void _Unwind_DeleteException(struct _Unwind_Exception *exception) {
    CHECK(exception != NULL);
    CHECK(exception->exception_cleanup != NULL);
    delete_count++;
    exception->exception_cleanup(_URC_NO_REASON, exception);
}

int main(void) {
    initialize_current_test_thread();

    test_throw_begin_end_preserves_identity_and_releases();
    test_rethrow_preserves_record_and_object_identity();
    test_new_throw_releases_old_handler_record_first();
    test_catches_resolve_the_relocated_original_object();
    test_new_throw_of_caught_object_has_independent_ownership();
    test_concurrent_threads_have_isolated_caught_stacks();
    test_invalid_catch_states_abort_with_stable_diagnostics();

    puts("EH lifecycle tests passed");
    return 0;
}
