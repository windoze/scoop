#ifndef SCOOP_UTF8_H
#define SCOOP_UTF8_H

#include <stdint.h>

/* Input values are Unicode scalars; byte input is a complete valid UTF-8
 * scalar from an immutable String. These operations do not allocate. */
uint64_t scoop_utf8_width(uint32_t value);
uint64_t scoop_utf8_encode(unsigned char *bytes, uint32_t value);
uint32_t scoop_utf8_decode(const unsigned char *bytes);

#endif
