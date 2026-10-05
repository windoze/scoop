#include <errno.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#include "eh_internal.h"

enum {
    TEST_REGISTER_CAPACITY = 32,
    TEST_OUTPUT_CAPACITY = 512,
};

struct _Unwind_Context {
    uintptr_t region_start;
    uintptr_t instruction_pointer;
    const uint8_t *lsda;
    int written_registers[2];
    uintptr_t register_values[2];
    size_t register_write_count;
    uintptr_t installed_ip;
    size_t ip_write_count;
};

static const uint8_t cleanup_lsda[] = {
    SCOOP_EH_DW_PE_OMIT,
    SCOOP_EH_DW_PE_OMIT,
    SCOOP_EH_DW_PE_ULEB128,
    4,
    0,
    4,
    32,
    0,
};

static const uint8_t catch_lsda[] = {
    SCOOP_EH_DW_PE_OMIT,
    SCOOP_EH_DW_PE_PCREL_SDATA4_INDIRECT,
    12,
    SCOOP_EH_DW_PE_ULEB128,
    4,
    0,
    4,
    48,
    1,
    1,
    0,
    0,
    0,
    0,
    0,
};

static _Noreturn void test_fail(const char *condition, int line) {
    fprintf(stderr, "EH personality test failed at line %d: %s\n", line,
            condition);
    exit(1);
}

