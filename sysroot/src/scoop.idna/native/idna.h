#ifndef SCOOP_IDNA_H
#define SCOOP_IDNA_H
#include <idn2.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef struct {
  uint8_t *data;
  int64_t size;
  int32_t code;
  int64_t label;
} scoop_idna_result;

enum {
  SCOOP_IDNA_EMPTY = -1000,
  SCOOP_IDNA_LABEL = -1001,
  SCOOP_IDNA_ALLOC = -1002,
  SCOOP_IDNA_DISALLOWED = -1003,
  SCOOP_IDNA_LENGTH = -1004,
  SCOOP_IDNA_SCALAR = -1005
};

scoop_idna_result scoop_idna_error(int code, int64_t label);
scoop_idna_result scoop_idna_bytes(uint8_t *bytes, size_t size);
#endif
