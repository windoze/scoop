#ifndef SCOOP_REGEX_H
#define SCOOP_REGEX_H

#define PCRE2_CODE_UNIT_WIDTH 8
#include <pcre2.h>
#include <stdbool.h>
#include <stdint.h>

typedef struct {
    uint64_t handle;
    int32_t code;
    int64_t offset;
} ScoopRegexCompile;

typedef struct {
    const uint8_t *text;
    int64_t length;
    int64_t group;
} ScoopRegexName;

typedef struct {
    int64_t start;
    int64_t end;
} ScoopRegexRange;

typedef struct {
    uint64_t data;
    int32_t code;
    int64_t next_start;
    uint32_t next_options;
    bool has_next;
} ScoopRegexMatch;

typedef struct {
    uint8_t *data;
    int64_t length;
    int32_t code;
    int64_t offset;
    bool replacement_error;
} ScoopRegexText;

static inline pcre2_code *scoop_regex_code(uint64_t handle) {
    return (pcre2_code *)(uintptr_t)handle;
}

static inline pcre2_match_data *scoop_regex_data(uint64_t handle) {
    return (pcre2_match_data *)(uintptr_t)handle;
}

#endif
