#ifndef SCOOP_CHARSET_H
#define SCOOP_CHARSET_H
#include <errno.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

enum { CHARSET_BACKEND = 9 };
typedef struct {
  char *data;
  int64_t size;
  int32_t code, backend;
} scoop_charset_text;
#endif
