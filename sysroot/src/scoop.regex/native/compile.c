#include "regex.h"
#include <string.h>

ScoopRegexCompile scoop_regex_compile(const uint8_t *pattern, int64_t length, uint32_t flags) {
    uint32_t options = PCRE2_UTF | PCRE2_UCP | PCRE2_NEVER_BACKSLASH_C;
    if (flags & 1)
        options |= PCRE2_CASELESS;
    if (flags & 2)
        options |= PCRE2_MULTILINE;
    if (flags & 4)
        options |= PCRE2_DOTALL;
    if (flags & 8)
        options |= PCRE2_EXTENDED;
    if (flags & 32)
        options |= PCRE2_DUPNAMES;
    pcre2_compile_context *context = pcre2_compile_context_create(NULL);
    if (!context)
        return (ScoopRegexCompile){0, PCRE2_ERROR_NOMEMORY, 0};
    pcre2_set_newline(context, PCRE2_NEWLINE_ANY);
    pcre2_set_bsr(context, PCRE2_BSR_UNICODE);
    if (flags & 16) {
        pcre2_set_compile_extra_options(context, PCRE2_EXTRA_ASCII_BSD | PCRE2_EXTRA_ASCII_BSS |
                                                     PCRE2_EXTRA_ASCII_BSW |
                                                     PCRE2_EXTRA_ASCII_POSIX);
    }
    int code = 0;
    PCRE2_SIZE offset = 0;
    pcre2_code *compiled =
        pcre2_compile(pattern, (PCRE2_SIZE)length, options, &code, &offset, context);
    pcre2_compile_context_free(context);
    /* JIT is optional: the compiled interpreter code remains usable on any JIT failure. */
    if (compiled)
        pcre2_jit_compile(compiled, PCRE2_JIT_COMPLETE);
    if (code == PCRE2_ERROR_HEAP_FAILED)
        code = PCRE2_ERROR_NOMEMORY;
    return (ScoopRegexCompile){(uint64_t)(uintptr_t)compiled, code, (int64_t)offset};
}

void scoop_regex_free(uint64_t handle) { pcre2_code_free(scoop_regex_code(handle)); }

int64_t scoop_regex_name_count(uint64_t handle) {
    uint32_t count = 0;
    pcre2_pattern_info(scoop_regex_code(handle), PCRE2_INFO_NAMECOUNT, &count);
    return count;
}

ScoopRegexName scoop_regex_name(uint64_t handle, int64_t index) {
    uint32_t width = 0;
    PCRE2_SPTR table = NULL;
    pcre2_pattern_info(scoop_regex_code(handle), PCRE2_INFO_NAMEENTRYSIZE, &width);
    pcre2_pattern_info(scoop_regex_code(handle), PCRE2_INFO_NAMETABLE, &table);
    PCRE2_SPTR entry = table + (size_t)index * width;
    return (ScoopRegexName){entry + 2, (int64_t)strlen((const char *)entry + 2),
                            (int64_t)((entry[0] << 8) | entry[1])};
}

int64_t scoop_regex_message(int32_t code, uint8_t *output, int64_t capacity) {
    int length = pcre2_get_error_message(code, output, (PCRE2_SIZE)capacity);
    if (length >= 0)
        return length;
    const char *fallback = "PCRE2 operation failed";
    size_t count = strlen(fallback);
    if (count > (size_t)capacity)
        count = (size_t)capacity;
    memcpy(output, fallback, count);
    return (int64_t)count;
}

int32_t scoop_regex_failure_kind(int32_t code) {
    switch (code) {
    case PCRE2_ERROR_MATCHLIMIT:
        return 0;
    case PCRE2_ERROR_DEPTHLIMIT:
        return 1;
    case PCRE2_ERROR_HEAPLIMIT:
        return 2;
    case PCRE2_ERROR_JIT_STACKLIMIT:
        return 3;
    case PCRE2_ERROR_NOMEMORY:
        return 4;
    default:
        return 5;
    }
}