#define CHECK(condition)                                                       \
    do {                                                                       \
        if (!(condition)) {                                                    \
            test_fail(#condition, __LINE__);                                   \
        }                                                                      \
    } while (0)

uintptr_t _Unwind_GetRegionStart(struct _Unwind_Context *context) {
    return context->region_start;
}

/* GCC declares a pointer here; LLVM's header declares uintptr_t. */
typedef __typeof__(_Unwind_GetLanguageSpecificData(NULL)) TestLsdaAddress;

TestLsdaAddress
_Unwind_GetLanguageSpecificData(struct _Unwind_Context *context) {
    return (TestLsdaAddress)context->lsda;
}

uintptr_t _Unwind_GetIP(struct _Unwind_Context *context) {
    return context->instruction_pointer;
}

void _Unwind_SetGR(struct _Unwind_Context *context, int index,
                   uintptr_t value) {
    CHECK(index >= 0);
    CHECK(index < TEST_REGISTER_CAPACITY);
    CHECK(context->register_write_count < 2);
    size_t write = context->register_write_count++;
    context->written_registers[write] = index;
    context->register_values[write] = value;
}

void _Unwind_SetIP(struct _Unwind_Context *context, uintptr_t value) {
    CHECK(context->ip_write_count == 0);
    context->installed_ip = value;
    context->ip_write_count++;
}

static struct _Unwind_Context make_context(const uint8_t *lsda) {
    return (struct _Unwind_Context){
        .region_start = (uintptr_t)UINT64_C(0x1000),
        .instruction_pointer = (uintptr_t)UINT64_C(0x1001),
        .lsda = lsda,
    };
}

static _Unwind_Reason_Code run_personality(_Unwind_Action actions,
                                           struct _Unwind_Exception *exception,
                                           struct _Unwind_Context *context) {
    return scoop_eh_personality(1, actions,
                                (_Unwind_Exception_Class)SCOOP_EXCEPTION_CLASS,
                                exception, context);
}

static void check_no_context_install(const struct _Unwind_Context *context) {
    CHECK(context->register_write_count == 0);
    CHECK(context->ip_write_count == 0);
}

static void check_context_install(const struct _Unwind_Context *context,
                                  const struct _Unwind_Exception *exception,
                                  uintptr_t landing_pad, uintptr_t selector) {
    CHECK(context->register_write_count == 2);
    CHECK(context->written_registers[0] == __builtin_eh_return_data_regno(0));
    CHECK(context->register_values[0] == (uintptr_t)exception);
    CHECK(context->written_registers[1] == __builtin_eh_return_data_regno(1));
    CHECK(context->register_values[1] == selector);
    CHECK(context->ip_write_count == 1);
    CHECK(context->installed_ip == landing_pad);
}

static void test_search_phase(void) {
    struct _Unwind_Exception exception = {0};

    struct _Unwind_Context catch_context = make_context(catch_lsda);
    CHECK(run_personality(_UA_SEARCH_PHASE, &exception, &catch_context) ==
          _URC_HANDLER_FOUND);
    check_no_context_install(&catch_context);

    struct _Unwind_Context cleanup_context = make_context(cleanup_lsda);
    CHECK(run_personality(_UA_SEARCH_PHASE, &exception, &cleanup_context) ==
          _URC_CONTINUE_UNWIND);
    check_no_context_install(&cleanup_context);
}

static void test_cleanup_phase(void) {
    struct _Unwind_Exception exception = {0};

    struct _Unwind_Context cleanup_context = make_context(cleanup_lsda);
    CHECK(run_personality(_UA_CLEANUP_PHASE, &exception, &cleanup_context) ==
          _URC_INSTALL_CONTEXT);
    check_context_install(&cleanup_context, &exception,
                          cleanup_context.region_start + 32, 0);

    struct _Unwind_Context handler_context = make_context(catch_lsda);
    CHECK(run_personality(_UA_CLEANUP_PHASE | _UA_HANDLER_FRAME, &exception,
                          &handler_context) == _URC_INSTALL_CONTEXT);
    check_context_install(&handler_context, &exception,
                          handler_context.region_start + 48, 1);

    struct _Unwind_Context intermediate_context = make_context(catch_lsda);
    CHECK(run_personality(_UA_CLEANUP_PHASE, &exception,
                          &intermediate_context) == _URC_CONTINUE_UNWIND);
    check_no_context_install(&intermediate_context);
}

typedef enum FatalCase {
    FATAL_FOREIGN_CLASS,
    FATAL_MALFORMED_LSDA,
    FATAL_PHASE_MISMATCH,
    FATAL_FORCED_UNWIND,
} FatalCase;

static void run_fatal_case(FatalCase fatal_case) {
    struct _Unwind_Exception exception = {0};
    struct _Unwind_Context context = make_context(catch_lsda);
    switch (fatal_case) {
    case FATAL_FOREIGN_CLASS:
        (void)scoop_eh_personality(
            1, _UA_SEARCH_PHASE,
            (_Unwind_Exception_Class)(SCOOP_EXCEPTION_CLASS ^ UINT64_C(1)),
            &exception, &context);
        break;
    case FATAL_MALFORMED_LSDA:
        context.lsda = NULL;
        (void)run_personality(_UA_SEARCH_PHASE, &exception, &context);
        break;
    case FATAL_PHASE_MISMATCH:
        context.lsda = cleanup_lsda;
        (void)run_personality(_UA_CLEANUP_PHASE | _UA_HANDLER_FRAME, &exception,
                              &context);
        break;
    case FATAL_FORCED_UNWIND:
        (void)run_personality(_UA_CLEANUP_PHASE | _UA_FORCE_UNWIND, &exception,
                              &context);
        break;
    }
    _exit(90);
}

static void expect_fatal(FatalCase fatal_case, const char *expected_stderr) {
    int stderr_pipe[2];
    CHECK(pipe(stderr_pipe) == 0);
    pid_t child = fork();
    CHECK(child >= 0);
    if (child == 0) {
        CHECK(close(stderr_pipe[0]) == 0);
        CHECK(dup2(stderr_pipe[1], STDERR_FILENO) == STDERR_FILENO);
        CHECK(close(stderr_pipe[1]) == 0);
        run_fatal_case(fatal_case);
    }

    CHECK(close(stderr_pipe[1]) == 0);
    char output[TEST_OUTPUT_CAPACITY];
    size_t length = 0;
    for (;;) {
        CHECK(length < sizeof output - 1);
        ssize_t count =
            read(stderr_pipe[0], output + length, sizeof output - 1 - length);
        if (count > 0) {
            length += (size_t)count;
            continue;
        }
        if (count < 0 && errno == EINTR) {
            continue;
        }
        CHECK(count == 0);
        break;
    }
    output[length] = '\0';
    CHECK(close(stderr_pipe[0]) == 0);

    int status = 0;
    CHECK(waitpid(child, &status, 0) == child);
    CHECK(WIFSIGNALED(status));
    CHECK(WTERMSIG(status) == SIGABRT);
    CHECK(strcmp(output, expected_stderr) == 0);
}

static void test_fatal_contracts(void) {
    expect_fatal(FATAL_FOREIGN_CLASS,
                 "scoop: fatal unwind error: foreign exception entered "
                 "Scoop EH\n");
    expect_fatal(FATAL_MALFORMED_LSDA,
                 "scoop: fatal unwind error: malformed LSDA (null byte "
                 "range) in function 0x1000\n");
    expect_fatal(FATAL_PHASE_MISMATCH,
                 "scoop: fatal unwind error: handler action changed between "
                 "unwind phases\n");
    expect_fatal(FATAL_FORCED_UNWIND,
                 "scoop: fatal unwind error: forced unwind entered Scoop "
                 "EH\n");
}

int main(void) {
    CHECK(setvbuf(stderr, NULL, _IONBF, 0) == 0);
    test_search_phase();
    test_cleanup_phase();
    test_fatal_contracts();
    puts("EH personality tests passed");
    return 0;
}
