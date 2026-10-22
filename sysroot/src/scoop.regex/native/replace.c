#include "regex.h"
#include <stdlib.h>

static bool replacement_error(int code) {
    switch (code) {
    case PCRE2_ERROR_BADREPLACEMENT:
    case PCRE2_ERROR_BADREPESCAPE:
    case PCRE2_ERROR_REPMISSINGBRACE:
    case PCRE2_ERROR_BADSUBSTITUTION:
    case PCRE2_ERROR_NOSUBSTRING:
    case PCRE2_ERROR_UNSET:
        return true;
    default:
        return false;
    }
}

ScoopRegexText scoop_regex_substitute(uint64_t handle, const uint8_t *input, int64_t input_length,
                                      const uint8_t *replacement, int64_t replacement_length,
                                      bool first, bool literal) {
    uint32_t options =
        PCRE2_NO_UTF_CHECK | PCRE2_SUBSTITUTE_UNSET_EMPTY | PCRE2_SUBSTITUTE_OVERFLOW_LENGTH;
    if (!first)
        options |= PCRE2_SUBSTITUTE_GLOBAL;
    if (literal)
        options |= PCRE2_SUBSTITUTE_LITERAL;
    PCRE2_SIZE capacity = (PCRE2_SIZE)input_length;
    if (capacity <= INT64_MAX - 64)
        capacity += 64;
    uint8_t *buffer = malloc(capacity);
    if (!buffer)
        return (ScoopRegexText){.code = PCRE2_ERROR_NOMEMORY, .offset = -1};
    PCRE2_SIZE length = capacity;
    int rc =
        pcre2_substitute(scoop_regex_code(handle), input, (PCRE2_SIZE)input_length, 0, options,
                         NULL, NULL, replacement, (PCRE2_SIZE)replacement_length, buffer, &length);
    if (rc == PCRE2_ERROR_NOMEMORY && length != PCRE2_UNSET && length > capacity &&
        length <= INT64_MAX) {
        uint8_t *larger = realloc(buffer, length);
        if (!larger) {
            free(buffer);
            return (ScoopRegexText){.code = PCRE2_ERROR_NOMEMORY, .offset = -1};
        }
        buffer = larger;
        rc = pcre2_substitute(scoop_regex_code(handle), input, (PCRE2_SIZE)input_length, 0, options,
                              NULL, NULL, replacement, (PCRE2_SIZE)replacement_length, buffer,
                              &length);
    }
    if (rc <= 0) {
        free(buffer);
        bool bad_template = replacement_error(rc);
        return (ScoopRegexText){.code = rc,
                                .replacement_error = bad_template,
                                .offset =
                                    bad_template && length != PCRE2_UNSET ? (int64_t)length : -1};
    }
    return (ScoopRegexText){buffer, (int64_t)length, rc, -1, false};
}

void scoop_regex_text_free(uint8_t *data) { free(data); }
