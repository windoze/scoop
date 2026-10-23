#include "regex.h"

ScoopRegexMatch scoop_regex_match(uint64_t handle, const uint8_t *input, int64_t length,
                                  int64_t start, uint32_t options, int32_t mode, bool captures) {
    pcre2_code *code = scoop_regex_code(handle);
    pcre2_match_data *data = captures ? pcre2_match_data_create_from_pattern(code, NULL)
                                      : pcre2_match_data_create(1, NULL);
    if (!data)
        return (ScoopRegexMatch){.code = PCRE2_ERROR_NOMEMORY};
    options |= PCRE2_NO_UTF_CHECK;
    if (mode >= 1)
        options |= PCRE2_ANCHORED;
    if (mode == 2)
        options |= PCRE2_ENDANCHORED;
    int rc = pcre2_match(code, input, (PCRE2_SIZE)length, (PCRE2_SIZE)start, options, data, NULL);
    if (rc < 0) {
        pcre2_match_data_free(data);
        return (ScoopRegexMatch){.code = rc};
    }
    PCRE2_SIZE next_start = 0;
    uint32_t next_options = 0;
    bool next = pcre2_next_match(data, &next_start, &next_options);
    return (ScoopRegexMatch){(uint64_t)(uintptr_t)data, rc, (int64_t)next_start, next_options,
                             next};
}

void scoop_regex_match_free(uint64_t data) { pcre2_match_data_free(scoop_regex_data(data)); }

int64_t scoop_regex_group_count(uint64_t data) {
    return pcre2_get_ovector_count(scoop_regex_data(data));
}

ScoopRegexRange scoop_regex_group(uint64_t data, int64_t group) {
    PCRE2_SIZE *vector = pcre2_get_ovector_pointer(scoop_regex_data(data));
    PCRE2_SIZE start = vector[group * 2];
    PCRE2_SIZE end = vector[group * 2 + 1];
    return (ScoopRegexRange){start == PCRE2_UNSET ? -1 : (int64_t)start,
                             end == PCRE2_UNSET ? -1 : (int64_t)end};
}
