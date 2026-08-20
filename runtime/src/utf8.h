#ifndef SCOOP_UTF8_H
#define SCOOP_UTF8_H

#include <stdbool.h>
#include <stdint.h>

/* Input values are Unicode scalars; byte input is a complete valid UTF-8
 * scalar from an immutable String. These operations do not allocate. */
uint64_t scoop_utf8_width(uint32_t value);
uint64_t scoop_utf8_encode(unsigned char *bytes, uint32_t value);
uint32_t scoop_utf8_decode(const unsigned char *bytes);

typedef struct ScoopUtf8Step {
    uint32_t scalar;
    uint8_t consumed;
    bool valid;
} ScoopUtf8Step;

/* Decode one scalar from a nonempty bounded range. Invalid input consumes
 * exactly one Unicode maximal subpart and yields U+FFFD. */
ScoopUtf8Step scoop_utf8_next(const unsigned char *bytes, uint64_t remaining);

#endif
