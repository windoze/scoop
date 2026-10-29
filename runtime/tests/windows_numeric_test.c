#include "floating.h"
#include "floating_abi.h"
#include "platform/primitives.h"
#include "value_shape.h"
#include <assert.h>
#include <locale.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct TaggedParts {
    uint64_t tag, payload;
} TaggedParts;
extern TaggedParts scoop_rt_json_parse_float(const ScoopString *text);
extern TaggedParts scoop_rt_json_parse_double(const ScoopString *text);

_Noreturn void scoop_shape_fatal(const char *message) {
    fprintf(stderr, "numeric test: %s\n", message);
    abort();
}

static ScoopString *string(const char *text, size_t length) {
    ScoopString *result = calloc(1, sizeof *result + length);
    assert(result != NULL);
    result->len = length;
    memcpy(result->data, text, length);
    return result;
}

static void check_float(const char *text, uint64_t tag, uint32_t bits) {
    ScoopString *input = string(text, strlen(text));
    ScoopFloatResult result;
    scoop_rt_json_parse_float_storage(&result, input);
    uint32_t actual;
    memcpy(&actual, &result.value, sizeof actual);
    assert(result.tag == tag && (tag != 0 || actual == bits));
    TaggedParts parts = scoop_rt_json_parse_float(input);
    assert(parts.tag == tag && (tag != 0 || parts.payload == bits));
    free(input);
}

static void check_double(const char *text, uint64_t tag, uint64_t bits) {
    ScoopString *input = string(text, strlen(text));
    ScoopDoubleResult result;
    scoop_rt_json_parse_double_storage(&result, input);
    uint64_t actual;
    memcpy(&actual, &result.value, sizeof actual);
    assert(result.tag == tag && (tag != 0 || actual == bits));
    TaggedParts parts = scoop_rt_json_parse_double(input);
    assert(parts.tag == tag && (tag != 0 || parts.payload == bits));
    free(input);
}

static void cases(void) {
    check_float("1.000000059604644775390625", 0, 0x3f800000);
    check_float("1.000000059604644775390625000001", 0, 0x3f800001);
    check_float("1.000000178813934326171875", 0, 0x3f800002);
    check_float("1e-45", 0, 1);
    check_float("-1e-10000", 0, 0x80000000);
    check_float("3.40282346638528859811704183484516925440e38", 0, 0x7f7fffff);
    check_float("3.40282356779733661637539395458142568448e38", 1, 0);
    check_double("1.00000000000000011102230246251565404236316680908203125", 0,
                 UINT64_C(0x3ff0000000000000));
    check_double("1.0000000000000001110223024625156540423631668090820312500001", 0,
                 UINT64_C(0x3ff0000000000001));
    check_double("4.9406564584124654e-324", 0, 1);
    check_double("-1e-10000", 0, UINT64_C(0x8000000000000000));
    check_double("1.7976931348623157e308", 0, UINT64_C(0x7fefffffffffffff));
    check_double("1.7976931348623159e308", 1, 0);
    const char *invalid[] = {"", "1,25", "1x", "NaN", "Infinity", "1e10000"};
    for (size_t index = 0; index < sizeof invalid / sizeof *invalid; ++index) {
        check_float(invalid[index], 1, 0);
        check_double(invalid[index], 1, 0);
    }
    char long_number[400];
    memset(long_number, '0', sizeof long_number);
    const char midpoint[] = "1.000000059604644775390625";
    memcpy(long_number, midpoint, sizeof midpoint - 1);
    long_number[sizeof long_number - 2] = '1';
    long_number[sizeof long_number - 1] = 0;
    check_float(long_number, 0, 0x3f800001);
    ScoopString *embedded = string("1\0suffix", 8);
    assert(scoop_rt_json_parse_float(embedded).tag == 1);
    assert(scoop_rt_json_parse_double(embedded).tag == 1);
    free(embedded);
}

static void *worker(void *argument) {
    assert(scoop_float_init_environment());
#if defined(__MINGW32__) && !defined(_UCRT)
    (void)argument;
    int mode = _DISABLE_PER_THREAD_LOCALE;
#else
    int mode = (uintptr_t)argument % 2 ? _ENABLE_PER_THREAD_LOCALE : _DISABLE_PER_THREAD_LOCALE;
#endif
    assert(_configthreadlocale(mode) != -1);
    if (mode == _ENABLE_PER_THREAD_LOCALE)
        assert(setlocale(LC_NUMERIC, "German_Germany.1252") != NULL);
    char *name = _strdup(setlocale(LC_NUMERIC, NULL));
    assert(name != NULL && strcmp(localeconv()->decimal_point, ",") == 0);
    for (unsigned iteration = 0; iteration < 20; ++iteration) {
        cases();
        assert(_configthreadlocale(0) == mode);
        assert(strcmp(setlocale(LC_NUMERIC, NULL), name) == 0);
        assert(strcmp(localeconv()->decimal_point, ",") == 0);
    }
    free(name);
    return NULL;
}

int wmain(int argc, uint16_t **argv) {
    (void)argc;
    (void)argv;
    assert(_configthreadlocale(_DISABLE_PER_THREAD_LOCALE) != -1);
    assert(setlocale(LC_NUMERIC, "French_France.1252") != NULL);
    char *name = _strdup(setlocale(LC_NUMERIC, NULL));
    assert(name != NULL);
    ScoopPlatformThread threads[4];
    for (uintptr_t index = 0; index < 4; ++index)
        assert(scoop_runtime_platform_thread_create(&threads[index], worker, (void *)index) == 0);
    for (unsigned index = 0; index < 4; ++index)
        assert(scoop_runtime_platform_thread_join(threads[index]) == 0);
    assert(_configthreadlocale(0) == _DISABLE_PER_THREAD_LOCALE);
    assert(strcmp(setlocale(LC_NUMERIC, NULL), name) == 0);
    free(name);
    puts("direct Float/Double rounding, long input, subnormals and native locales passed");
    return 0;
}
