#if defined(__linux__)
#define _GNU_SOURCE
#endif

#include <locale.h>
#include <math.h>
#include <pthread.h>
#include <stdlib.h>
#include <string.h>
#if defined(__APPLE__)
#include <xlocale.h>
#endif

#include "floating_abi.h"
#include "value_shape.h"

static pthread_once_t locale_once = PTHREAD_ONCE_INIT;
static locale_t number_locale;

static void initialize_number_locale(void) {
    number_locale = newlocale(LC_NUMERIC_MASK, "C", (locale_t)0);
    if (number_locale == (locale_t)0) {
        scoop_shape_fatal("cannot create the C numeric locale");
    }
}

static char *terminated_number(const ScoopString *text) {
    if (pthread_once(&locale_once, initialize_number_locale) != 0) {
        scoop_shape_fatal("cannot initialize the C numeric locale");
    }
    if (text->len >= SIZE_MAX) {
        scoop_shape_fatal("numeric text allocation size overflow");
    }
    char *buffer = malloc((size_t)text->len + 1);
    if (buffer == NULL) {
        scoop_shape_fatal("numeric text allocation failed");
    }
    memcpy(buffer, text->data, (size_t)text->len);
    buffer[text->len] = '\0';
    return buffer;
}

void scoop_rt_json_parse_float_storage(ScoopFloatResult *result,
                                       const ScoopString *text) {
    char *buffer = terminated_number(text);
    char *end;
    float value = strtof_l(buffer, &end, number_locale);
    *result = (ScoopFloatResult){.tag = 1};
    /* ERANGE also reports valid subnormal and signed-zero underflow. */
    if (end != buffer && end == buffer + text->len && isfinite(value)) {
        result->tag = 0;
        result->value = value;
    }
    free(buffer);
}

void scoop_rt_json_parse_double_storage(ScoopDoubleResult *result,
                                        const ScoopString *text) {
    char *buffer = terminated_number(text);
    char *end;
    double value = strtod_l(buffer, &end, number_locale);
    *result = (ScoopDoubleResult){.tag = 1};
    if (end != buffer && end == buffer + text->len && isfinite(value)) {
        result->tag = 0;
        result->value = value;
    }
    free(buffer);
}
