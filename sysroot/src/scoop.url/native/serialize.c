#include "url.h"
#include <limits.h>

int scoop_url_finish(scoop_url *value) {
  int count;
  int code = uriToStringCharsRequiredA(&value->uri, &count);
  if (code)
    return code;
  if (count == INT_MAX)
    return URI_ERROR_OUTPUT_TOO_LARGE;
  char *serialized = malloc((size_t)count + 1);
  char *path = malloc((size_t)count + 1);
  if (!serialized || !path) {
    free(serialized);
    free(path);
    return URI_ERROR_MALLOC;
  }
  code = uriToStringA(serialized, &value->uri, count + 1, NULL);
  if (code) {
    free(serialized);
    free(path);
    return code;
  }
  size_t used = 0;
  int slash = value->uri.absolutePath ||
              (value->uri.hostText.first && value->uri.pathHead);
  for (UriPathSegmentA *part = value->uri.pathHead; part; part = part->next) {
    if (slash)
      path[used++] = '/';
    size_t size = part->text.first
                      ? (size_t)(part->text.afterLast - part->text.first)
                      : 0;
    if (size)
      memcpy(path + used, part->text.first, size);
    used += size;
    slash = 1;
  }
  if (!value->uri.pathHead && value->uri.absolutePath)
    path[used++] = '/';
  path[used] = 0;
  free(value->path);
  free(value->serialized);
  value->path = path;
  value->serialized = serialized;
  return 0;
}
