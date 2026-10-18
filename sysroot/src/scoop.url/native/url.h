#ifndef SCOOP_URL_H
#define SCOOP_URL_H

#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <uriparser/Uri.h>

typedef struct {
  UriUriA uri;
  char *input;
  char *path;
  char *serialized;
} scoop_url;

typedef struct {
  scoop_url *value;
  int32_t code;
  int64_t offset;
} scoop_url_result;

typedef struct {
  const char *data;
  int64_t size;
} scoop_url_text;

void scoop_url_free(scoop_url *value);
scoop_url_result scoop_url_error(scoop_url *value, int code, int64_t offset);
int scoop_url_finish(scoop_url *value);

#endif
