#if defined(__linux__)
#define _GNU_SOURCE
#endif
#include "../../value_shape.h"
#include "../numeric.h"
#include "../primitives.h"
#include <locale.h>
#include <stdlib.h>
#if defined(__APPLE__)
#include <xlocale.h>
#endif

static ScoopPlatformOnce locale_once = SCOOP_PLATFORM_ONCE_INIT;
static locale_t number_locale;

static void initialize_number_locale(void) {
    number_locale = newlocale(LC_NUMERIC_MASK, "C", (locale_t)0);
    if (number_locale == (locale_t)0)
        scoop_shape_fatal("cannot create the C numeric locale");
}

static locale_t numeric_locale(void) {
    if (scoop_platform_once(&locale_once, initialize_number_locale) != 0)
        scoop_shape_fatal("cannot initialize the C numeric locale");
    return number_locale;
}

float scoop_platform_parse_float(const char *text, char **end) {
    return strtof_l(text, end, numeric_locale());
}

double scoop_platform_parse_double(const char *text, char **end) {
    return strtod_l(text, end, numeric_locale());
}
