#include "../../value_shape.h"
#include "../numeric.h"
#include "../primitives.h"

extern float scoop_gdtoa_strtof(const char *text, char **end);
extern double scoop_gdtoa_strtod(const char *text, char **end);

static ScoopPlatformMutex locks[2] = {SCOOP_PLATFORM_MUTEX_INIT, SCOOP_PLATFORM_MUTEX_INIT};

void scoop_gdtoa_lock(unsigned index) {
    if (scoop_platform_mutex_lock(&locks[index]) != 0)
        scoop_shape_fatal("cannot lock the numeric parser");
}

void scoop_gdtoa_unlock(unsigned index) {
    if (scoop_platform_mutex_unlock(&locks[index]) != 0)
        scoop_shape_fatal("cannot unlock the numeric parser");
}

float scoop_platform_parse_float(const char *text, char **end) {
    return scoop_gdtoa_strtof(text, end);
}

double scoop_platform_parse_double(const char *text, char **end) {
    return scoop_gdtoa_strtod(text, end);
}
