#include "platform/numeric.h"
#include <math.h>
#include <stdlib.h>
#include <string.h>

#include "floating_abi.h"
#include "value_shape.h"

static char *terminated_number(const ScoopString *text, char *scratch, size_t capacity) {
    if (text->len >= SIZE_MAX) {
        scoop_shape_fatal("numeric text allocation size overflow");
    }
    char *buffer = text->len < capacity ? scratch : malloc((size_t)text->len + 1);
    if (buffer == NULL) {
        scoop_shape_fatal("numeric text allocation failed");
    }
    memcpy(buffer, text->data, (size_t)text->len);
    buffer[text->len] = '\0';
    return buffer;
}

void scoop_rt_json_parse_float_storage(ScoopFloatResult *result,
                                       const ScoopString *text) {
    char scratch[256];
    char *buffer = terminated_number(text, scratch, sizeof scratch);
    char *end;
    float value = scoop_platform_parse_float(buffer, &end);
    *result = (ScoopFloatResult){.tag = 1};
    /* ERANGE also reports valid subnormal and signed-zero underflow. */
    if (end != buffer && end == buffer + text->len && isfinite(value)) {
        result->tag = 0;
        result->value = value;
    }
    if (buffer != scratch) {
        free(buffer);
    }
}

void scoop_rt_json_parse_double_storage(ScoopDoubleResult *result,
                                        const ScoopString *text) {
    char scratch[256];
    char *buffer = terminated_number(text, scratch, sizeof scratch);
    char *end;
    double value = scoop_platform_parse_double(buffer, &end);
    *result = (ScoopDoubleResult){.tag = 1};
    if (end != buffer && end == buffer + text->len && isfinite(value)) {
        result->tag = 0;
        result->value = value;
    }
    if (buffer != scratch) {
        free(buffer);
    }
}
